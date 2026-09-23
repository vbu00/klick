//! Настройки и подключения.
//!
//! Настройки — settings.json. Подключения (подписки и одиночные
//! конфигурации) — profiles.dat, зашифрованный DPAPI текущего пользователя:
//! там ссылки подписок, UUID и пароли серверов.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Как трафик попадает в mihomo.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Смешанный прокси 127.0.0.1:порт — программы настраиваются вручную.
    Proxy,
    /// Тот же прокси, прописанный в Windows.
    Sysproxy,
    /// Виртуальный адаптер: весь трафик всех программ.
    #[default]
    Tun,
}

/// Режим маршрутизации самого mihomo.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    #[default]
    Rule,
    Global,
    Direct,
}

impl RouteMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteMode::Rule => "rule",
            RouteMode::Global => "global",
            RouteMode::Direct => "direct",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Proxy,
    Direct,
    Block,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SiteRule {
    pub pattern: String,
    pub action: Action,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppRule {
    pub name: String,
    pub exe: String,
    pub action: Action,
}

/// Программа под защитой Kill Switch. Правилу брандмауэра нужен полный
/// путь к exe — одного имени мало.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KsApp {
    pub name: String,
    pub exe: String,
    pub path: String,
    pub on: bool,
}

/// Сайт под защитой Kill Switch: пока VPN выключен, его адреса закрыты
/// для всех программ.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KsSite {
    pub pattern: String,
    pub on: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Presets {
    /// Домены .ru, .su и .рф — напрямую.
    pub ru: bool,
    /// Локальная сеть — напрямую.
    pub lan: bool,
    /// Российские IP по базе GeoIP — напрямую.
    pub geoip: bool,
}

impl Default for Presets {
    fn default() -> Self {
        Presets { ru: true, lan: true, geoip: false }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub active_profile: Option<String>,
    pub mode: Mode,
    pub route_mode: RouteMode,
    pub proxy_port: u16,
    pub presets: Presets,
    pub sites: Vec<SiteRule>,
    pub apps: Vec<AppRule>,
    pub kill_switch: bool,
    pub ks_apps: Vec<KsApp>,
    pub ks_sites: Vec<KsSite>,
    pub auto_update: bool,
    pub notify_drops: bool,
    /// Подключаться сразу после запуска (в том числе при входе в Windows).
    pub connect_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            active_profile: None,
            mode: Mode::Tun,
            route_mode: RouteMode::Rule,
            proxy_port: 7890,
            presets: Presets::default(),
            sites: vec![],
            apps: vec![],
            kill_switch: false,
            ks_apps: vec![],
            ks_sites: vec![],
            auto_update: true,
            notify_drops: true,
            connect_on_launch: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Подписка по https://: серверы, лимит трафика, срок.
    Sub,
    /// Одиночная ссылка vless:// и т. п.
    Single,
    /// Импортированный файл mihomo/Clash.
    File,
}

/// Из заголовка subscription-userinfo, в байтах и unix-времени.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubInfo {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Прокси в формате mihomo (как в YAML-конфиге), имена уникальны.
    pub proxies: Vec<Value>,
    /// Выбранный сервер (имя прокси).
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub info: Option<SubInfo>,
    #[serde(default)]
    pub updated_at: Option<u64>,
    /// Как часто обновлять, часов (из profile-update-interval).
    #[serde(default)]
    pub update_hours: Option<u32>,
}

impl Profile {
    pub fn active_proxy(&self) -> Option<&Value> {
        let name = self.active.as_deref();
        self.proxies
            .iter()
            .find(|p| p.get("name").and_then(Value::as_str) == name)
            .or_else(|| self.proxies.first())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Vault {
    pub profiles: Vec<Profile>,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub vault: Mutex<Vault>,
    pub dir: PathBuf,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// %LOCALAPPDATA%\com.vbu00.klick — локальная папка: profiles.dat
/// зашифрован под эту машину и учётку.
pub fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_local_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("klick"))
}

impl AppState {
    pub fn load(app: &AppHandle) -> AppState {
        let dir = data_dir(app);
        let _ = std::fs::create_dir_all(&dir);
        let settings = std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let vault = std::fs::read(dir.join("profiles.dat"))
            .ok()
            .and_then(|enc| dpapi::unprotect(&enc).ok())
            .and_then(|plain| serde_json::from_slice(&plain).ok())
            .unwrap_or_default();
        AppState { settings: Mutex::new(settings), vault: Mutex::new(vault), dir }
    }

    pub fn save_settings(&self) {
        let s = self.settings.lock().unwrap().clone();
        if let Ok(json) = serde_json::to_string_pretty(&s) {
            write_atomic(&self.dir.join("settings.json"), json.as_bytes());
        }
    }

    pub fn save_vault(&self) -> Result<(), String> {
        let v = self.vault.lock().unwrap().clone();
        let json = serde_json::to_vec(&v).map_err(|e| e.to_string())?;
        write_atomic(&self.dir.join("profiles.dat"), &dpapi::protect(&json)?);
        Ok(())
    }

    /// Активное подключение, а если оно пропало — первое.
    pub fn active_profile(&self) -> Option<Profile> {
        let id = self.settings.lock().unwrap().active_profile.clone();
        let v = self.vault.lock().unwrap();
        id.and_then(|id| v.profiles.iter().find(|p| p.id == id).cloned()).or_else(|| v.profiles.first().cloned())
    }
}

/// Запись через временный файл: оборванная запись не должна оставлять
/// пустой файл и сбрасывать всё по умолчанию.
pub fn write_atomic(path: &std::path::Path, data: &[u8]) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, data).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

pub mod dpapi {
    //! DPAPI, область CurrentUser, без дополнительной энтропии.

    #[cfg(target_os = "windows")]
    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        crypt(data, true)
    }

    #[cfg(target_os = "windows")]
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        crypt(data, false)
    }

    #[cfg(target_os = "windows")]
    fn crypt(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        };
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = unsafe {
            if encrypt {
                CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            }
        };
        if ok == 0 {
            return Err(format!("DPAPI: ошибка {}", std::io::Error::last_os_error()));
        }
        let out = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe { LocalFree(output.pbData as _) };
        Ok(out)
    }

    #[cfg(not(target_os = "windows"))]
    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.to_vec())
    }

    #[cfg(not(target_os = "windows"))]
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_туда_и_обратно() {
        let enc = dpapi::protect("секрет".as_bytes()).unwrap();
        assert_eq!(dpapi::unprotect(&enc).unwrap(), "секрет".as_bytes());
    }

    #[test]
    fn настройки_по_умолчанию_из_пустого() {
        let s: Settings = serde_json::from_str(r#"{"mode":"proxy"}"#).unwrap();
        assert_eq!(s.mode, Mode::Proxy);
        assert_eq!(s.proxy_port, 7890);
        assert!(s.presets.ru && s.presets.lan && !s.presets.geoip);
    }
}
