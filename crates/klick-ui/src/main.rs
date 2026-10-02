//! Окно kl!ck: интерфейс из `app/ui`, мост к службе, значок и своё окно трея,
//! системный прокси по состоянию службы (на macOS его ставит служба), уведомления Windows и macOS,
//! ссылки `klick://add` со страницы подписки.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
mod deeplink;
mod notify;
#[cfg(windows)]
mod sysproxy;
#[cfg(not(windows))]
#[path = "sysproxy_mac.rs"]
mod sysproxy;
mod tray;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(target_os = "macos")]
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_positioner::{Position, WindowExt};

/// Служба для разработки слушает другой канал: `KLICK_DEV=1`.
#[cfg(windows)]
fn pipe_name() -> String {
    if std::env::var_os("KLICK_DEV").is_some() {
        r"\\.\pipe\klick-dev".into()
    } else {
        r"\\.\pipe\klick".into()
    }
}

/// macOS: Unix-сокет службы (`/var/run/klick.sock`), для разработки — `/tmp/klick-dev.sock`.
#[cfg(not(windows))]
fn pipe_name() -> String {
    if std::env::var_os("KLICK_DEV").is_some() {
        "/tmp/klick-dev.sock".into()
    } else {
        "/var/run/klick.sock".into()
    }
}

/// Окно запущено из пакета `.app`, а не из папки сборки: от этого зависит, от чьего имени уведомления.
#[cfg(all(target_os = "macos", feature = "mac-notify"))]
pub(crate) fn in_app_bundle() -> bool {
    std::env::current_exe().ok().is_some_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
}

/// Рисует ли страница окна. Tauri прячет и сворачивает только само окно, а WebView2 о том не знает
/// и продолжает крутить анимации: скрытое окно грызло почти ядро процессора и видеокарту, игры подлагивали.
fn set_rendering(w: &WebviewWindow, on: bool) {
    #[cfg(windows)]
    let _ = w.with_webview(move |wv| unsafe {
        let _ = wv.controller().SetIsVisible(on);
    });
    #[cfg(not(windows))]
    let _ = (w, on);
}

fn hide_window(w: &WebviewWindow) {
    let _ = w.hide();
    set_rendering(w, false);
}

fn show_window(w: &WebviewWindow) {
    set_rendering(w, true);
    let _ = w.show();
}

pub(crate) fn show_main(app: &AppHandle) {
    if let Some(t) = app.get_webview_window("tray") {
        hide_window(&t);
    }
    if let Some(w) = app.get_webview_window("main") {
        show_window(&w);
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Главное окно фиксированного размера и не разворачивается. Если экран ниже окна (ноутбук 1280×720,
/// крупный масштаб), окно становится ниже, чтобы влезть целиком: страницы прокручиваются внутри.
fn fit_main(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let Some(m) = w.current_monitor().ok().flatten().or_else(|| w.primary_monitor().ok().flatten()) else { return };
    let Ok(size) = w.inner_size() else { return };
    let size = size.to_logical::<f64>(m.scale_factor());
    let room = m.work_area().size.height as f64 / m.scale_factor() - 16.0;
    if size.height > room {
        let _ = w.set_size(tauri::LogicalSize::new(size.width, room.max(480.0)));
        let _ = w.center();
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Когда окно трея спряталось, потеряв фокус. Щелчок по значку сначала снимает фокус с открытого окна —
/// не открываем его заново тем же щелчком.
static TRAY_HIDDEN_AT: AtomicU64 = AtomicU64::new(0);

fn show_tray(app: &AppHandle) {
    let Some(w) = app.get_webview_window("tray") else { return };
    let _ = w.move_window_constrained(Position::TrayCenter);
    show_window(&w);
    let _ = w.set_focus();
}

fn toggle_tray(app: &AppHandle) {
    let Some(w) = app.get_webview_window("tray") else { return };
    if w.is_visible().unwrap_or(false) {
        hide_window(&w);
        return;
    }
    if now_ms().saturating_sub(TRAY_HIDDEN_AT.load(Ordering::Relaxed)) < 300 {
        return;
    }
    show_tray(app);
}

/// Главное окно из трея; `target` — какой экран открыть («Все настройки в kl!ck»).
#[tauri::command]
fn open_main(app: AppHandle, target: Option<String>) {
    show_main(&app);
    if let Some(t) = target {
        let _ = app.emit_to("main", "klick://navigate", t);
    }
}

/// Ширина окна трея — как в макете; высоту подгоняет страница под содержимое.
const TRAY_WIDTH: f64 = 320.0;

#[tauri::command]
fn fit_tray(app: AppHandle, height: f64) {
    let Some(w) = app.get_webview_window("tray") else { return };
    let mut h = height.max(120.0);
    // Не выше рабочей области экрана: остальное прокрутится внутри.
    if let Ok(Some(m)) = w.current_monitor() {
        h = h.min(m.work_area().size.height as f64 / m.scale_factor() - 24.0);
    }
    let _ = w.set_size(tauri::LogicalSize::new(TRAY_WIDTH, h));
    if w.is_visible().unwrap_or(false) {
        let _ = w.move_window_constrained(Position::TrayCenter);
    }
}

/// Текст из буфера обмена — «Вставить из буфера» в трее.
#[cfg(windows)]
#[tauri::command]
fn clipboard_text() -> String {
    use windows::Win32::Foundation::{HGLOBAL, HWND};
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() || OpenClipboard(HWND::default()).is_err() {
            return String::new();
        }
        let mut out = String::new();
        if let Ok(h) = GetClipboardData(CF_UNICODETEXT) {
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u16;
            if !p.is_null() {
                let mut len = 0;
                while len < 1 << 20 && *p.add(len) != 0 {
                    len += 1;
                }
                out = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
                let _ = GlobalUnlock(g);
            }
        }
        let _ = CloseClipboard();
        out
    }
}

/// macOS: `pbpaste`. Кодировку он берёт из LANG, а у программ из Finder LANG не задан —
/// без него кириллица и эмодзи в ссылке превратились бы в знаки вопроса.
#[cfg(not(windows))]
#[tauri::command]
fn clipboard_text() -> String {
    std::process::Command::new("/usr/bin/pbpaste")
        .env("LANG", "en_US.UTF-8")
        .env("LC_ALL", "en_US.UTF-8")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Авторы kl!ck — в окне «О kl!ck» на macOS.
#[cfg(target_os = "macos")]
const COPYRIGHT: &str = "© vbu00, Dmitriy Medvedev · порт на macOS — limeflash";

/// «Выход»: спросить в окне трея, отключить VPN или оставить его работать.
fn request_exit(app: &AppHandle) {
    show_tray(app);
    let _ = app.emit_to("tray", "klick://exit-request", ());
}

/// macOS: окно «О kl!ck» — авторы.
#[cfg(target_os = "macos")]
fn about_metadata() -> tauri::menu::AboutMetadata<'static> {
    tauri::menu::AboutMetadata {
        credits: Some(
            "Разработка — vbu00 (github.com/vbu00)\nДизайн интерфейса и логотип — Dmitriy Medvedev (github.com/aleuuu)\nПорт на macOS — limeflash (github.com/limeflash)".into(),
        ),
        copyright: Some(COPYRIGHT.into()),
        ..Default::default()
    }
}

/// macOS: своё меню приложения. «Выйти из kl!ck» (⌘Q) спрашивает про VPN, как «Выход» в трее;
/// меню «Правка» нужно, чтобы в полях работали ⌘C, ⌘V и ⌘A.
#[cfg(target_os = "macos")]
fn macos_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    use tauri::menu::{PredefinedMenuItem, Submenu};
    let app_menu = Submenu::with_items(
        app,
        "kl!ck",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("О kl!ck"), Some(about_metadata()))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "app-settings", "Настройки…", true, Some("CmdOrCtrl+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("Скрыть kl!ck"))?,
            &PredefinedMenuItem::hide_others(app, Some("Скрыть остальные"))?,
            &PredefinedMenuItem::show_all(app, Some("Показать все"))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "app-quit", "Выйти из kl!ck", true, Some("CmdOrCtrl+Q"))?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Правка",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("Отменить"))?,
            &PredefinedMenuItem::redo(app, Some("Повторить"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("Вырезать"))?,
            &PredefinedMenuItem::copy(app, Some("Скопировать"))?,
            &PredefinedMenuItem::paste(app, Some("Вставить"))?,
            &PredefinedMenuItem::select_all(app, Some("Выбрать всё"))?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Окно",
        true,
        &[&PredefinedMenuItem::minimize(app, Some("Свернуть"))?, &PredefinedMenuItem::close_window(app, Some("Закрыть окно"))?],
    )?;
    Menu::with_items(app, &[&app_menu, &edit, &window])
}

#[tauri::command]
fn open_tray(app: AppHandle) {
    show_tray(&app);
}

#[tauri::command]
fn hide_tray(app: AppHandle) {
    if let Some(t) = app.get_webview_window("tray") {
        hide_window(&t);
    }
}

/// Кнопка «Свернуть в трей» в заголовке окна.
#[tauri::command]
fn hide_self(window: WebviewWindow) {
    hide_window(&window);
}

/// «Выход». `clear_proxy` — VPN выключили: наш системный прокси тоже снять.
/// Если VPN оставили работать, прокси остаётся: он нужен ядру, а когда VPN выключат, его снимет служба.
#[tauri::command]
fn app_exit(app: AppHandle, clear_proxy: bool) {
    if clear_proxy {
        sysproxy::clear_ours();
    }
    app.exit(0);
}

/// WM_ENDSESSION: Windows завершает сеанс (выключение, перезагрузка, выход). Окно — скрытое
/// или нет — получает это сообщение; ловим его подклассом окна.
#[cfg(windows)]
mod session_end {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;

    unsafe extern "system" fn on_message(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
        if msg == WM_ENDSESSION && wp.0 != 0 {
            crate::sysproxy::session_ending();
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }

    pub fn watch(window: &tauri::WebviewWindow) {
        let Ok(hwnd) = window.hwnd() else { return };
        unsafe {
            let _ = SetWindowSubclass(HWND(hwnd.0 as _), Some(on_message), 1, 0);
        }
    }
}

/// «Перезапустить службу» на экране «Служба не отвечает»: переустановить её из этой программы.
/// macOS спрашивает пароль администратора своим окном. VPN после перезапуска выключен — служба
/// не начнёт переподключаться сама, если зависла именно на этом.
#[tauri::command]
async fn repair_service() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let service = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("klick-service");
        let script = format!(
            "rm -f {} ; {} install",
            sh_quote("/Library/Application Support/klick/runtime.json"),
            sh_quote(&service.to_string_lossy())
        );
        let apple = format!(
            "do shell script \"{}\" with administrator privileges with prompt \"kl!ck перезапустит свою службу.\"",
            script.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let out = tauri::async_runtime::spawn_blocking(move || std::process::Command::new("/usr/bin/osascript").args(["-e", &apple]).output())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            return Ok(());
        }
        let err = String::from_utf8_lossy(&out.stderr);
        // -128 — нажали «Отменить» в окне пароля.
        return Err(if err.contains("-128") { "cancelled".into() } else { err.trim().to_string() });
    }
    #[cfg(not(target_os = "macos"))]
    Err("unsupported".into())
}

/// Строка для /bin/sh в одинарных кавычках: `kl!ck` и пробелы внутри безопасны.
#[cfg(target_os = "macos")]
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Автозапуск при входе в систему: окно сразу в трей, VPN — по кнопке или «Восстанавливать подключение».
/// На macOS — агент launchd пользователя (`~/Library/LaunchAgents/app.klick.desktop.plist`).
fn autostart() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let builder = tauri_plugin_autostart::Builder::new().args(["--hidden"]);
    #[cfg(target_os = "macos")]
    let builder = builder.macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent).app_name("app.klick.desktop");
    builder.build()
}

fn main() {
    let builder = tauri::Builder::default()
        // Первым: вторая копия (ярлык, ссылка klick:// в Windows) передаёт аргументы первой и выходит.
        // Ссылку из них плагин сам отдаёт в on_open_url; окно показываем, кроме автозапуска в трей.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !argv.iter().any(|a| a == "--hidden") {
                show_main(app);
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(autostart());
    #[cfg(target_os = "macos")]
    let builder = builder.menu(macos_menu).on_menu_event(|app, event| match event.id().as_ref() {
        "app-quit" => request_exit(app),
        "app-settings" => {
            show_main(app);
            let _ = app.emit_to("main", "klick://navigate", "settings");
        }
        _ => {}
    });
    builder
        .manage(bridge::Bridge::new(pipe_name()))
        .manage(deeplink::Pending::default())
        .invoke_handler(tauri::generate_handler![
            bridge::service_call,
            bridge::service_up,
            notify::notify,
            open_main,
            open_tray,
            hide_tray,
            hide_self,
            fit_tray,
            clipboard_text,
            app_exit,
            repair_service,
            deeplink::take_pending_link
        ])
        .setup(|app| {
            bridge::start_events(app.handle().clone());
            fit_main(app.handle());
            #[cfg(windows)]
            if let Some(w) = app.get_webview_window("main") {
                session_end::watch(&w);
            }
            // Окно создаётся скрытым: при автозапуске оно остаётся в трее.
            if !std::env::args().any(|a| a == "--hidden") {
                show_main(app.handle());
            }

            // klick://add: пока kl!ck работает — от второй копии (Windows) или от системы (macOS);
            // при запуске по ссылке — из аргументов этого же процесса.
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |e| {
                for url in e.urls() {
                    deeplink::open(&handle, url.as_str());
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    deeplink::open(app.handle(), url.as_str());
                }
            }
            // Отладочная сборка без установщика: схему klick в реестре пишет она сама.
            // Выпуск регистрирует klick-setup, на macOS — Info.plist пакета.
            #[cfg(all(debug_assertions, windows))]
            if let Err(e) = app.deep_link().register_all() {
                eprintln!("схема klick не зарегистрирована: {e}");
            }

            // Windows: левый щелчок — главное окно; правый — своё окно трея вместо системного меню
            // («Выход» — крестиком в нём). macOS: щелчок — окно трея под значком, как у программ
            // в строке меню; правый — запасное меню.
            let mut tray = TrayIconBuilder::with_id("main").tooltip("kl!ck");
            #[cfg(target_os = "macos")]
            {
                let open = MenuItem::with_id(app, "open", "Открыть kl!ck", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
                tray = tray.menu(&Menu::with_items(app, &[&open, &quit])?).show_menu_on_left_click(false);
            }
            if let Some(icon) = tray::initial().or_else(|| app.default_window_icon().cloned()) {
                tray = tray.icon(icon);
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "open" => show_main(app),
                // «Выход» спрашивает в окне трея: отключить VPN или оставить его работать.
                "quit" => request_exit(app),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                if let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, .. } = event {
                    match button {
                        #[cfg(windows)]
                        MouseButton::Left => show_main(tray.app_handle()),
                        #[cfg(windows)]
                        MouseButton::Right => toggle_tray(tray.app_handle()),
                        #[cfg(not(windows))]
                        MouseButton::Left => toggle_tray(tray.app_handle()),
                        _ => {}
                    }
                }
            })
            .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(w) = window.app_handle().get_webview_window(window.label()) else { return };
            match (window.label(), event) {
                // Крестик сворачивает в трей: VPN и служба продолжают работать.
                (_, WindowEvent::CloseRequested { api, .. }) => {
                    api.prevent_close();
                    hide_window(&w);
                }
                // Окно трея — как всплывающее меню: щёлкнули мимо, и оно спряталось.
                ("tray", WindowEvent::Focused(false)) => {
                    TRAY_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
                    hide_window(&w);
                }
                // Окна фиксированного размера: двойной щелчок по заголовку, Win+↑ и «прилипание» к краю
                // экрана разворачивают их и без кнопки — сразу возвращаем как было.
                // Свёрнутое окно не рисует; развернули из панели задач — рисует снова.
                (_, WindowEvent::Resized(_)) => {
                    if window.is_maximized().unwrap_or(false) {
                        let _ = window.unmaximize();
                    }
                    let shown = window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false);
                    set_rendering(&w, shown);
                }
                // Окно перетащили на экран с другим масштабом: высоту — снова под рабочую область.
                ("main", WindowEvent::ScaleFactorChanged { .. }) => fit_main(window.app_handle()),
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("окно kl!ck не запустилось")
        .run(|_app, _event| {
            // macOS: щелчок по значку в Dock, когда окно спрятано в трей, — показать окно.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows: false, .. } = _event {
                show_main(_app);
            }
        });
}
