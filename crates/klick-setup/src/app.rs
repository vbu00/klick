//! Установщик kl!ck: ставит, обновляет, переустанавливает и удаляет программу. Окно — на том же
//! движке, что и kl!ck (Tauri + React), страница `setup.html`.
//!
//! Без окна, для администраторов и проверок:
//!   klick-setup.exe --silent [--path <папка>] [--no-desktop] [--no-autostart] [--no-migrate] [--wipe]
//!   klick-setup.exe --silent --uninstall [--wipe]
//! Код выхода: 0 — готово, 1 — ошибка, 2 — неверные аргументы.

use klick_setup::{payload, plan, steps, webview2, win};

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use steps::{Kind, Request};
use tauri::{Emitter, Manager, WindowEvent};

#[derive(Default)]
struct Args {
    silent: bool,
    uninstall: bool,
    path: Option<String>,
    no_desktop: bool,
    no_autostart: bool,
    wipe: bool,
    /// Не переносить подписки прежней kl!ck.
    no_migrate: bool,
    /// Эта копия запущена из временной папки и удалит себя при выходе.
    temp_copy: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--silent" | "/S" => a.silent = true,
            "--uninstall" => a.uninstall = true,
            "--path" => a.path = Some(it.next().ok_or("после --path нужна папка")?),
            "--no-desktop" => a.no_desktop = true,
            "--no-autostart" => a.no_autostart = true,
            "--wipe" => a.wipe = true,
            "--no-migrate" => a.no_migrate = true,
            "--temp-copy" => a.temp_copy = true,
            other => return Err(format!("неизвестный аргумент: {other}")),
        }
    }
    Ok(a)
}

pub fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            attach_console();
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    // Копия из папки программы не может удалить или заменить саму себя — работает её двойник
    // из временной папки.
    if let Some(code) = relaunch_from_temp(&args) {
        std::process::exit(code);
    }
    let code = if args.silent {
        let code = silent(&args);
        remove_self_later(None);
        code
    } else {
        window(&args)
    };
    std::process::exit(code);
}

fn relaunch_from_temp(args: &Args) -> Option<i32> {
    if args.temp_copy {
        return None;
    }
    let me = std::env::current_exe().ok()?;
    let installed = plan::installed()?;
    if !win::inside(&me, Path::new(&installed.path)) {
        return None;
    }
    // Тихо переустановить копией из папки программы нельзя: пока она ждёт двойника, её файл
    // занят и папку не отодвинуть в сторону. Удаление — можно: этот файл уберётся после выхода.
    if args.silent && !args.uninstall {
        attach_console();
        eprintln!("запустите установщик не из папки программы: {}", installed.path);
        return Some(2);
    }
    let tmp = std::env::temp_dir().join(format!("klick-setup-{}.exe", std::process::id()));
    std::fs::copy(&me, &tmp).ok()?;
    let mut cmd = Command::new(&tmp);
    cmd.args(std::env::args_os().skip(1)).arg("--temp-copy");
    if args.silent {
        // Ждать и отдать код выхода: так ждут «Установленные приложения» и winget.
        Some(cmd.status().ok()?.code().unwrap_or(1))
    } else {
        cmd.spawn().ok()?;
        Some(0)
    }
}

/// Когда процесс завершится, убрать данные его окна и временную копию установщика.
fn remove_self_later(webview: Option<&Path>) {
    let mut items = Vec::new();
    if let Some(w) = webview {
        items.push(w.to_path_buf());
    }
    if let Ok(me) = std::env::current_exe() {
        // TEMP бывает записан коротким именем (C:\Users\ABCD~1\…) — сравниваем полные пути.
        let in_temp = match (std::fs::canonicalize(&me), std::fs::canonicalize(std::env::temp_dir())) {
            (Ok(exe), Ok(temp)) => exe.starts_with(temp),
            _ => false,
        };
        if in_temp && me.file_name().is_some_and(|n| n.to_string_lossy().starts_with("klick-setup-")) {
            items.push(me);
        }
    }
    if items.is_empty() {
        return;
    }
    // Этот процесс и WebView2 ещё держат файлы — пробовать, пока не отпустят (до минуты).
    let list = items.iter().map(|p| win::ps_quote(&p.to_string_lossy())).collect::<Vec<_>>().join(", ");
    win::spawn_powershell(&format!(
        "$items = @({list}); for ($i = 0; $i -lt 120; $i++) {{ $left = 0; foreach ($p in $items) {{ if (Test-Path -LiteralPath $p) {{ try {{ Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction Stop }} catch {{ $left++ }} }} }}; if ($left -eq 0) {{ break }}; Start-Sleep -Milliseconds 500 }}"
    ));
}

/// Тихий режим из консоли: окна у установщика нет, печатаем в консоль того, кто запустил.
/// Если вывод перенаправлен в файл, он уже есть — ничего не трогаем.
fn attach_console() {
    use windows::Win32::System::Console::{AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE};
    unsafe {
        if GetStdHandle(STD_OUTPUT_HANDLE).map_or(true, |h| h.is_invalid() || h.0.is_null()) {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

// ── Без окна ───────────────────────────────────────────────────────────

fn silent(args: &Args) -> i32 {
    attach_console();
    let info = plan::detect();
    let kind = if args.uninstall {
        if info.installed.is_none() {
            println!("kl!ck не установлен");
            return 0;
        }
        Kind::Uninstall
    } else {
        match &info.installed {
            None => Kind::Install,
            Some(i) if i.relation == "same" => Kind::Reinstall,
            Some(_) => Kind::Update,
        }
    };
    let path = args.path.clone().unwrap_or_else(|| info.default_path.clone());
    let req = Request { kind, path, desktop: !args.no_desktop, autostart: !args.no_autostart, wipe: args.wipe, keep_old: !args.no_migrate };
    // Окно kl!ck без WebView2 не откроется — поставить заранее, без вопросов: это тихий режим.
    if kind != Kind::Uninstall && !webview2::installed() {
        println!("ставлю Microsoft Edge WebView2 Runtime…");
        if let Err(e) = webview2::install(true) {
            eprintln!("предупреждение: {e}. Служба kl!ck работает и без него, окно — нет.");
        }
    }
    let cancel = AtomicBool::new(false);
    let mut last = usize::MAX;
    let out = steps::run(&req, &info, &cancel, &mut |p| {
        if p.active != last {
            last = p.active;
            if let Some(t) = p.tasks.get(p.active) {
                println!("[{:>3.0}%] {}", p.pct, t.label());
            }
        }
    });
    if out.ok {
        println!("[100%] готово: {}", out.path);
        if out.migrated > 0 || !out.not_migrated.is_empty() {
            println!("перенесено из прежней kl!ck: {}", out.migrated);
            for n in &out.not_migrated {
                println!("не перенеслось: {n}");
            }
        }
        for n in &out.notes {
            println!("заметка: {n}");
        }
        0
    } else {
        eprintln!("ошибка: {} {}", out.error.as_deref().unwrap_or("?"), out.detail.as_deref().unwrap_or(""));
        if out.rolled_back {
            eprintln!("всё возвращено как было");
        }
        1
    }
}

// ── Окно ───────────────────────────────────────────────────────────────

struct Setup {
    /// С какого экрана начать: `install`, `maintain` или `uninstall`.
    start: &'static str,
    info: Mutex<plan::Info>,
    cancel: Arc<AtomicBool>,
    busy: AtomicBool,
}

#[derive(Serialize)]
struct Hello {
    start: &'static str,
    info: plan::Info,
}

#[tauri::command]
fn setup_info(state: tauri::State<Setup>) -> Hello {
    Hello { start: state.start, info: state.info.lock().unwrap().clone() }
}

/// Проверить папку; вернуть её в обычном виде или код ошибки.
#[tauri::command]
fn check_path(path: String) -> Result<String, String> {
    plan::check_path(&path).map(|p| p.to_string_lossy().into_owned()).map_err(String::from)
}

#[tauri::command]
fn free_space(path: String) -> Option<u64> {
    win::free_bytes(Path::new(path.trim()))
}

#[derive(Serialize)]
struct Licenses {
    klick: String,
    notices: String,
}

#[tauri::command]
fn licenses() -> Licenses {
    Licenses {
        klick: payload::read("resources/licenses/LICENSE.txt").unwrap_or_default(),
        notices: payload::read("resources/licenses/THIRD_PARTY_NOTICES.md").unwrap_or_default(),
    }
}

#[tauri::command]
fn start(app: tauri::AppHandle, state: tauri::State<Setup>, req: Request) -> Result<(), String> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("busy".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    std::thread::spawn(move || {
        // Заново: за время на экранах что-то могло поменяться.
        let info = plan::detect();
        let out = steps::run(&req, &info, &cancel, &mut |p| {
            let _ = app.emit("setup://progress", p);
        });
        let state = app.state::<Setup>();
        *state.info.lock().unwrap() = plan::detect();
        state.busy.store(false, Ordering::SeqCst);
        let _ = app.emit("setup://done", &out);
    });
    Ok(())
}

#[tauri::command]
fn cancel(state: tauri::State<Setup>) {
    state.cancel.store(true, Ordering::SeqCst);
}

/// Открыть kl!ck от имени пользователя, а не администратора.
#[tauri::command]
fn launch_app(path: String) -> Result<(), String> {
    win::launch_as_user(&PathBuf::from(path).join("klick.exe"))
}

#[tauri::command]
fn quit(app: tauri::AppHandle, state: tauri::State<Setup>) {
    if !state.busy.load(Ordering::SeqCst) {
        app.exit(0);
    }
}

fn window(args: &Args) -> i32 {
    // Окно установщика — тоже WebView2. Без него Tauri показал бы «Could not find the WebView2
    // Runtime» и закрылся; спрашиваем и ставим сами.
    if !webview2::installed() {
        if !webview2::ask_install() {
            return 1;
        }
        if let Err(e) = webview2::install(false) {
            webview2::tell(&format!("Не получилось установить WebView2: {e}.\n\nСкачайте его с сайта Microsoft (Microsoft Edge WebView2 Runtime) и запустите установщик kl!ck снова."));
            return 1;
        }
    }
    let info = plan::detect();
    let first = if args.uninstall && info.installed.is_some() {
        "uninstall"
    } else if info.installed.is_some() {
        "maintain"
    } else {
        "install"
    };
    // Данные WebView2 — во временной папке: установщик не оставляет следов в профиле.
    let webview = std::env::temp_dir().join(format!("klick-setup-webview-{}", std::process::id()));
    let data = webview.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Setup { start: first, info: Mutex::new(info), cancel: Arc::new(AtomicBool::new(false)), busy: AtomicBool::new(false) })
        .invoke_handler(tauri::generate_handler![setup_info, check_path, free_space, licenses, start, cancel, launch_app, quit])
        .setup(move |app| {
            tauri::WebviewWindowBuilder::new(app, "setup", tauri::WebviewUrl::App("setup.html".into()))
                .title("Установка kl!ck")
                .inner_size(680.0, 440.0)
                .resizable(false)
                .maximizable(false)
                .decorations(false)
                .shadow(true)
                .center()
                .background_color(tauri::window::Color(0x1a, 0x1a, 0x1d, 0xff))
                .data_directory(data.clone())
                .build()?;
            Ok(())
        })
        .on_window_event(|w, e| {
            // Крестик, Alt+F4: решает страница — спросить «Прервать?» или закрыть.
            if let WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = w.emit("setup://close", ());
            }
        })
        .build(tauri::generate_context!());
    let code = match app {
        Ok(app) => app.run_return(|_, _| {}),
        Err(e) => {
            eprintln!("окно установщика: {e}");
            1
        }
    };
    remove_self_later(Some(&webview));
    code
}
