//! Что умеют только Swift и Go: туннель Network Extension, общая папка App Group, ID устройства,
//! буфер обмена (`gen/apple/Sources/klick-ios/Host.swift`) и ядро mihomo (`core/export.go`).
//! Всё это — функции C, которые связываются при сборке приложения в Xcode.

use klick_service::ios::Host;
use std::ffi::{c_char, c_int, CStr, CString};
use std::path::PathBuf;

extern "C" {
    fn klick_group_dir() -> *mut c_char;
    fn klick_device_id() -> *mut c_char;
    fn klick_clipboard() -> *mut c_char;
    fn klick_setup_webview(webview: *mut std::ffi::c_void);
    fn klick_tunnel_start() -> *mut c_char;
    fn klick_tunnel_reload() -> *mut c_char;
    fn klick_tunnel_stop();
    fn klick_tunnel_connected() -> bool;
    fn klickcore_start(home: *const c_char, config: *const c_char, tun_fd: c_int) -> *mut c_char;
    fn klickcore_stop();
}

/// Строка от Swift или Go (выделена `malloc`) — в `String`, память освобождается. NULL — `None`.
fn take(p: *mut c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    unsafe { libc::free(p.cast()) };
    Some(s)
}

/// NULL — получилось, иначе текст ошибки.
fn result(p: *mut c_char) -> Result<(), String> {
    take(p).map_or(Ok(()), Err)
}

/// Общая папка приложения и расширения VPN. `None` — подпись без App Group.
pub fn group_dir() -> Option<PathBuf> {
    take(unsafe { klick_group_dir() }).map(PathBuf::from)
}

/// Окно на весь экран (WKWebView без автоматических отступов). Зовётся на главном потоке.
pub fn setup_webview(webview: *mut std::ffi::c_void) {
    unsafe { klick_setup_webview(webview) }
}

pub fn clipboard() -> String {
    take(unsafe { klick_clipboard() }).unwrap_or_default()
}

fn core_start(home: &str, config: &str) -> Result<(), String> {
    let (home, config) = (CString::new(home).map_err(|e| e.to_string())?, CString::new(config).map_err(|e| e.to_string())?);
    // -1: без адаптера — это проверочное ядро в приложении.
    result(unsafe { klickcore_start(home.as_ptr(), config.as_ptr(), -1) })
}

pub fn host() -> Host {
    Host {
        tunnel_start: || result(unsafe { klick_tunnel_start() }),
        tunnel_reload: || result(unsafe { klick_tunnel_reload() }),
        tunnel_stop: || unsafe { klick_tunnel_stop() },
        tunnel_connected: || unsafe { klick_tunnel_connected() },
        core_start,
        core_stop: || unsafe { klickcore_stop() },
        device_id: || take(unsafe { klick_device_id() }),
    }
}

/// Swift: туннель подключился или остановился. `by_user` — его выключили в Настройках или его место
/// занял другой VPN.
#[no_mangle]
pub extern "C" fn klick_tunnel_changed(connected: bool, by_user: bool) {
    crate::tunnel_changed(connected, by_user);
}
