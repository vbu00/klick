//! Окно kl!ck: интерфейс из `app/ui`, мост к службе, значок и своё окно трея,
//! системный прокси по состоянию службы, уведомления Windows.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
mod notify;
mod sysproxy;
mod tray;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

/// Служба для разработки слушает другой канал: `KLICK_DEV=1`.
fn pipe_name() -> String {
    if std::env::var_os("KLICK_DEV").is_some() {
        r"\\.\pipe\klick-dev".into()
    } else {
        r"\\.\pipe\klick".into()
    }
}

pub(crate) fn show_main(app: &AppHandle) {
    if let Some(t) = app.get_webview_window("tray") {
        let _ = t.hide();
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
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
    let _ = w.show();
    let _ = w.set_focus();
}

fn toggle_tray(app: &AppHandle) {
    let Some(w) = app.get_webview_window("tray") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
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

#[tauri::command]
fn open_tray(app: AppHandle) {
    show_tray(&app);
}

#[tauri::command]
fn hide_tray(app: AppHandle) {
    if let Some(t) = app.get_webview_window("tray") {
        let _ = t.hide();
    }
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        // Автозапуск при входе в Windows: окно сразу в трей, VPN — по кнопке или «Восстанавливать подключение».
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .manage(bridge::Bridge::new(pipe_name()))
        .invoke_handler(tauri::generate_handler![bridge::service_call, bridge::service_up, notify::notify, open_main, open_tray, hide_tray, fit_tray, clipboard_text, app_exit])
        .setup(|app| {
            bridge::start_events(app.handle().clone());
            fit_main(app.handle());
            if let Some(w) = app.get_webview_window("main") {
                session_end::watch(&w);
            }
            // Окно создаётся скрытым: при автозапуске оно остаётся в трее.
            if !std::env::args().any(|a| a == "--hidden") {
                show_main(app.handle());
            }

            // Левый щелчок — главное окно; правый — своё окно трея вместо системного меню
            // («Выход» — крестиком в нём).
            let mut tray = TrayIconBuilder::with_id("main").tooltip("kl!ck");
            if let Some(icon) = tray::initial().or_else(|| app.default_window_icon().cloned()) {
                tray = tray.icon(icon);
            }
            tray.on_tray_icon_event(|tray, event| {
                tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                if let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, .. } = event {
                    match button {
                        MouseButton::Left => show_main(tray.app_handle()),
                        MouseButton::Right => toggle_tray(tray.app_handle()),
                        _ => {}
                    }
                }
            })
            .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            // Крестик сворачивает в трей: VPN и служба продолжают работать.
            (_, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            // Окно трея — как всплывающее меню: щёлкнули мимо, и оно спряталось.
            ("tray", WindowEvent::Focused(false)) => {
                TRAY_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
                let _ = window.hide();
            }
            // Окна фиксированного размера: двойной щелчок по заголовку, Win+↑ и «прилипание» к краю
            // экрана разворачивают их и без кнопки — сразу возвращаем как было.
            (_, WindowEvent::Resized(_)) => {
                if window.is_maximized().unwrap_or(false) {
                    let _ = window.unmaximize();
                }
            }
            // Окно перетащили на экран с другим масштабом: высоту — снова под рабочую область.
            ("main", WindowEvent::ScaleFactorChanged { .. }) => fit_main(window.app_handle()),
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("окно kl!ck не запустилось");
}
