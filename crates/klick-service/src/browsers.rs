//! Браузеры, чей прокси перехватило расширение (VPN-расширения и подобные) или свои настройки
//! Firefox. В режиме «Системный прокси» такой браузер идёт мимо kl!ck: прокси он берёт не у
//! Windows, а у расширения. Служба читает файлы профилей всех пользователей компьютера.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Кто перехватил прокси и в каком браузере.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hijack {
    pub browser: String,
    /// Название расширения или «свои настройки прокси».
    pub what: String,
}

/// Браузеры на Chromium: папка профилей относительно папки пользователя.
const CHROMIUM: [(&str, &str); 8] = [
    (r"AppData\Local\Google\Chrome\User Data", "Chrome"),
    (r"AppData\Local\Microsoft\Edge\User Data", "Edge"),
    (r"AppData\Local\Yandex\YandexBrowser\User Data", "Яндекс Браузер"),
    (r"AppData\Local\BraveSoftware\Brave-Browser\User Data", "Brave"),
    (r"AppData\Local\Vivaldi\User Data", "Vivaldi"),
    (r"AppData\Local\Chromium\User Data", "Chromium"),
    (r"AppData\Roaming\Opera Software\Opera Stable", "Opera"),
    (r"AppData\Roaming\Opera Software\Opera GX Stable", "Opera GX"),
];
const FIREFOX: &str = r"AppData\Roaming\Mozilla\Firefox\Profiles";

/// Все пользователи компьютера.
pub fn scan() -> Vec<Hijack> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let users = PathBuf::from(format!(r"{drive}\Users"));
    let mut out = Vec::new();
    for home in fs::read_dir(users).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        for h in scan_user(&home) {
            if !out.contains(&h) {
                out.push(h);
            }
        }
    }
    out
}

pub fn scan_user(home: &Path) -> Vec<Hijack> {
    let mut out = Vec::new();
    for (rel, browser) in CHROMIUM {
        for profile in chromium_profiles(&home.join(rel)) {
            for what in chromium_profile(&profile) {
                out.push(Hijack { browser: browser.into(), what });
            }
        }
    }
    for profile in fs::read_dir(home.join(FIREFOX)).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        for what in firefox_profile(&profile) {
            out.push(Hijack { browser: "Firefox".into(), what });
        }
    }
    out
}

/// Папки профилей: `Default`, `Profile 1`…; у Opera профиль — сама папка.
fn chromium_profiles(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if root.join("Preferences").is_file() {
        out.push(root.to_path_buf());
    }
    for dir in fs::read_dir(root).into_iter().flatten().flatten().map(|e| e.path()) {
        if dir.join("Preferences").is_file() {
            out.push(dir);
        }
    }
    out
}

fn read_json(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Расширения профиля, которые сейчас задают прокси браузера (не «как в системе») и включены.
fn chromium_profile(profile: &Path) -> Vec<String> {
    // Настройки расширений лежат в «Secure Preferences», у старых версий — в «Preferences».
    let files: Vec<Value> = ["Secure Preferences", "Preferences"].iter().filter_map(|f| read_json(&profile.join(f))).collect();
    let mut ids: Vec<String> = Vec::new();
    for f in &files {
        if let Some(Value::Object(m)) = f.pointer("/extensions/settings") {
            ids.extend(m.keys().filter(|k| !ids.contains(k)).cloned().collect::<Vec<_>>());
        }
    }
    let mut out = Vec::new();
    for id in ids {
        let entries: Vec<&Value> = files.iter().filter_map(|f| f.pointer(&format!("/extensions/settings/{id}"))).collect();
        let sets_proxy = entries.iter().any(|s| sets_proxy(s));
        let disabled = entries.iter().any(|s| disabled(s));
        if sets_proxy && !disabled {
            let name = entries.iter().find_map(|s| s.pointer("/manifest/name").and_then(Value::as_str).filter(|n| !n.starts_with("__MSG_")).map(str::to_string));
            out.push(name.unwrap_or_else(|| extension_name(&profile.join("Extensions").join(&id)).unwrap_or(id)));
        }
    }
    out
}

/// Расширение выставило прокси: режим «напрямую», PAC, свой сервер или автоопределение.
/// Режим `system` — браузер берёт прокси у Windows, это не мешает.
fn sets_proxy(s: &Value) -> bool {
    ["preferences", "regular_only_preferences"].iter().any(|key| {
        s.get(key).and_then(|p| p.get("proxy")).is_some_and(|p| p.get("mode").and_then(Value::as_str) != Some("system"))
    })
}

fn disabled(s: &Value) -> bool {
    let reasons = match s.get("disable_reasons") {
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Number(n)) => n.as_u64() != Some(0),
        _ => false,
    };
    reasons || s.get("state").and_then(Value::as_u64) == Some(0)
}

/// Название из manifest.json последней версии; `__MSG_name__` — из `_locales`.
fn extension_name(dir: &Path) -> Option<String> {
    let mut versions: Vec<PathBuf> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.join("manifest.json").is_file()).collect();
    versions.sort();
    let ver = versions.pop()?;
    let manifest = read_json(&ver.join("manifest.json"))?;
    let name = manifest.get("name").and_then(Value::as_str)?;
    let Some(key) = name.strip_prefix("__MSG_").and_then(|n| n.strip_suffix("__")) else { return Some(name.to_string()) };
    let default = manifest.get("default_locale").and_then(Value::as_str).unwrap_or("en");
    for locale in ["ru", default, "en"] {
        if let Some(Value::Object(m)) = read_json(&ver.join("_locales").join(locale).join("messages.json")) {
            if let Some(msg) = m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).and_then(|(_, v)| v.get("message")).and_then(Value::as_str) {
                return Some(msg.to_string());
            }
        }
    }
    None
}

/// Firefox: расширение, управляющее прокси, или свои настройки прокси вместо системных.
fn firefox_profile(profile: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let addons = read_json(&profile.join("extensions.json"));
    if let Some(Value::Array(list)) = read_json(&profile.join("extension-settings.json")).and_then(|v| v.pointer("/prefs/proxy.settings/precedenceList").cloned()) {
        for item in list.iter().filter(|i| i.get("enabled").and_then(Value::as_bool).unwrap_or(false)) {
            let Some(id) = item.get("id").and_then(Value::as_str) else { continue };
            let addon = addons.as_ref().and_then(|a| a.get("addons")).and_then(Value::as_array).and_then(|l| l.iter().find(|x| x.get("id").and_then(Value::as_str) == Some(id)));
            if addon.is_some_and(|a| a.get("active").and_then(Value::as_bool) == Some(false)) {
                continue;
            }
            let name = addon.and_then(|a| a.pointer("/defaultLocale/name")).and_then(Value::as_str).unwrap_or(id);
            out.push(name.to_string());
        }
    }
    // network.proxy.type: 5 (по умолчанию) — системный прокси; 0 — без прокси; 1, 2, 4 — свои.
    if out.is_empty() {
        let prefs = fs::read_to_string(profile.join("prefs.js")).unwrap_or_default();
        let kind = prefs.lines().find_map(|l| l.strip_prefix("user_pref(\"network.proxy.type\",").map(|r| r.trim().trim_end_matches(");").trim().to_string()));
        if kind.is_some_and(|k| k != "5") {
            out.push("свои настройки прокси".into());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("klick-browsers-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn chrome_profile(home: &Path, settings: Value) -> PathBuf {
        let p = home.join(r"AppData\Local\Google\Chrome\User Data\Default");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("Preferences"), "{}").unwrap();
        fs::write(p.join("Secure Preferences"), json!({ "extensions": { "settings": settings } }).to_string()).unwrap();
        p
    }

    #[test]
    fn extension_with_pac_is_found_by_localized_name() {
        let home = tmp("pac");
        let p = chrome_profile(&home, json!({ "abc": { "preferences": { "proxy": { "mode": "pac_script", "pac_url": "data:..." } } } }));
        let ver = p.join(r"Extensions\abc\5.0.23_0");
        fs::create_dir_all(ver.join(r"_locales\en")).unwrap();
        fs::write(ver.join("manifest.json"), json!({ "name": "__MSG_name__", "default_locale": "en" }).to_string()).unwrap();
        fs::write(ver.join(r"_locales\en\messages.json"), json!({ "NAME": { "message": "Touch VPN" } }).to_string()).unwrap();
        assert_eq!(scan_user(&home), vec![Hijack { browser: "Chrome".into(), what: "Touch VPN".into() }]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn system_mode_disabled_and_plain_extensions_are_ignored() {
        let home = tmp("quiet");
        chrome_profile(
            &home,
            json!({
                "sys": { "preferences": { "proxy": { "mode": "system" } } },
                "off": { "preferences": { "proxy": { "mode": "fixed_servers" } }, "disable_reasons": [1] },
                "plain": { "preferences": { "net.network_prediction_options": 2 } }
            }),
        );
        assert!(scan_user(&home).is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    /// Профили на этом компьютере: `cargo test -p klick-service live_browsers -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_browsers() {
        for h in scan() {
            println!("{} в {}", h.what, h.browser);
        }
    }

    #[test]
    fn firefox_extension_and_manual_proxy() {
        let home = tmp("ff");
        let a = home.join(r"AppData\Roaming\Mozilla\Firefox\Profiles\a.default");
        let b = home.join(r"AppData\Roaming\Mozilla\Firefox\Profiles\b.work");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("extension-settings.json"), json!({ "prefs": { "proxy.settings": { "precedenceList": [{ "id": "vpn@x", "enabled": true }] } } }).to_string()).unwrap();
        fs::write(a.join("extensions.json"), json!({ "addons": [{ "id": "vpn@x", "active": true, "defaultLocale": { "name": "Browsec" } }] }).to_string()).unwrap();
        fs::write(b.join("prefs.js"), "user_pref(\"network.proxy.type\", 1);\n").unwrap();
        let mut found: Vec<String> = scan_user(&home).into_iter().map(|h| h.what).collect();
        found.sort();
        assert_eq!(found, vec!["Browsec".to_string(), "свои настройки прокси".to_string()]);
        let _ = fs::remove_dir_all(&home);
    }
}
