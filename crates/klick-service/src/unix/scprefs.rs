//! Настройки сети через SystemConfiguration — тот же preferences.plist, что меняют «Системные
//! настройки» и networksetup, но одной транзакцией: прочитать все сетевые службы, поменять нужные,
//! записать и применить разом. networksetup — отдельный процесс на каждую настройку каждой службы:
//! на Mac с пятью-шестью сетевыми службами отключение VPN занимало 8–10 секунд.

use super::{Change, Entry, Service, ServiceProxy};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{Boolean, CFAllocatorRef, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionary};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::time::Duration;

type ScRef = *const c_void;
type Dict = CFDictionary<CFString, CFType>;
type MutDict = CFMutableDictionary<CFString, CFType>;

#[link(name = "SystemConfiguration", kind = "framework")]
extern "C" {
    fn SCPreferencesCreate(allocator: CFAllocatorRef, name: CFStringRef, prefs_id: CFStringRef) -> ScRef;
    fn SCPreferencesLock(prefs: ScRef, wait: Boolean) -> Boolean;
    fn SCPreferencesUnlock(prefs: ScRef) -> Boolean;
    fn SCPreferencesSynchronize(prefs: ScRef);
    fn SCPreferencesCommitChanges(prefs: ScRef) -> Boolean;
    fn SCPreferencesApplyChanges(prefs: ScRef) -> Boolean;
    fn SCNetworkSetCopyCurrent(prefs: ScRef) -> ScRef;
    fn SCNetworkSetCopyServices(set: ScRef) -> CFArrayRef;
    fn SCNetworkServiceGetName(service: ScRef) -> CFStringRef;
    fn SCNetworkServiceGetEnabled(service: ScRef) -> Boolean;
    fn SCNetworkServiceCopyProtocol(service: ScRef, kind: CFStringRef) -> ScRef;
    fn SCNetworkServiceAddProtocolType(service: ScRef, kind: CFStringRef) -> Boolean;
    fn SCNetworkProtocolGetConfiguration(protocol: ScRef) -> CFDictionaryRef;
    fn SCNetworkProtocolSetConfiguration(protocol: ScRef, config: CFDictionaryRef) -> Boolean;
    fn SCError() -> c_int;
    fn SCErrorString(status: c_int) -> *const c_char;
}

/// kSCStatusPrefsBusy: настройки заблокировала другая программа (например, «Системные настройки»).
const PREFS_BUSY: c_int = 3002;
/// kSCStatusStale: настройки поменялись с тех пор, как мы их прочитали.
const STALE: c_int = 3005;

const PROXIES: &str = "Proxies";
const DNS: &str = "DNS";

/// Прочитать сетевые службы текущего размещения и записать то, что вернёт `plan` (индекс службы
/// в прочитанном списке и новые настройки). Всё — под блокировкой настроек, одной записью.
/// `Ok(true)` — настройки поменялись, `Ok(false)` — менять было нечего.
pub(super) fn edit(plan: impl FnOnce(&[Service]) -> Vec<(usize, Change)>) -> Result<bool, String> {
    let name = CFString::from_static_string("kl!ck");
    let prefs = owned(unsafe { SCPreferencesCreate(std::ptr::null(), name.as_concrete_TypeRef(), std::ptr::null()) })
        .ok_or_else(|| error("настройки сети не открылись"))?;
    let prefs = prefs.as_CFTypeRef();
    let _lock = Lock::take(prefs)?;
    let set = owned(unsafe { SCNetworkSetCopyCurrent(prefs) }).ok_or_else(|| error("нет текущего размещения сети"))?;
    let list = unsafe { SCNetworkSetCopyServices(set.as_CFTypeRef()) };
    if list.is_null() {
        return Err(error("сетевые службы не прочитались"));
    }
    let list: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(list) };
    let mut handles = Vec::new();
    let mut services = Vec::new();
    for service in list.iter() {
        let raw = service.as_CFTypeRef();
        let name = unsafe { SCNetworkServiceGetName(raw) };
        if name.is_null() {
            continue;
        }
        let proxies = protocol(raw, PROXIES, false)?;
        let dns = protocol(raw, DNS, false)?;
        services.push(Service {
            name: unsafe { CFString::wrap_under_get_rule(name) }.to_string(),
            enabled: unsafe { SCNetworkServiceGetEnabled(raw) } != 0,
            proxy: proxies.as_ref().map(|p| read_proxy(&config(p))).unwrap_or_default(),
            dns: dns.as_ref().map(|p| list_of(&config(p), "ServerAddresses")).unwrap_or_default(),
        });
        handles.push((*service).clone());
    }

    let changes = plan(&services);
    if changes.is_empty() {
        return Ok(false);
    }
    for (i, change) in &changes {
        let service = handles.get(*i).ok_or("служба вне списка")?.as_CFTypeRef();
        let (kind, what) = match change {
            Change::Proxy(_) => (PROXIES, "прокси"),
            Change::Dns(_) => (DNS, "DNS"),
        };
        let protocol = protocol(service, kind, true)?.ok_or_else(|| error(what))?;
        let current = config(&protocol);
        let mut next = MutDict::from(&current);
        match change {
            Change::Proxy(want) => write_proxy(&mut next, &read_proxy(&current), want),
            Change::Dns(want) => put_list(&mut next, "ServerAddresses", want),
        }
        let ok = unsafe { SCNetworkProtocolSetConfiguration(protocol.as_CFTypeRef(), next.as_concrete_TypeRef()) };
        if ok == 0 {
            return Err(error(&format!("{what} службы «{}» не записался", services[*i].name)));
        }
    }
    if unsafe { SCPreferencesCommitChanges(prefs) } == 0 {
        return Err(error("настройки сети не записались"));
    }
    if unsafe { SCPreferencesApplyChanges(prefs) } == 0 {
        return Err(error("настройки сети записались, но не применились"));
    }
    Ok(true)
}

/// Блокировка настроек сети: снимается при выходе из `edit`, в том числе по ошибке.
struct Lock(ScRef);

impl Lock {
    /// Не ждём бесконечно: «Системные настройки» держат блокировку доли секунды, а зависшая чужая
    /// программа не должна подвесить отключение VPN.
    fn take(prefs: ScRef) -> Result<Lock, String> {
        for _ in 0..50 {
            if unsafe { SCPreferencesLock(prefs, 0) } != 0 {
                return Ok(Lock(prefs));
            }
            match unsafe { SCError() } {
                STALE => unsafe { SCPreferencesSynchronize(prefs) },
                PREFS_BUSY => std::thread::sleep(Duration::from_millis(100)),
                _ => return Err(error("настройки сети не заблокировались")),
            }
        }
        Err("настройки сети заняты другой программой".into())
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        unsafe { SCPreferencesUnlock(self.0) };
    }
}

fn error(what: &str) -> String {
    let code = unsafe { SCError() };
    let text = unsafe { SCErrorString(code) };
    let text = if text.is_null() { String::new() } else { unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned() };
    format!("{what}: {text} ({code})")
}

/// Объект, полученный по правилу Create/Copy, — освобождается сам.
fn owned(r: ScRef) -> Option<CFType> {
    (!r.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(r as CFTypeRef) })
}

/// Раздел настроек службы: «Proxies» или «DNS». Если его нет и `create` — добавить.
fn protocol(service: ScRef, kind: &str, create: bool) -> Result<Option<CFType>, String> {
    let kind = CFString::new(kind);
    let copy = || owned(unsafe { SCNetworkServiceCopyProtocol(service, kind.as_concrete_TypeRef()) });
    if let Some(p) = copy() {
        return Ok(Some(p));
    }
    if !create {
        return Ok(None);
    }
    if unsafe { SCNetworkServiceAddProtocolType(service, kind.as_concrete_TypeRef()) } == 0 {
        return Err(error(&format!("раздел {kind} не добавился")));
    }
    Ok(copy())
}

fn config(protocol: &CFType) -> Dict {
    let d = unsafe { SCNetworkProtocolGetConfiguration(protocol.as_CFTypeRef()) };
    if d.is_null() {
        Dict::from_CFType_pairs(&[])
    } else {
        unsafe { Dict::wrap_under_get_rule(d) }
    }
}

fn value(d: &Dict, key: &str) -> Option<CFType> {
    d.find(CFString::new(key)).map(|v| (*v).clone())
}

fn number(d: &Dict, key: &str) -> i64 {
    match value(d, key) {
        Some(v) => v
            .downcast::<CFNumber>()
            .and_then(|n| n.to_i64())
            .or_else(|| v.downcast::<CFBoolean>().map(|b| bool::from(b) as i64))
            .unwrap_or(0),
        None => 0,
    }
}

fn string(d: &Dict, key: &str) -> Option<String> {
    value(d, key)?.downcast::<CFString>().map(|s| s.to_string())
}

fn list_of(d: &Dict, key: &str) -> Vec<String> {
    let Some(array) = value(d, key).and_then(|v| v.downcast::<CFArray>()) else { return Vec::new() };
    array
        .iter()
        .filter_map(|item| unsafe { CFType::wrap_under_get_rule(*item as CFTypeRef) }.downcast::<CFString>())
        .map(|s| s.to_string())
        .collect()
}

/// Три вида прокси в настройках: HTTP (веб), HTTPS (защищённый веб) и SOCKS.
fn entry(d: &Dict, kind: &str) -> Entry {
    Entry {
        enabled: number(d, &format!("{kind}Enable")) != 0,
        server: string(d, &format!("{kind}Proxy")).unwrap_or_default(),
        port: u16::try_from(number(d, &format!("{kind}Port"))).unwrap_or(0),
    }
}

fn read_proxy(d: &Dict) -> ServiceProxy {
    ServiceProxy {
        web: entry(d, "HTTP"),
        secure: entry(d, "HTTPS"),
        socks: entry(d, "SOCKS"),
        bypass: list_of(d, "ExceptionsList"),
        auto_url: string(d, "ProxyAutoConfigURLString").filter(|u| !u.is_empty() && number(d, "ProxyAutoConfigEnable") != 0),
        auto_discovery: number(d, "ProxyAutoDiscoveryEnable") != 0,
    }
}

fn put_number(d: &mut MutDict, key: &str, v: i32) {
    d.set(CFString::new(key), CFNumber::from(v).as_CFType());
}

fn put_string(d: &mut MutDict, key: &str, v: &str) {
    d.set(CFString::new(key), CFString::new(v).as_CFType());
}

fn put_list(d: &mut MutDict, key: &str, list: &[String]) {
    if list.is_empty() {
        d.remove(CFString::new(key));
    } else {
        let items: Vec<CFString> = list.iter().map(|s| CFString::new(s)).collect();
        d.set(CFString::new(key), CFArray::from_CFTypes(&items).as_CFType());
    }
}

fn put_entry(d: &mut MutDict, kind: &str, e: &Entry) {
    let set = !e.server.is_empty() && e.port > 0;
    put_number(d, &format!("{kind}Enable"), (e.enabled && set) as i32);
    if set {
        put_string(d, &format!("{kind}Proxy"), &e.server);
        put_number(d, &format!("{kind}Port"), e.port.into());
    } else {
        d.remove(CFString::new(&format!("{kind}Proxy")));
        d.remove(CFString::new(&format!("{kind}Port")));
    }
}

/// Записать только то, что отличается: остальные ключи (FTP, RTSP, «не для простых имён»…) не трогаем.
fn write_proxy(d: &mut MutDict, now: &ServiceProxy, want: &ServiceProxy) {
    for (kind, a, b) in [("HTTP", &now.web, &want.web), ("HTTPS", &now.secure, &want.secure), ("SOCKS", &now.socks, &want.socks)] {
        if a != b {
            put_entry(d, kind, b);
        }
    }
    if now.bypass != want.bypass {
        put_list(d, "ExceptionsList", &want.bypass);
    }
    if now.auto_url != want.auto_url {
        match &want.auto_url {
            Some(url) => {
                put_string(d, "ProxyAutoConfigURLString", url);
                put_number(d, "ProxyAutoConfigEnable", 1);
            }
            // Адрес сценария оставляем: вернуть — значит снова включить.
            None => put_number(d, "ProxyAutoConfigEnable", 0),
        }
    }
    if now.auto_discovery != want.auto_discovery {
        put_number(d, "ProxyAutoDiscoveryEnable", want.auto_discovery as i32);
    }
}
