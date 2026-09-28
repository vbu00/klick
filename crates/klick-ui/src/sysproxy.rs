//! Системный прокси Windows текущего пользователя. Служба говорит, каким он должен быть
//! (`system_proxy` в состоянии), окно приводит к этому настройки Windows и оповещает программы.
//!
//! Ставим и снимаем через WinINet одной операцией (`INTERNET_OPTION_PER_CONNECTION_OPTION`):
//! так сразу обновляется запись настроек, которую читают Chrome, Edge и все программы на WinHTTP.
//! Если писать в реестр `ProxyEnable` и `ProxyServer` по одному, Windows пересобирает эту запись
//! из них, и браузер, перечитавший настройки посреди записи, остаётся «без прокси» — а с Kill Switch
//! это значит «без интернета».
//!
//! Прежние настройки пользователя — вместе с «Определять параметры автоматически» и сценарием
//! настройки — лежат в файле и возвращаются, когда VPN выключают.

use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use windows::core::PWSTR;
use windows::Win32::Foundation::{GlobalFree, HGLOBAL};
use windows::Win32::Networking::WinInet::{
    InternetQueryOptionW, InternetSetOptionW, INTERNET_OPTION_PER_CONNECTION_OPTION, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED, INTERNET_PER_CONN,
    INTERNET_PER_CONN_AUTOCONFIG_URL, INTERNET_PER_CONN_FLAGS, INTERNET_PER_CONN_FLAGS_UI, INTERNET_PER_CONN_OPTIONW, INTERNET_PER_CONN_OPTIONW_0,
    INTERNET_PER_CONN_OPTION_LISTW, INTERNET_PER_CONN_PROXY_BYPASS, INTERNET_PER_CONN_PROXY_SERVER, PROXY_TYPE_AUTO_DETECT, PROXY_TYPE_AUTO_PROXY_URL,
    PROXY_TYPE_DIRECT, PROXY_TYPE_PROXY,
};

/// Порты kl!ck: рабочая служба и служба для разработки.
const PORTS: [u16; 2] = [7890, 17890];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Spec {
    pub host: String,
    pub port: u16,
    pub bypass: Vec<String>,
}

/// Что стояло у пользователя до kl!ck.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Backup {
    enable: u32,
    server: Option<String>,
    bypass: Option<String>,
    /// Флаги WinINet (`PROXY_TYPE_*`) и адрес сценария настройки. В копиях прежних версий их нет —
    /// тогда флаги выводятся из `enable`.
    #[serde(default)]
    flags: Option<u32>,
    #[serde(default)]
    pac: Option<String>,
}

/// Настройки прокси подключения «по локальной сети» — им пользуются Wi-Fi и Ethernet.
#[derive(Clone, Debug, Default, PartialEq)]
struct Settings {
    flags: u32,
    server: Option<String>,
    bypass: Option<String>,
    pac: Option<String>,
}

impl Settings {
    fn proxy_on(&self) -> bool {
        self.flags & PROXY_TYPE_PROXY != 0
    }

    /// Стоит прокси kl!ck (127.0.0.1:7890 бывает и у других программ — это проверяет копия настроек).
    fn ours(&self) -> bool {
        self.proxy_on() && self.server.as_deref().is_some_and(|s| PORTS.iter().any(|p| s == format!("127.0.0.1:{p}")))
    }
}

impl Backup {
    fn of(s: &Settings) -> Self {
        Backup { enable: s.proxy_on() as u32, server: s.server.clone(), bypass: s.bypass.clone(), flags: Some(s.flags), pac: s.pac.clone() }
    }

    fn restore(&self, now: &Settings) -> Settings {
        let flags = self.flags.unwrap_or(if self.enable == 1 { PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY } else { PROXY_TYPE_DIRECT });
        Settings { flags, server: self.server.clone(), bypass: self.bypass.clone(), pac: self.pac.clone().or_else(|| now.pac.clone()) }
    }
}

/// Последнее, что поставили: чтобы не трогать настройки на каждое состояние.
static APPLIED: Mutex<Option<Spec>> = Mutex::new(None);
/// Первое состояние после подключения к службе сверяем с настройками всегда:
/// там мог остаться наш прокси от прошлого запуска.
static SYNCED: AtomicBool = AtomicBool::new(false);

/// Привести настройки Windows к тому, что просит служба.
pub fn reconcile(desired: Option<Spec>) {
    let first = !SYNCED.swap(true, Ordering::Relaxed);
    let applied = APPLIED.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match desired {
        Some(spec) if first || applied.as_ref() != Some(&spec) => {
            apply(&spec);
            *APPLIED.lock().unwrap_or_else(|e| e.into_inner()) = Some(spec);
        }
        None if first || applied.is_some() => clear_ours(),
        _ => {}
    }
}

/// Служба пропала: ядро остановилось вместе с ней, прокси смотрит в пустоту.
pub fn service_lost() {
    SYNCED.store(false, Ordering::Relaxed);
    clear_ours();
}

/// Снять наш прокси: при выходе, когда VPN выключили и когда служба пропала.
/// Трогаем настройки, только если прокси ставили мы (есть файл с прежними настройками):
/// 127.0.0.1:7890 бывает и у других программ, например у Clash for Windows.
pub fn clear_ours() {
    let Some(backup) = read_backup() else { return };
    if let Some(now) = query() {
        if now.ours() {
            set(&backup.restore(&now));
        }
    }
    let _ = std::fs::remove_file(backup_path());
    *APPLIED.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn apply(spec: &Spec) {
    let Some(now) = query() else { return };
    let ours = format!("{}:{}", spec.host, spec.port);
    let bypass = spec.bypass.join(";");
    // Запоминаем настройки пользователя один раз: повторное включение не должно затереть их нашими.
    // Прокси уже наш (его поставила служба, пока окна не было) — вернуть потом значит просто выключить.
    if read_backup().is_none() {
        let b = if now.ours() { Backup::default() } else { Backup::of(&now) };
        if let (Some(dir), Ok(text)) = (backup_path().parent(), serde_json::to_string(&b)) {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(backup_path(), text);
        }
    }
    // Автообнаружение и сценарий настройки на время VPN выключаем: браузер сначала пробует их,
    // и прямые запросы к ним с Kill Switch всё равно не пройдут.
    let wanted = Settings { flags: PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY, server: Some(ours), bypass: Some(bypass), pac: now.pac.clone() };
    let same = now.flags & (PROXY_TYPE_PROXY | PROXY_TYPE_AUTO_DETECT | PROXY_TYPE_AUTO_PROXY_URL) == PROXY_TYPE_PROXY
        && now.server == wanted.server
        && now.bypass == wanted.bypass;
    if !same {
        set(&wanted);
    }
}

/// Настройки прокси подключения «по локальной сети». Флаги — как в окне настроек Windows
/// (`FLAGS_UI`, с «Определять параметры автоматически»); на старых системах — обычные.
fn query() -> Option<Settings> {
    for flags_option in [INTERNET_PER_CONN_FLAGS_UI, INTERNET_PER_CONN_FLAGS.0] {
        let mut opts = [option(flags_option), option(INTERNET_PER_CONN_PROXY_SERVER.0), option(INTERNET_PER_CONN_PROXY_BYPASS.0), option(INTERNET_PER_CONN_AUTOCONFIG_URL.0)];
        let mut list = INTERNET_PER_CONN_OPTION_LISTW {
            dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            pszConnection: PWSTR::null(),
            dwOptionCount: opts.len() as u32,
            dwOptionError: 0,
            pOptions: opts.as_mut_ptr(),
        };
        let mut size = list.dwSize;
        let ok = unsafe { InternetQueryOptionW(None, INTERNET_OPTION_PER_CONNECTION_OPTION, Some(&mut list as *mut _ as *mut c_void), &mut size).is_ok() };
        if ok {
            return Some(unsafe {
                Settings { flags: opts[0].Value.dwValue, server: take(opts[1].Value.pszValue), bypass: take(opts[2].Value.pszValue), pac: take(opts[3].Value.pszValue) }
            });
        }
    }
    None
}

fn option(id: u32) -> INTERNET_PER_CONN_OPTIONW {
    INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN(id), ..Default::default() }
}

/// Строка от WinINet: забрать и освободить.
unsafe fn take(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let s = p.to_string().ok();
    let _ = GlobalFree(HGLOBAL(p.0 as *mut c_void));
    s.filter(|s| !s.is_empty())
}

/// Записать настройки одной операцией и сообщить программам на WinINet (браузеры, Telegram, магазины).
fn set(s: &Settings) {
    let mut server = wide(s.server.as_deref().unwrap_or(""));
    let mut bypass = wide(s.bypass.as_deref().unwrap_or(""));
    let mut pac = wide(s.pac.as_deref().unwrap_or(""));
    let mut opts = [
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_FLAGS, Value: INTERNET_PER_CONN_OPTIONW_0 { dwValue: s.flags } },
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_PROXY_SERVER, Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(server.as_mut_ptr()) } },
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_PROXY_BYPASS, Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(bypass.as_mut_ptr()) } },
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_AUTOCONFIG_URL, Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(pac.as_mut_ptr()) } },
    ];
    let list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        pszConnection: PWSTR::null(),
        dwOptionCount: opts.len() as u32,
        dwOptionError: 0,
        pOptions: opts.as_mut_ptr(),
    };
    unsafe {
        let _ = InternetSetOptionW(None, INTERNET_OPTION_PER_CONNECTION_OPTION, Some(&list as *const _ as *const c_void), list.dwSize);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
    }
}

fn backup_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("klick").join("proxy-backup.json")
}

fn read_backup() -> Option<Backup> {
    serde_json::from_str(&std::fs::read_to_string(backup_path()).ok()?).ok()
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_backups_restore_by_enable_flag() {
        let now = Settings { flags: PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY, server: Some("127.0.0.1:7890".into()), bypass: None, pac: Some("http://pac".into()) };
        let old: Backup = serde_json::from_str(r#"{"enable":1,"server":"10.0.0.1:3128","bypass":"<local>"}"#).unwrap();
        let r = old.restore(&now);
        assert_eq!(r.flags, PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY);
        assert_eq!(r.server.as_deref(), Some("10.0.0.1:3128"));
        assert_eq!(r.pac.as_deref(), Some("http://pac"));
        assert_eq!(Backup::default().restore(&now).flags, PROXY_TYPE_DIRECT);
    }

    #[test]
    fn auto_detect_comes_back() {
        let before = Settings { flags: PROXY_TYPE_DIRECT | PROXY_TYPE_AUTO_DETECT, server: None, bypass: None, pac: None };
        let b = Backup::of(&before);
        assert_eq!(b.enable, 0);
        assert_eq!(b.restore(&before), before);
        assert!(!before.ours());
        assert!(Settings { flags: PROXY_TYPE_PROXY, server: Some("127.0.0.1:7890".into()), ..Default::default() }.ours());
        assert!(!Settings { flags: PROXY_TYPE_DIRECT, server: Some("127.0.0.1:7890".into()), ..Default::default() }.ours());
    }
}
