//! Трей. Меню по правому клику — маленькое окно в стиле приложения
//! (src/tray-menu.html), как в Klutz: встаёт над иконкой и прячется без фокуса.

use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::core;
use crate::state::{AppState, Mode};

pub const TRAY_ID: &str = "klick-tray";
pub const POPUP: &str = "tray-menu";

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Look {
    Ok,
    Warn,
    Bad,
    Idle,
}

impl Look {
    fn key(self) -> &'static str {
        match self {
            Look::Ok => "ok",
            Look::Warn => "warn",
            Look::Bad => "bad",
            Look::Idle => "idle",
        }
    }
}

macro_rules! icon_set {
    ($s:literal) => {
        [
            (16, include_bytes!(concat!("../icons/tray-", $s, "-16.png")) as &[u8]),
            (20, include_bytes!(concat!("../icons/tray-", $s, "-20.png")) as &[u8]),
            (24, include_bytes!(concat!("../icons/tray-", $s, "-24.png")) as &[u8]),
            (28, include_bytes!(concat!("../icons/tray-", $s, "-28.png")) as &[u8]),
            (32, include_bytes!(concat!("../icons/tray-", $s, "-32.png")) as &[u8]),
            (40, include_bytes!(concat!("../icons/tray-", $s, "-40.png")) as &[u8]),
            (48, include_bytes!(concat!("../icons/tray-", $s, "-48.png")) as &[u8]),
            (64, include_bytes!(concat!("../icons/tray-", $s, "-64.png")) as &[u8]),
        ]
    };
}

const ICONS_OK: [(i32, &[u8]); 8] = icon_set!("ok");
const ICONS_WARN: [(i32, &[u8]); 8] = icon_set!("warn");
const ICONS_BAD: [(i32, &[u8]); 8] = icon_set!("bad");
const ICONS_IDLE: [(i32, &[u8]); 8] = icon_set!("idle");

/// Иконка ровно под SM_CXSMICON (16 при 100 %, 20 при 125 %…), а не ужатая.
#[cfg(target_os = "windows")]
fn small_icon_px() -> i32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};
    let px = unsafe { GetSystemMetrics(SM_CXSMICON) };
    if px > 0 { px } else { 16 }
}

#[cfg(not(target_os = "windows"))]
fn small_icon_px() -> i32 {
    32
}

fn icon_bytes(l: Look) -> &'static [u8] {
    let set: &[(i32, &'static [u8])] = match l {
        Look::Ok => &ICONS_OK,
        Look::Warn => &ICONS_WARN,
        Look::Bad => &ICONS_BAD,
        Look::Idle => &ICONS_IDLE,
    };
    let want = small_icon_px();
    set.iter().find(|(px, _)| *px >= want).or_else(|| set.last()).map(|(_, b)| *b).unwrap_or(&[])
}

fn look(s: &core::Status) -> Look {
    match s.state {
        "on" => match &s.health {
            Some(h) if h.ok => Look::Ok,
            Some(_) => Look::Bad,
            None => Look::Warn,
        },
        "connecting" | "reconnecting" => Look::Warn,
        "error" => Look::Bad,
        _ => Look::Idle,
    }
}

fn label(s: &core::Status) -> &'static str {
    match (s.state, s.health.as_ref().map(|h| h.ok)) {
        ("on", Some(false)) => "Сервер не отвечает",
        ("on", _) => "Подключено",
        ("connecting", _) => "Подключаюсь…",
        ("reconnecting", _) => "Переподключаюсь…",
        ("error", _) => "Не подключено",
        _ => "Не подключено",
    }
}

fn tooltip(s: &core::Status) -> String {
    let mut t = format!("kl!ck — {}", label(s).to_lowercase());
    if s.state == "on" {
        if let Some(n) = &s.server {
            t.push_str(&format!(" · {n}"));
        }
        if let Some(ms) = s.health.as_ref().and_then(|h| h.ms) {
            t.push_str(&format!(" · {ms} мс"));
        }
    }
    t.chars().take(120).collect()
}

#[derive(Serialize)]
pub struct ServerRow {
    name: String,
    active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuState {
    look: &'static str,
    label: &'static str,
    state: &'static str,
    profile: Option<String>,
    server: Option<String>,
    ms: Option<u32>,
    mode: &'static str,
    servers: Vec<ServerRow>,
    version: String,
}

static ANCHOR: Lazy<Mutex<Option<(f64, f64, f64, f64)>>> = Lazy::new(|| Mutex::new(None));
/// Клик по иконке при открытом меню сначала снимает с него фокус (меню
/// прячется), а потом приходит сам клик — без отсечки меню открылось бы снова.
static LAST_HIDE: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));

fn open_popup(app: &AppHandle, rect: tauri::Rect) {
    if let Some(t) = *LAST_HIDE.lock().unwrap() {
        if t.elapsed() < Duration::from_millis(300) {
            return;
        }
    }
    let scale = app.primary_monitor().ok().flatten().map(|m| m.scale_factor()).unwrap_or(1.0);
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    *ANCHOR.lock().unwrap() = Some((pos.x, pos.y, size.width, size.height));
    if app.get_webview_window(POPUP).is_some() {
        let _ = app.emit_to(POPUP, "tray-menu-open", ());
        return;
    }
    let built = tauri::WebviewWindowBuilder::new(app, POPUP, tauri::WebviewUrl::App("tray-menu.html".into()))
        .title("kl!ck")
        .inner_size(300.0, 420.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .build();
    if built.is_err() {
        show_window(app);
    }
}

pub fn hide_popup(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(POPUP) {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            *LAST_HIDE.lock().unwrap() = Some(Instant::now());
        }
    }
}

#[tauri::command]
pub fn tray_menu_state(app: AppHandle) -> TrayMenuState {
    let s = core::status();
    let st = app.state::<AppState>();
    let mode = st.settings.lock().unwrap().mode;
    let active = st.active_profile();
    let chosen = active.as_ref().and_then(|p| p.active_proxy()).and_then(|v| v.get("name")).and_then(Value::as_str).map(String::from);
    let servers = active
        .as_ref()
        .map(|p| {
            p.proxies
                .iter()
                .filter_map(|v| v.get("name").and_then(Value::as_str))
                .take(15)
                .map(|n| ServerRow { name: n.to_string(), active: Some(n) == chosen.as_deref() })
                .collect()
        })
        .unwrap_or_default();
    TrayMenuState {
        look: look(&s).key(),
        label: label(&s),
        state: s.state,
        profile: active.as_ref().map(|p| p.name.clone()),
        server: if s.state == "on" { s.server.clone() } else { chosen },
        ms: s.health.as_ref().and_then(|h| h.ms),
        mode: match mode {
            Mode::Tun => "VPN (TUN)",
            Mode::Proxy => "Proxy",
            Mode::Sysproxy => "Системный proxy",
        },
        servers,
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn tray_menu_ready(app: AppHandle, width: f64, height: f64) {
    let Some(w) = app.get_webview_window(POPUP) else { return };
    let Some((ax, ay, aw, ah)) = *ANCHOR.lock().unwrap() else { return };
    let (cx, cy) = (ax + aw / 2.0, ay + ah / 2.0);
    let mon = app.monitor_from_point(cx, cy).ok().flatten().or_else(|| app.primary_monitor().ok().flatten());
    let scale = mon.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let (wx, wy, ww, wh) = mon
        .as_ref()
        .map(|m| {
            let a = m.work_area();
            (a.position.x as f64, a.position.y as f64, a.size.width as f64, a.size.height as f64)
        })
        .unwrap_or((0.0, 0.0, 1920.0, 1080.0));
    let pw = (width * scale).round();
    let ph = (height * scale).round();
    let gap = 2.0 * scale;
    let y = if cy < wy + wh / 2.0 { ay + ah + gap } else { ay.min(wy + wh) - ph - gap };
    let y = y.clamp(wy, (wy + wh - ph).max(wy));
    let x = (cx - pw / 2.0).clamp(wx, (wx + ww - pw).max(wx));
    let _ = w.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
    let _ = w.set_size(tauri::PhysicalSize::new(pw as u32, ph as u32));
    if !w.is_visible().unwrap_or(false) {
        let _ = w.show();
    }
    let _ = w.set_focus();
}

#[tauri::command]
pub fn tray_menu_hide(app: AppHandle) {
    hide_popup(&app);
}

#[tauri::command]
pub fn tray_menu_action(app: AppHandle, id: String) {
    hide_popup(&app);
    match id.as_str() {
        "open" => show_window(&app),
        "toggle" => {
            std::thread::spawn(move || {
                if matches!(core::status().state, "on" | "connecting" | "reconnecting") {
                    core::disconnect(&app);
                } else if let Err(e) = core::connect(&app) {
                    crate::notify::send(&app, "Не удалось подключиться", &e, true);
                }
            });
        }
        "quit" => {
            std::thread::spawn(move || {
                core::shutdown(&app);
                app.exit(0);
            });
        }
        other => {
            if let Some(server) = other.strip_prefix("server:") {
                let server = server.to_string();
                std::thread::spawn(move || {
                    let pid = app.state::<AppState>().active_profile().map(|p| p.id);
                    if let Some(pid) = pid {
                        if let Err(e) = core::select_server(&app, &pid, &server) {
                            crate::notify::send(&app, "Не удалось переключить сервер", &e, true);
                        }
                    }
                    let _ = app.emit("profiles-changed", ());
                });
            }
        }
    }
}

static LAST_ICON: Lazy<Mutex<Option<Look>>> = Lazy::new(|| Mutex::new(None));

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let s = core::status();
    let l = look(&s);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::image::Image::from_bytes(icon_bytes(l))?)
        .tooltip(tooltip(&s))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, rect, .. } = event {
                match button {
                    MouseButton::Left => show_window(tray.app_handle()),
                    MouseButton::Right => open_popup(tray.app_handle(), rect),
                    _ => {}
                }
            }
        })
        .build(app)?;
    *LAST_ICON.lock().unwrap() = Some(l);
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let s = core::status();
    let _ = tray.set_tooltip(Some(tooltip(&s)));
    let l = look(&s);
    let mut last = LAST_ICON.lock().unwrap();
    if *last != Some(l) {
        if let Ok(img) = tauri::image::Image::from_bytes(icon_bytes(l)) {
            let _ = tray.set_icon(Some(img));
            *last = Some(l);
        }
    }
    drop(last);
    if let Some(w) = app.get_webview_window(POPUP) {
        if w.is_visible().unwrap_or(false) {
            let _ = app.emit_to(POPUP, "tray-menu-update", ());
        }
    }
}
