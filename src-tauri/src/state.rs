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

/// Положение включённой маршрутизации: куда идёт то, что не попало ни под
/// одно правило. У каждого положения свой список правил.
///
/// Маршрутизация выключена (`Settings::routing`) — всё через VPN, списки
/// не действуют. Режим mihomo «Глобально / Напрямую» убран: куда идёт
/// трафик — только тумблер, положение и списки.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum DefaultRoute {
    /// «Всё, кроме списка»: через VPN, список — исключения.
    #[default]
    Proxy,
    /// «Только выбранное»: напрямую, через VPN — список.
    Direct,
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
    /// Полный путь — нужен, чтобы перевести программу в «Только через VPN»
    /// (правилу брандмауэра одного имени мало). У старых правил пуст.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
}

/// Список правил одного положения.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct RuleList {
    pub apps: Vec<AppRule>,
    pub sites: Vec<SiteRule>,
}

/// Свой список у каждого положения: пока включено одно, список другого
/// сохраняется.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Lists {
    /// «Всё, кроме списка»: что пустить напрямую или заблокировать.
    pub proxy: RuleList,
    /// «Только выбранное»: что пустить через VPN или заблокировать.
    pub direct: RuleList,
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

/// Готовые наборы — по одному на положение. Локальная сеть напрямую
/// всегда, без тумблера.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Sets {
    /// «Всё, кроме списка»: Россия напрямую — .ru/.su/.рф и российские IP
    /// по базе GeoIP.
    pub ru: bool,
    /// «Только выбранное»: заблокированные в РФ и закрывшиеся для РФ
    /// сервисы через VPN (список itdoginfo/allow-domains, качает mihomo).
    pub blocked: bool,
}

impl Default for Sets {
    fn default() -> Self {
        Sets { ru: true, blocked: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub active_profile: Option<String>,
    pub mode: Mode,
    /// Маршрутизация включена. Выключена — всё через VPN, списки и блок
    /// не действуют. У нового пользователя выключена.
    pub routing: bool,
    pub default_route: DefaultRoute,
    pub proxy_port: u16,
    pub sets: Sets,
    pub lists: Lists,
    pub kill_switch: bool,
    pub ks_apps: Vec<KsApp>,
    pub ks_sites: Vec<KsSite>,
    pub auto_update: bool,
    pub notify_drops: bool,
    /// Подключаться сразу после запуска (в том числе при входе в Windows).
    pub connect_on_launch: bool,
}

impl Settings {
    /// Список текущего положения.
    pub fn list(&self) -> &RuleList {
        match self.default_route {
            DefaultRoute::Proxy => &self.lists.proxy,
            DefaultRoute::Direct => &self.lists.direct,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            active_profile: None,
            mode: Mode::Tun,
            routing: false,
            default_route: DefaultRoute::Proxy,
            proxy_port: 7890,
            sets: Sets::default(),
            lists: Lists::default(),
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
    /// Сообщение окну один раз после запуска (переезд настроек).
    pub notice: Mutex<Option<String>>,
}

/// Настройки до 0.4: один общий список, быстрые исключения, режим mihomo.
/// Переносим так, чтобы в каждом положении всё работало как раньше:
/// «напрямую» и «блок» — в список «Всё, кроме списка», «через VPN» и
/// «блок» — в «Только выбранное». Маршрутизация у старого пользователя
/// включена (она у него была), кроме «Глобально» — это и есть «выключена».
/// Возвращает сообщение окну, если было что переносить.
fn migrate(raw: &mut Value) -> Option<String> {
    let obj = raw.as_object_mut()?;
    if obj.contains_key("routing") {
        return None;
    }
    let route = obj.remove("routeMode").and_then(|v| v.as_str().map(String::from));
    let apps = obj.remove("apps").unwrap_or(Value::Null);
    let sites = obj.remove("sites").unwrap_or(Value::Null);
    let presets = obj.remove("presets").unwrap_or(Value::Null);
    let pick = |list: &Value, keep: &[&str]| -> Value {
        Value::Array(list.as_array().map(|a| a.iter().filter(|r| r.get("action").and_then(Value::as_str).is_some_and(|x| keep.contains(&x))).cloned().collect()).unwrap_or_default())
    };
    obj.insert("lists".into(), serde_json::json!({
        "proxy": { "apps": pick(&apps, &["direct", "block"]), "sites": pick(&sites, &["direct", "block"]) },
        "direct": { "apps": pick(&apps, &["proxy", "block"]), "sites": pick(&sites, &["proxy", "block"]) },
    }));
    let flag = |k: &str| presets.get(k).and_then(Value::as_bool);
    let ru = flag("ru").unwrap_or(true) || flag("geoip").unwrap_or(false);
    obj.insert("sets".into(), serde_json::json!({ "ru": ru, "blocked": true }));
    let global = route.as_deref() == Some("global");
    obj.insert("routing".into(), Value::Bool(!global));
    Some(match route.as_deref() {
        Some("global") => "Режим «Глобально» теперь называется «Маршрутизация выключена»: всё идёт через VPN. Правила сохранены — включите маршрутизацию, и они заработают.".into(),
        Some("direct") => "Режим «Напрямую» убран: чтобы ничего не шло через VPN, просто выключите VPN. Правила перенесены на экран «Маршрутизация».".into(),
        _ => "Маршрутизация обновилась: у каждого положения теперь свой список. Ваши правила перенесены — загляните на экран «Маршрутизация».".into(),
    })
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
        let mut raw: Option<Value> = std::fs::read_to_string(dir.join("settings.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
        let notice = raw.as_mut().and_then(migrate);
        let settings = raw.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let vault = std::fs::read(dir.join("profiles.dat"))
            .ok()
            .and_then(|enc| dpapi::unprotect(&enc).ok())
            .and_then(|plain| serde_json::from_slice(&plain).ok())
            .unwrap_or_default();
        let state = AppState { settings: Mutex::new(settings), vault: Mutex::new(vault), dir, notice: Mutex::new(notice) };
        // Переписать без полей, которых больше нет, — сообщение один раз.
        if state.notice.lock().unwrap().is_some() {
            state.save_settings();
        }
        state
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
        assert!(!s.routing, "у нового пользователя маршрутизация выключена");
        assert!(s.sets.ru && s.sets.blocked);
    }

    #[test]
    fn переезд_старых_правил() {
        let mut old: Value = serde_json::from_str(r#"{"mode":"tun","routeMode":"rule","defaultRoute":"direct","presets":{"ru":false,"geoip":true},
            "apps":[{"name":"Discord","exe":"Discord.exe","action":"proxy"},{"name":"Steam","exe":"steam.exe","action":"direct"}],
            "sites":[{"pattern":"ads.example","action":"block"},{"pattern":"gosuslugi.ru","action":"direct"}]}"#).unwrap();
        assert!(migrate(&mut old).is_some());
        let s: Settings = serde_json::from_value(old.clone()).unwrap();
        assert!(s.routing);
        assert_eq!(s.default_route, DefaultRoute::Direct);
        assert!(s.sets.ru, "GeoIP был включён — набор «Россия» включён");
        let names = |l: &RuleList| l.apps.iter().map(|a| a.exe.clone()).chain(l.sites.iter().map(|x| x.pattern.clone())).collect::<Vec<_>>();
        assert_eq!(names(&s.lists.proxy), ["steam.exe", "ads.example", "gosuslugi.ru"]);
        assert_eq!(names(&s.lists.direct), ["Discord.exe", "ads.example"]);
        assert!(migrate(&mut old).is_none(), "второй раз не переносим");
        let mut global: Value = serde_json::json!({"routeMode": "global"});
        assert!(migrate(&mut global).unwrap().contains("выключена"));
        assert_eq!(global["routing"], false);
        let s: Settings = serde_json::from_value(global).unwrap();
        assert!(!serde_json::to_string(&s).unwrap().contains("routeMode"));
    }
}

/// Переезд настоящих настроек, без записи на диск:
/// KLICK_SETTINGS=путь cargo test --lib переезд_файла -- --ignored --nocapture
#[cfg(test)]
#[test]
#[ignore]
fn переезд_файла() {
    let path = std::env::var("KLICK_SETTINGS").expect("KLICK_SETTINGS");
    let mut raw: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    println!("сообщение: {:?}", migrate(&mut raw));
    let s: Settings = serde_json::from_value(raw).unwrap();
    let short = |l: &RuleList| format!("программ {}, сайтов {}", l.apps.len(), l.sites.len());
    println!("routing={} положение={:?} наборы={:?}", s.routing, s.default_route, s.sets);
    println!("«Всё, кроме списка»: {}; «Только выбранное»: {}; Kill Switch: {} ({} программ)", short(&s.lists.proxy), short(&s.lists.direct), s.kill_switch, s.ks_apps.len());
}
