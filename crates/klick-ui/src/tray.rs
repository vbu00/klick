//! Значок в трее по состоянию — клавиша kl!ck пяти цветов: подключено — зелёный; подключаюсь и
//! переподключаюсь — синий (цвет логотипа); сервер не отвечает — оранжевый; ошибка и «служба не
//! отвечает» — красный; выключено и подключений ещё нет — серый. Значки нарисованы крупно, на всю
//! клетку трея; размер — под масштаб экрана, чтобы значок был чётким. В полноэкранных играх и в
//! «Не беспокоить» Windows прячет уведомления — значок остаётся на виду.

use serde_json::Value;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::AppHandle;

/// Картинки одного значка во всех размерах (`icons/tray`).
macro_rules! sizes {
    ($name:literal) => {
        [
            (16, include_bytes!(concat!("../icons/tray/", $name, "-16.png")) as &[u8]),
            (20, include_bytes!(concat!("../icons/tray/", $name, "-20.png"))),
            (24, include_bytes!(concat!("../icons/tray/", $name, "-24.png"))),
            (32, include_bytes!(concat!("../icons/tray/", $name, "-32.png"))),
            (40, include_bytes!(concat!("../icons/tray/", $name, "-40.png"))),
            (48, include_bytes!(concat!("../icons/tray/", $name, "-48.png"))),
            (64, include_bytes!(concat!("../icons/tray/", $name, "-64.png"))),
        ]
    };
}

const OK: [(u32, &[u8]); 7] = sizes!("ok");
const BUSY: [(u32, &[u8]); 7] = sizes!("busy");
const WARN: [(u32, &[u8]); 7] = sizes!("warn");
const BAD: [(u32, &[u8]); 7] = sizes!("bad");
const IDLE: [(u32, &[u8]); 7] = sizes!("idle");

/// Какой значок у состояния службы. `_empty` — подключений ещё нет (тоже серый).
fn kind(vpn: &str, _empty: bool) -> &'static str {
    match vpn {
        "connected" => "ok",
        "connecting" | "reconnecting" => "busy",
        "server_down" => "warn",
        "error" => "bad",
        _ => "idle",
    }
}

/// Размер значка в области уведомлений: 16 точек на 100 % масштаба.
fn wanted_size() -> u32 {
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForSystem() };
    let dpi = if dpi == 0 { 96 } else { dpi };
    (16 * dpi).div_ceil(96)
}

fn icon(kind: &str, size: u32) -> Option<Image<'static>> {
    let set = match kind {
        "ok" => &OK,
        "busy" => &BUSY,
        "warn" => &WARN,
        "bad" => &BAD,
        _ => &IDLE,
    };
    let bytes = set.iter().find(|(s, _)| *s >= size).unwrap_or(&set[set.len() - 1]).1;
    Image::from_bytes(bytes).ok()
}

fn tooltip(state: &Value) -> String {
    let vpn = state["vpn"].as_str().unwrap_or("off");
    let server = state["server"].as_str().unwrap_or_default();
    let status = match vpn {
        "connected" => "подключено".to_string(),
        "connecting" => "подключаюсь…".to_string(),
        "reconnecting" => match (state["attempt"][0].as_u64(), state["attempt"][1].as_u64()) {
            (Some(a), Some(b)) => format!("переподключаюсь… {a} из {b}"),
            _ => "переподключаюсь…".to_string(),
        },
        "server_down" => "сервер не отвечает".to_string(),
        "error" => "VPN не включился".to_string(),
        _ if state["connection"].is_null() => "нет подключений".to_string(),
        _ => "выключено".to_string(),
    };
    if server.is_empty() || vpn == "off" || vpn == "error" {
        format!("kl!ck — {status}")
    } else {
        format!("kl!ck — {status} · {server}")
    }
}

static LAST: Mutex<String> = Mutex::new(String::new());

fn set(app: &AppHandle, kind: &str, tip: String) {
    let size = wanted_size();
    let key = format!("{kind}|{size}|{tip}");
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if *last == key {
            return;
        }
        *last = key;
    }
    if let Some(tray) = app.tray_by_id("main") {
        if let Some(img) = icon(kind, size) {
            let _ = tray.set_icon(Some(img));
        }
        let _ = tray.set_tooltip(Some(tip));
    }
}

/// Значок при запуске, пока служба не прислала состояние.
pub fn initial() -> Option<Image<'static>> {
    icon("idle", wanted_size())
}

/// Новое состояние службы.
pub fn update(app: &AppHandle, state: &Value) {
    let vpn = state["vpn"].as_str().unwrap_or("off");
    set(app, kind(vpn, state["connection"].is_null()), tooltip(state));
}

/// Служба не отвечает: красный значок и честная подсказка.
pub fn service_down(app: &AppHandle) {
    set(app, "bad", "kl!ck — служба не отвечает".into());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_has_a_readable_icon() {
        for k in ["ok", "busy", "warn", "bad", "idle"] {
            for size in [16, 20, 24, 32, 40, 48, 64, 80] {
                let img = icon(k, size).expect("значок");
                assert_eq!(img.width(), size.min(64), "{k} {size}");
                assert!(img.rgba().chunks_exact(4).any(|p| p[3] == 0), "{k}: прозрачный фон");
            }
        }
    }

    #[test]
    fn states_map_to_icons() {
        assert_eq!(kind("connected", false), "ok");
        assert_eq!(kind("connecting", false), "busy");
        assert_eq!(kind("reconnecting", false), "busy");
        assert_eq!(kind("server_down", false), "warn");
        assert_eq!(kind("error", false), "bad");
        assert_eq!(kind("off", false), "idle");
        assert_eq!(kind("off", true), "idle");
    }
}
