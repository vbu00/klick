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
use crate::state::{AppState, DefaultRoute, Mode, RouteMode};

pub const TRAY_ID: &str = "klick-tray";
pub const POPUP: &str = "tray-menu";

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Состояние трея — по цветам логотипа-клавиши (docs/logo).
#[derive(PartialEq, Clone, Copy)]
enum Look {
    /// Зелёная: подключено.
    Ok,
    /// Синяя, мигает: подключаюсь.
    Busy,
    /// Оранжевая: туннель есть, но сервер не отвечает.
    Warn,
    /// Красная: не подключилось.
    Bad,
    /// Серая: выключено.
    Idle,
}

impl Look {
    fn key(self) -> &'static str {
        match self {
            Look::Ok => "ok",
            Look::Busy => "busy",
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
const ICONS_BUSY: [(i32, &[u8]); 8] = icon_set!("busy");
const ICONS_BAD: [(i32, &[u8]); 8] = icon_set!("bad");
const ICONS_IDLE: [(i32, &[u8]); 8] = icon_set!("idle");
/// Второй кадр мигания, пока идёт подключение.
const ICONS_BUSY2: [(i32, &[u8]); 8] = icon_set!("busy2");

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
    pick(match l {
        Look::Ok => &ICONS_OK,
        Look::Busy => &ICONS_BUSY,
        Look::Warn => &ICONS_WARN,
        Look::Bad => &ICONS_BAD,
        Look::Idle => &ICONS_IDLE,
    })
}

fn pick(set: &[(i32, &'static [u8])]) -> &'static [u8] {
    let want = small_icon_px();
    set.iter().find(|(px, _)| *px >= want).or_else(|| set.last()).map(|(_, b)| *b).unwrap_or(&[])
}

fn look(s: &core::Status) -> Look {
    match s.state {
        "on" => match &s.health {
            Some(h) if !h.ok => Look::Warn,
            _ => Look::Ok,
        },
        "connecting" | "reconnecting" => Look::Busy,
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
    /// Последний замер: поля нет — не мерили, null — не ответил.
    #[serde(skip_serializing_if = "Option::is_none")]
    ms: Option<Option<u32>>,
}

#[derive(Serialize)]
pub struct ProfileRow {
    id: String,
    name: String,
    active: bool,
    live: bool,
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
    route_mode: &'static str,
    since: Option<u64>,
    traffic: core::Traffic,
    profiles: Vec<ProfileRow>,
    servers: Vec<ServerRow>,
    kill_switch: bool,
    ks_apps: usize,
    ks_sites: usize,
    version: String,
    mihomo: Option<String>,
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
        .inner_size(340.0, 520.0)
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
    let settings = st.settings.lock().unwrap().clone();
    let mode = settings.mode;
    let active = st.active_profile();
    let chosen = active.as_ref().and_then(|p| p.active_proxy()).and_then(|v| v.get("name")).and_then(Value::as_str).map(String::from);
    let live_id = matches!(s.state, "on" | "connecting" | "reconnecting").then(|| s.profile_id.clone()).flatten();
    let pings = active.as_ref().map(|p| core::cached_pings(&p.id)).unwrap_or_default();
    let servers = active
        .as_ref()
        .map(|p| {
            p.proxies
                .iter()
                .filter_map(|v| v.get("name").and_then(Value::as_str))
                .take(30)
                .map(|n| ServerRow { name: n.to_string(), active: Some(n) == chosen.as_deref(), ms: pings.get(n).copied() })
                .collect()
        })
        .unwrap_or_default();
    let profiles = st
        .vault
        .lock()
        .unwrap()
        .profiles
        .iter()
        .map(|p| ProfileRow {
            id: p.id.clone(),
            name: p.name.clone(),
            active: active.as_ref().map(|a| a.id == p.id).unwrap_or(false),
            live: live_id.as_deref() == Some(p.id.as_str()),
        })
        .collect();
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
        route_mode: match settings.route_mode {
            RouteMode::Rule if settings.default_route == DefaultRoute::Direct => "только выбранное",
            RouteMode::Rule => "по правилам",
            RouteMode::Global => "всё через VPN",
            RouteMode::Direct => "всё напрямую",
        },
        since: s.since,
        traffic: core::traffic(),
        profiles,
        servers,
        kill_switch: settings.kill_switch,
        ks_apps: settings.ks_apps.iter().filter(|a| a.on).count(),
        ks_sites: settings.ks_sites.iter().filter(|a| a.on).count(),
        version: app.package_info().version.to_string(),
        mihomo: crate::commands::mihomo_version(&app),
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
    // Меню остаётся открытым: подключение, смена сервера и Kill Switch видны
    // в нём же. Прячется только на «Открыть» и «Выйти».
    match id.as_str() {
        "open" => {
            hide_popup(&app);
            show_window(&app);
        }
        "toggle" => {
            std::thread::spawn(move || {
                if matches!(core::status().state, "on" | "connecting" | "reconnecting") {
                    core::disconnect(&app);
                } else if let Err(e) = core::connect(&app) {
                    crate::notify::send(&app, "Не удалось подключиться", &e, true);
                }
            });
        }
        "ping" => {
            std::thread::spawn(move || {
                let Some(pid) = app.state::<AppState>().active_profile().map(|p| p.id) else { return };
                if let Ok(m) = core::ping_profile(&app, &pid) {
                    let ok = m.values().filter(|v| v.is_some()).count();
                    core::note("INFO", &format!("Проверка задержки: ответили {ok} из {}", m.len()));
                    let _ = app.emit("pings", &m);
                }
                refresh(&app);
            });
        }
        "killswitch" => {
            std::thread::spawn(move || {
                let st = app.state::<AppState>();
                let s = {
                    let mut s = st.settings.lock().unwrap();
                    s.kill_switch = !s.kill_switch;
                    s.clone()
                };
                st.save_settings();
                core::ks_apply(&app, core::ks_engaged(&s, core::is_on()), &s.ks_apps, &s.ks_sites);
                // Защищённое Kill Switch — в правилах «только через VPN».
                if let Err(e) = core::apply_config(&app) {
                    core::note("WARN", &e);
                }
                let _ = app.emit("profiles-changed", ());
                refresh(&app);
            });
        }
        "quit" => {
            hide_popup(&app);
            std::thread::spawn(move || {
                core::shutdown(&app);
                app.exit(0);
            });
        }
        other => {
            if let Some(id) = other.strip_prefix("profile:") {
                let id = id.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = core::select_profile(&app, &id) {
                        crate::notify::send(&app, "Не удалось переключить подключение", &e, true);
                    }
                    let _ = app.emit("profiles-changed", ());
                    refresh(&app);
                });
            } else if let Some(server) = other.strip_prefix("server:") {
                let server = server.to_string();
                std::thread::spawn(move || {
                    let pid = app.state::<AppState>().active_profile().map(|p| p.id);
                    if let Some(pid) = pid {
                        if let Err(e) = core::select_server(&app, &pid, &server) {
                            crate::notify::send(&app, "Не удалось переключить сервер", &e, true);
                        }
                    }
                    let _ = app.emit("profiles-changed", ());
                    refresh(&app);
                });
            }
        }
    }
}

static LAST_ICON: Lazy<Mutex<Option<Look>>> = Lazy::new(|| Mutex::new(None));
/// Идёт ли мигание «подключаюсь» — им занят отдельный поток.
static BLINK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn busy(s: &core::Status) -> bool {
    matches!(s.state, "connecting" | "reconnecting")
}

fn set_icon(app: &AppHandle, bytes: &[u8]) {
    if let (Some(tray), Ok(img)) = (app.tray_by_id(TRAY_ID), tauri::image::Image::from_bytes(bytes)) {
        let _ = tray.set_icon(Some(img));
    }
}

/// Пока идёт подключение, иконка мигает; как только оно закончилось —
/// встаёт иконка итогового состояния.
fn start_blink(app: &AppHandle) {
    use std::sync::atomic::Ordering;
    if BLINK.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut dim = false;
        loop {
            std::thread::sleep(Duration::from_millis(450));
            let s = core::status();
            if !busy(&s) {
                let l = look(&s);
                set_icon(&app, icon_bytes(l));
                *LAST_ICON.lock().unwrap() = Some(l);
                BLINK.store(false, Ordering::SeqCst);
                return;
            }
            dim = !dim;
            set_icon(&app, if dim { pick(&ICONS_BUSY2) } else { icon_bytes(Look::Busy) });
        }
    });
}

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
    if busy(&s) {
        start_blink(app);
    }
    let mut last = LAST_ICON.lock().unwrap();
    if !BLINK.load(std::sync::atomic::Ordering::SeqCst) && *last != Some(l) {
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
