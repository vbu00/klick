//! Значок в трее по состоянию — пять картинок из макета: подключено — зелёный; подключаюсь,
//! переподключаюсь и «сервер не отвечает» — оранжевый; ошибка и «служба не отвечает» — красный;
//! выключено — синий логотип; подключений ещё нет — серый. Размер — под масштаб экрана, чтобы
//! значок был чётким. В полноэкранных играх и в «Не беспокоить» Windows прячет уведомления —
//! значок остаётся на виду.

use serde_json::Value;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::AppHandle;

/// Картинки одного значка во всех размерах (значки нарисованы в `icons/tray`, SVG — у Димы).
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

const DEFAULT: [(u32, &[u8]); 7] = sizes!("default");
const SUCCESS: [(u32, &[u8]); 7] = sizes!("success");
const WARNING: [(u32, &[u8]); 7] = sizes!("warning");
const ERROR: [(u32, &[u8]); 7] = sizes!("error");
const NO_CONNECTION: [(u32, &[u8]); 7] = sizes!("no-connection");

/// Какой значок у состояния службы. `empty` — подключений ещё нет.
fn kind(vpn: &str, empty: bool) -> &'static str {
    match vpn {
        "connected" => "success",
        "connecting" | "reconnecting" | "server_down" => "warning",
        "error" => "error",
        _ if empty => "no-connection",
        _ => "default",
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
        "success" => &SUCCESS,
        "warning" => &WARNING,
        "error" => &ERROR,
        "no-connection" => &NO_CONNECTION,
        _ => &DEFAULT,
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
    icon("default", wanted_size())
}

/// Новое состояние службы.
pub fn update(app: &AppHandle, state: &Value) {
    let vpn = state["vpn"].as_str().unwrap_or("off");
    set(app, kind(vpn, state["connection"].is_null()), tooltip(state));
}

/// Служба не отвечает: красный значок и честная подсказка.
pub fn service_down(app: &AppHandle) {
    set(app, "error", "kl!ck — служба не отвечает".into());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_has_a_readable_icon() {
        for k in ["default", "success", "warning", "error", "no-connection"] {
            for size in [16, 20, 24, 32, 40, 48, 64, 80] {
                let img = icon(k, size).expect("значок");
                assert_eq!(img.width(), size.min(64), "{k} {size}");
                assert!(img.rgba().chunks_exact(4).any(|p| p[3] == 0), "{k}: прозрачный фон");
            }
        }
    }

    #[test]
    fn states_map_to_icons() {
        assert_eq!(kind("connected", false), "success");
        assert_eq!(kind("server_down", false), "warning");
        assert_eq!(kind("reconnecting", false), "warning");
        assert_eq!(kind("error", false), "error");
        assert_eq!(kind("off", false), "default");
        assert_eq!(kind("off", true), "no-connection");
    }
}
