//! Установка, обновление и удаление kl!ck.
//!
//! Файлы, ярлык в «Пуске», запись в «Установленных приложениях» и хуки
//! (права на папку, снятие правил Kill Switch при удалении) делает вшитый
//! NSIS-установщик kl!ck в тихом режиме — тот же, что выкладывается в
//! релизах. Здесь — то, чего он в тихом режиме не умеет: выбор ярлыка на
//! рабочем столе и автозапуска, прогресс, откат при отмене, удаление данных
//! и собственная запись «Удалить» в параметрах Windows.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::reg;

pub const VERSION: &str = env!("KLICK_VERSION");
static PAYLOAD: &[u8] = include_bytes!(env!("KLICK_PAYLOAD"));
pub const LICENSE: &str = include_str!(env!("KLICK_LICENSE"));
const EXE: &str = "klick.exe";
/// Копия установщика в папке kl!ck — её открывает «Удалить» в параметрах Windows.
pub const SELF_NAME: &str = "klick-setup.exe";
const TASK: &str = "klick-Autostart";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const fn parse(s: &str) -> u64 {
    let b = s.as_bytes();
    let mut i = 0;
    let mut n = 0u64;
    while i < b.len() {
        n = n * 10 + (b[i] - b'0') as u64;
        i += 1;
    }
    n
}
/// Сколько займёт kl!ck после установки (приложение, ядро, база GeoIP).
pub const SIZE: u64 = parse(env!("KLICK_SIZE"));
const APP_SIZE: u64 = parse(env!("KLICK_APP_SIZE"));

static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static CANCEL: AtomicBool = AtomicBool::new(false);
/// Установщик запущен из папки, которую только что удалили, — стереть себя при выходе.
pub static SELF_DELETE: AtomicBool = AtomicBool::new(false);

fn system_exe(name: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(name)
}

fn quiet(exe: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(exe);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

fn semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim().trim_start_matches('v').split(['.', '-']).map(|x| x.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

// ─────────── Что уже стоит ───────────

#[derive(Serialize, Clone)]
pub struct Installed {
    pub version: String,
    pub path: String,
}

pub fn installed() -> Option<Installed> {
    let k = reg::hklm(reg::UNINSTALL_KEY, false)?;
    let path = k.get_string("InstallLocation")?.trim().trim_matches('"').to_string();
    if !Path::new(&path).join(EXE).exists() {
        return None;
    }
    Some(Installed { version: k.get_string("DisplayVersion").unwrap_or_default(), path })
}

pub fn default_path() -> String {
    let pf = std::env::var("ProgramW6432").or_else(|_| std::env::var("ProgramFiles")).unwrap_or_else(|_| r"C:\Program Files".into());
    format!(r"{pf}\kl!ck")
}

/// Папка, выбранная в диалоге: kl!ck ставится в свою подпапку, а не
/// россыпью в «D:\Games». Права на папку установщик меняет только у папки
/// с именем kl!ck — так безопаснее.
pub fn product_dir(picked: &Path) -> PathBuf {
    let last = picked.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if last == "kl!ck" { picked.to_path_buf() } else { picked.join("kl!ck") }
}

/// Свободно на диске папки (по ближайшему существующему родителю).
pub fn free_space(path: &str) -> Option<u64> {
    let mut p = PathBuf::from(path);
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let w: Vec<u16> = p.as_os_str().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        let mut free = 0u64;
        let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
        return (ok != 0).then_some(free);
    }
    #[allow(unreachable_code)]
    None
}

/// −1 — стоит старее, 0 — та же версия, 1 — стоит новее.
pub fn compare(installed: &str) -> i32 {
    match semver(installed).cmp(&semver(VERSION)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

// ─────────── Работающий kl!ck и системный прокси ───────────

fn app_running() -> bool {
    quiet(system_exe("tasklist.exe")).args(["/FI", "IMAGENAME eq klick.exe", "/NH"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("klick.exe")).unwrap_or(false)
}

/// Остановить kl!ck перед заменой файлов. Правила Kill Switch остаются —
/// это их работа, пока приложения нет. Системный прокси, если kl!ck его
/// включал, возвращается как было: иначе он указывал бы в никуда.
fn stop_app() -> bool {
    if !app_running() {
        return false;
    }
    let _ = quiet(system_exe("taskkill.exe")).args(["/F", "/T", "/IM", EXE]).status();
    std::thread::sleep(Duration::from_millis(700));
    restore_proxy();
    true
}

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("com.vbu00.klick"))
}

/// То же, что делает sysproxy::disable в приложении.
fn restore_proxy() {
    let Some(dir) = data_dir() else { return };
    let port = std::fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("proxyPort").and_then(|p| p.as_u64()))
        .unwrap_or(7890);
    let ours = format!("127.0.0.1:{port}");
    let Some(k) = reg::hkcu(reg::INET_KEY, true) else { return };
    if k.get_dword("ProxyEnable") != Some(1) || k.get_string("ProxyServer").as_deref() != Some(ours.as_str()) {
        return;
    }
    let backup = dir.join("sysproxy-backup.json");
    let prev = std::fs::read(&backup).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    let server = prev.as_ref().and_then(|v| v.get("server")).and_then(|s| s.as_str()).map(String::from);
    let overrides = prev.as_ref().and_then(|v| v.get("overrides")).and_then(|s| s.as_str()).map(String::from);
    let enable = prev.as_ref().and_then(|v| v.get("enable")).and_then(|e| e.as_u64()).unwrap_or(0) as u32;
    let enable = if server.as_deref() == Some(ours.as_str()) { 0 } else { enable };
    k.set_dword("ProxyEnable", enable);
    match &server {
        Some(s) if s != &ours => { k.set_string("ProxyServer", s); }
        _ => k.delete("ProxyServer"),
    }
    match &overrides {
        Some(o) => { k.set_string("ProxyOverride", o); }
        None => k.delete("ProxyOverride"),
    }
    let _ = std::fs::remove_file(backup);
    reg::proxy_changed();
}

// ─────────── Прогресс ───────────

#[derive(Serialize, Clone)]
struct Progress {
    pct: f64,
    stage: usize,
}
fn emit(app: &AppHandle, pct: f64, stage: usize) {
    let _ = app.emit("progress", Progress { pct: pct.clamp(0.0, 100.0), stage });
}

fn file_len(p: PathBuf) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn desktop_shortcuts() -> Vec<PathBuf> {
    ["PUBLIC", "USERPROFILE"].iter().filter_map(|v| std::env::var_os(v)).map(|d| PathBuf::from(d).join("Desktop").join("kl!ck.lnk")).collect()
}

// ─────────── Установка ───────────

/// kind: install — первая установка; update / reinstall / downgrade — поверх.
pub fn install(app: &AppHandle, path: &str, desktop: bool, autostart: bool, kind: &str) -> Result<(), String> {
    CANCEL.store(false, Ordering::SeqCst);
    let fresh = kind == "install";
    let dir = PathBuf::from(path);
    emit(app, 2.0, 0);
    let had_desktop = desktop_shortcuts().iter().any(|p| p.exists());
    if !fresh {
        stop_app();
    }
    let existed = dir.exists();

    // Вшитый установщик — во временный файл.
    let payload = std::env::temp_dir().join(format!("klick-{}-payload.exe", VERSION));
    std::fs::write(&payload, PAYLOAD).map_err(|e| format!("Не удалось распаковать установщик: {e}"))?;
    emit(app, 8.0, 0);

    // /D= — последним и без кавычек, даже с пробелами: так требует NSIS.
    let mut cmd = quiet(&payload);
    cmd.arg("/S");
    #[cfg(windows)]
    cmd.raw_arg(format!("/D={}", dir.display()));
    let child = cmd.spawn().map_err(|e| format!("Не удалось запустить установку: {e}"))?;
    *CHILD.lock().unwrap() = Some(child);

    let started = Instant::now();
    let code = loop {
        std::thread::sleep(Duration::from_millis(80));
        if CANCEL.load(Ordering::SeqCst) {
            if let Some(mut c) = CHILD.lock().unwrap().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            rollback(&dir, existed);
            let _ = std::fs::remove_file(&payload);
            return Err("cancelled".into());
        }
        let status = CHILD.lock().unwrap().as_mut().and_then(|c| c.try_wait().ok().flatten());
        if let Some(s) = status {
            CHILD.lock().unwrap().take();
            break s.code().unwrap_or(-1);
        }
        // Первая установка — по байтам, что уже легли в папку; поверх старой
        // версии файлы на месте с самого начала, там — по времени.
        let secs = started.elapsed().as_secs_f64();
        let by_time = 1.0 - (-secs / 2.2).exp();
        let bytes = file_len(dir.join(EXE)) + file_len(dir.join("bin").join("mihomo.exe")) + file_len(dir.join("bin").join("Country.mmdb"));
        let by_bytes = if fresh && SIZE > 0 { bytes as f64 / SIZE as f64 } else { 0.0 };
        let p = if fresh { by_bytes.max(by_time * 0.5) } else { by_time };
        let app_done = if fresh { bytes >= APP_SIZE } else { by_time > 0.45 };
        emit(app, 8.0 + 80.0 * p.min(1.0), if app_done { 2 } else { 1 });
    };
    let _ = std::fs::remove_file(&payload);
    if code != 0 {
        if fresh {
            rollback(&dir, existed);
        }
        return Err(format!("Установщик kl!ck завершился с кодом {code}."));
    }

    // Ярлыки и автозапуск.
    emit(app, 92.0, 3);
    // В тихом режиме NSIS всегда кладёт ярлык на рабочий стол: убираем,
    // если его не просили (при обновлении — если его не было).
    if (fresh && !desktop) || (!fresh && !had_desktop) {
        for p in desktop_shortcuts() {
            let _ = std::fs::remove_file(p);
        }
    }
    if fresh {
        set_autostart(&dir, autostart);
    }
    install_self(&dir);
    emit(app, 100.0, 3);
    Ok(())
}

/// Задача Планировщика — та же, что включает сам kl!ck в настройках.
fn set_autostart(dir: &Path, on: bool) {
    let schtasks = system_exe("schtasks.exe");
    if on {
        let tr = format!("\"{}\" --autostart", dir.join(EXE).display());
        let _ = quiet(&schtasks).args(["/Create", "/TN", TASK, "/TR", &tr, "/SC", "ONLOGON", "/RL", "HIGHEST", "/F"]).status();
    } else {
        let _ = quiet(&schtasks).args(["/Delete", "/TN", TASK, "/F"]).status();
    }
}

/// Копия установщика в папку kl!ck — и «Удалить» в параметрах Windows
/// открывает её, а не безликий деинсталлятор NSIS.
fn install_self(dir: &Path) {
    let target = dir.join(SELF_NAME);
    let Ok(me) = std::env::current_exe() else { return };
    if me != target && std::fs::copy(&me, &target).is_err() {
        return;
    }
    if let Some(k) = reg::hklm(reg::UNINSTALL_KEY, true) {
        k.set_string("UninstallString", &format!("\"{}\" --uninstall", target.display()));
        k.set_string("ModifyPath", &format!("\"{}\"", target.display()));
    }
}

/// Откат прерванной первой установки: удалить всё, что успело появиться.
fn rollback(dir: &Path, existed: bool) {
    let un = dir.join("uninstall.exe");
    if un.exists() {
        let mut c = quiet(&un);
        #[cfg(windows)]
        c.raw_arg(format!("/S _?={}", dir.display()));
        let _ = c.status();
    }
    if !existed {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

// ─────────── Удаление ───────────

pub fn uninstall(app: &AppHandle, wipe: bool) -> Result<(), String> {
    let inst = installed().ok_or("kl!ck не найден — возможно, уже удалён.")?;
    let dir = PathBuf::from(&inst.path);
    emit(app, 3.0, 0);
    stop_app();
    emit(app, 12.0, 1);

    // Деинсталлятор NSIS сам снимает правила Kill Switch и прокси
    // (klick.exe --cleanup), задачу автозапуска, ярлыки и запись в реестре.
    // _?= — работать на месте, а не копией во временной папке: иначе не
    // дождаться его конца.
    let un = dir.join("uninstall.exe");
    let mut c = quiet(&un);
    #[cfg(windows)]
    c.raw_arg(format!("/S _?={}", dir.display()));
    let mut child = c.spawn().map_err(|e| format!("Не удалось запустить удаление: {e}"))?;
    let started = Instant::now();
    let code = loop {
        std::thread::sleep(Duration::from_millis(80));
        if let Some(s) = child.try_wait().ok().flatten() {
            break s.code().unwrap_or(-1);
        }
        let p = 1.0 - (-started.elapsed().as_secs_f64() / 1.6).exp();
        emit(app, 12.0 + 70.0 * p, if p > 0.4 { 2 } else { 1 });
    };
    if code != 0 {
        return Err(format!("Деинсталлятор kl!ck завершился с кодом {code}."));
    }
    let _ = std::fs::remove_file(&un);
    let me = std::env::current_exe().ok();
    let copy = dir.join(SELF_NAME);
    if me.as_deref() == Some(copy.as_path()) {
        SELF_DELETE.store(true, Ordering::SeqCst);
    } else {
        let _ = std::fs::remove_file(&copy);
        let _ = std::fs::remove_dir(&dir);
    }
    emit(app, 88.0, 2);
    if wipe {
        emit(app, 92.0, 3);
        if let Some(d) = data_dir() {
            let _ = std::fs::remove_dir_all(d);
        }
    }
    emit(app, 100.0, if wipe { 3 } else { 2 });
    Ok(())
}

/// Стереть себя и пустую папку kl!ck после выхода — пока процесс жив,
/// Windows не даёт удалить его файл.
pub fn schedule_self_delete() {
    let Ok(me) = std::env::current_exe() else { return };
    let dir = me.parent().map(Path::to_path_buf).unwrap_or_default();
    let script = format!("ping -n 3 127.0.0.1 >nul & del /f /q \"{}\" & rmdir \"{}\"", me.display(), dir.display());
    let mut c = quiet(system_exe("cmd.exe"));
    c.arg("/c");
    #[cfg(windows)]
    c.raw_arg(script);
    let _ = c.spawn();
}

pub fn launch(dir: &str) -> Result<(), String> {
    quiet(Path::new(dir).join(EXE)).spawn().map(|_| ()).map_err(|e| format!("Не удалось запустить kl!ck: {e}"))
}

