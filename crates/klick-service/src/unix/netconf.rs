//! Системный прокси и DNS на macOS.
//!
//! Настройки сети в macOS общие для всех пользователей, живут у каждой сетевой службы
//! (Wi-Fi, Ethernet, USB-модем, Thunderbolt Bridge…) и меняются только с правами администратора.
//! Поэтому, в отличие от Windows, прокси ставит служба (она работает от root), а не окно.
//! Настройки переживают выход из программы и перезагрузку: всё, что kl!ck поменял, записано
//! в отметку на диске, и служба возвращает прежнее при отключении, после сбоя и при запуске.
//! Все сетевые службы читаются и меняются одной транзакцией SystemConfiguration (`scprefs.rs`).
//!
//! DNS в режиме VPN (TUN): ядро перехватывает запросы к порту 53 только внутри адаптера, а DNS-сервер
//! из локальной сети (обычно роутер 192.168.x.1) macOS спрашивает мимо адаптера. Без подмены имена
//! заблокированных сайтов отвечал бы провайдер. На время VPN служба ставит DNS 198.18.0.2 — этот адрес
//! ведёт в адаптер, и ядро отвечает само. Если ядро упадёт, DNS не заработает мимо VPN, пока служба
//! не вернёт прежний, — утечки нет.

#[cfg(target_os = "macos")]
#[path = "scprefs.rs"]
mod sc;

use klick_proto::SystemProxy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// DNS на время VPN: адрес внутри адаптера TUN (у адаптера 198.18.0.1/30).
pub const DNS_ADDR: &str = "198.18.0.2";
/// Отметки в папке данных службы.
pub const PROXY_MARKER: &str = "sysproxy.json";
pub const DNS_MARKER: &str = "dns.json";

/// Куда ходить мимо прокси: сам компьютер и локальная сеть.
pub const BYPASS: [&str; 9] = ["127.0.0.1", "localhost", "*.local", "169.254.0.0/16", "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "::1", "fe80::/10"];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    enabled: bool,
    server: String,
    port: u16,
}

/// Настройки прокси одной сетевой службы.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ServiceProxy {
    web: Entry,
    secure: Entry,
    socks: Entry,
    bypass: Vec<String>,
    /// Адрес сценария настройки (PAC), если он включён.
    auto_url: Option<String>,
    auto_discovery: bool,
}

impl ServiceProxy {
    fn ours(&self, port: u16) -> bool {
        self.web.enabled && self.web.server == "127.0.0.1" && self.web.port == port
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProxyBackup {
    port: u16,
    services: BTreeMap<String, ServiceProxy>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DnsBackup {
    /// Пустой список — DNS приходил от роутера (DHCP), своих адресов не было.
    services: BTreeMap<String, Vec<String>>,
}

/// Сетевая служба Mac и её настройки, которые трогает kl!ck.
#[derive(Clone, Debug, Default)]
struct Service {
    name: String,
    enabled: bool,
    proxy: ServiceProxy,
    /// Свои адреса DNS; пустой список — DNS от роутера (DHCP).
    dns: Vec<String>,
}

/// Что записать сетевой службе.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
    Proxy(ServiceProxy),
    Dns(Vec<String>),
}

/// Индекс службы в прочитанном списке и её новые настройки.
type Plan = Vec<(usize, Change)>;

/// Прочитать сетевые службы и записать то, что вернёт `plan`, — одной транзакцией.
/// `Ok(true)` — настройки поменялись, `Ok(false)` — менять было нечего.
fn edit(plan: impl FnOnce(&[Service]) -> Plan) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    return sc::edit(plan);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = plan;
        Err("настройки сети меняются только на macOS".into())
    }
}

/// Отметка на диске. Испорченную удаляем: вернуть по ней всё равно нечего, а повторять попытку
/// при каждой смене сети незачем.
fn read_marker<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    let parsed = serde_json::from_slice(&bytes);
    if parsed.is_err() {
        tracing::warn!("отметка {} испорчена, удаляю", path.display());
        let _ = std::fs::remove_file(path);
    }
    parsed.ok()
}

fn write_marker<T: Serialize>(path: &Path, v: &T) {
    if let Ok(bytes) = serde_json::to_vec_pretty(v) {
        if let Err(e) = crate::storage::write_atomic(path, &bytes) {
            tracing::warn!("отметка {} не записалась: {e:#}", path.display());
        }
    }
}

/// Поставить прокси kl!ck всем включённым сетевым службам. Повторный вызов (сменилась сеть,
/// появился USB-модем) досылает прокси новым службам и не затирает сохранённые прежние настройки.
/// `false` — включённых сетевых служб нет или настройки не записались.
pub fn proxy_apply(spec: &SystemProxy, marker: &Path) -> bool {
    let mut any = false;
    let result = edit(|services| {
        any = services.iter().any(|s| s.enabled);
        let mut backup: ProxyBackup = read_marker(marker).unwrap_or_default();
        let plan = plan_proxy_apply(&mut backup, spec, services);
        // Сначала отметка, потом настройки: при сбое посередине служба знает, что возвращать.
        write_marker(marker, &backup);
        plan
    });
    match result {
        Ok(_) => any,
        Err(e) => {
            tracing::warn!("системный прокси: {e}");
            false
        }
    }
}

fn plan_proxy_apply(backup: &mut ProxyBackup, spec: &SystemProxy, services: &[Service]) -> Plan {
    backup.port = spec.port;
    let ours = Entry { enabled: true, server: spec.host.clone(), port: spec.port };
    let want = ServiceProxy {
        web: ours.clone(),
        secure: ours.clone(),
        socks: ours,
        bypass: spec.bypass.clone(),
        // Сценарий настройки и автообнаружение на время VPN выключаем: браузер сначала пробует их.
        auto_url: None,
        auto_discovery: false,
    };
    let mut plan = Vec::new();
    for (i, s) in services.iter().enumerate().filter(|(_, s)| s.enabled) {
        // Прокси уже наш, а отметки нет (служба упала до записи): вернуть потом — значит выключить.
        backup.services.entry(s.name.clone()).or_insert_with(|| if s.proxy.ours(spec.port) { ServiceProxy::default() } else { s.proxy.clone() });
        if s.proxy != want {
            plan.push((i, Change::Proxy(want.clone())));
        }
    }
    plan
}

/// Снять прокси kl!ck: вернуть то, что было. Службы, где прокси уже сменили на чужой, не трогаем.
/// Не получилось — отметка остаётся, служба попробует снова при смене сети и при запуске.
pub fn proxy_clear(marker: &Path) -> bool {
    let Some(backup) = read_marker::<ProxyBackup>(marker) else { return false };
    match edit(|services| plan_proxy_clear(&backup, services)) {
        Ok(changed) => {
            let _ = std::fs::remove_file(marker);
            changed
        }
        Err(e) => {
            tracing::warn!("системный прокси не снялся, верну при следующей попытке: {e}");
            false
        }
    }
}

fn plan_proxy_clear(backup: &ProxyBackup, services: &[Service]) -> Plan {
    services
        .iter()
        .enumerate()
        .filter(|(_, s)| s.proxy.ours(backup.port))
        .filter_map(|(i, s)| Some((i, Change::Proxy(backup.services.get(&s.name)?.clone()))))
        .collect()
}

/// Осталась отметка от неудачного восстановления.
pub fn has_leftover(marker: &Path) -> bool {
    marker.exists()
}

fn dns_is_ours(dns: &[String]) -> bool {
    dns.len() == 1 && dns[0] == DNS_ADDR
}

/// DNS 198.18.0.2 всем включённым сетевым службам на время VPN (TUN).
pub fn dns_apply(marker: &Path) -> bool {
    let mut any = false;
    let result = edit(|services| {
        any = services.iter().any(|s| s.enabled);
        let mut backup: DnsBackup = read_marker(marker).unwrap_or_default();
        let plan = plan_dns_apply(&mut backup, services);
        write_marker(marker, &backup);
        plan
    });
    match result {
        Ok(changed) => {
            if changed {
                flush_dns_cache();
            }
            any
        }
        Err(e) => {
            tracing::warn!("DNS на время VPN: {e}");
            false
        }
    }
}

fn plan_dns_apply(backup: &mut DnsBackup, services: &[Service]) -> Plan {
    let mut plan = Vec::new();
    for (i, s) in services.iter().enumerate().filter(|(_, s)| s.enabled) {
        let ours = dns_is_ours(&s.dns);
        backup.services.entry(s.name.clone()).or_insert_with(|| if ours { Vec::new() } else { s.dns.clone() });
        if !ours {
            plan.push((i, Change::Dns(vec![DNS_ADDR.to_string()])));
        }
    }
    plan
}

/// Вернуть DNS, который стоял до VPN. Где DNS уже сменили на чужой, не трогаем.
pub fn dns_restore(marker: &Path) -> bool {
    let Some(backup) = read_marker::<DnsBackup>(marker) else { return false };
    match edit(|services| plan_dns_restore(&backup, services)) {
        Ok(changed) => {
            let _ = std::fs::remove_file(marker);
            if changed {
                flush_dns_cache();
            }
            changed
        }
        Err(e) => {
            tracing::warn!("DNS не вернулся, верну при следующей попытке: {e}");
            false
        }
    }
}

fn plan_dns_restore(backup: &DnsBackup, services: &[Service]) -> Plan {
    services
        .iter()
        .enumerate()
        .filter(|(_, s)| dns_is_ours(&s.dns))
        .filter_map(|(i, s)| Some((i, Change::Dns(backup.services.get(&s.name)?.clone()))))
        .collect()
}

/// Забыть ответы, полученные через VPN: подменные адреса 198.18.x.x без ядра никуда не ведут.
fn flush_dns_cache() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let limit = std::time::Duration::from_secs(5);
    let _ = crate::sys::run_within("/usr/bin/dscacheutil", &["-flushcache"], None, limit);
    let _ = crate::sys::run_within("/usr/bin/killall", &["-HUP", "mDNSResponder"], None, limit);
}

/// После сбоя службы или при удалении: вернуть прокси и DNS по отметкам.
pub fn restore_leftovers(data: &Path) -> (bool, bool) {
    (proxy_clear(&data.join(PROXY_MARKER)), dns_restore(&data.join(DNS_MARKER)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(server: &str, port: u16, enabled: bool) -> Entry {
        Entry { enabled, server: server.into(), port }
    }

    fn service(name: &str, enabled: bool, proxy: ServiceProxy, dns: &[&str]) -> Service {
        Service { name: name.into(), enabled, proxy, dns: dns.iter().map(|s| s.to_string()).collect() }
    }

    fn spec() -> SystemProxy {
        SystemProxy { host: "127.0.0.1".into(), port: 7890, bypass: BYPASS.iter().map(|s| s.to_string()).collect() }
    }

    fn applied(services: &mut [Service], plan: Plan) {
        for (i, change) in plan {
            match change {
                Change::Proxy(p) => services[i].proxy = p,
                Change::Dns(d) => services[i].dns = d,
            }
        }
    }

    #[test]
    fn proxy_round_trip_restores_what_was_there() {
        let user = ServiceProxy {
            web: entry("10.0.0.5", 3128, false),
            bypass: vec!["*.corp".into()],
            auto_url: Some("http://wpad/proxy.pac".into()),
            auto_discovery: true,
            ..Default::default()
        };
        let before = vec![
            service("Wi-Fi", true, user.clone(), &[]),
            service("Ethernet", true, ServiceProxy::default(), &[]),
            service("Bluetooth PAN", false, ServiceProxy::default(), &[]),
        ];
        let mut now = before.clone();
        let mut backup = ProxyBackup::default();
        let plan = plan_proxy_apply(&mut backup, &spec(), &now);
        assert_eq!(plan.len(), 2, "выключенную службу не трогаем");
        applied(&mut now, plan);
        assert!(now[0].proxy.ours(7890) && now[1].proxy.ours(7890) && !now[2].proxy.ours(7890));
        assert_eq!(now[0].proxy.auto_url, None);
        assert!(!now[0].proxy.auto_discovery);

        // Повторный вызов (сменилась сеть) ничего не меняет и прежние настройки не затирает.
        assert!(plan_proxy_apply(&mut backup, &spec(), &now).is_empty());
        assert_eq!(backup.services["Wi-Fi"], user);

        let plan = plan_proxy_clear(&backup, &now);
        applied(&mut now, plan);
        for (a, b) in now.iter().zip(&before) {
            assert_eq!(a.proxy, b.proxy, "{}", a.name);
        }
    }

    #[test]
    fn foreign_proxy_is_left_alone() {
        let mut backup = ProxyBackup::default();
        let mut now = vec![service("Wi-Fi", true, ServiceProxy::default(), &[])];
        let plan = plan_proxy_apply(&mut backup, &spec(), &now);
        applied(&mut now, plan);
        // Пока VPN работал, человек поставил свой прокси.
        now[0].proxy.web = entry("192.168.1.2", 8080, true);
        assert!(plan_proxy_clear(&backup, &now).is_empty());
    }

    #[test]
    fn our_proxy_without_marker_is_turned_off_later() {
        let mut backup = ProxyBackup::default();
        let mut now = vec![service("Wi-Fi", true, ServiceProxy::default(), &[])];
        let plan = plan_proxy_apply(&mut ProxyBackup::default(), &spec(), &now);
        applied(&mut now, plan);
        // Служба упала после записи настроек, отметки нет: вернуть — значит выключить.
        assert!(plan_proxy_apply(&mut backup, &spec(), &now).is_empty());
        assert_eq!(backup.services["Wi-Fi"], ServiceProxy::default());
    }

    #[test]
    fn dns_round_trip_restores_what_was_there() {
        let before = vec![
            service("Wi-Fi", true, ServiceProxy::default(), &["1.1.1.1", "8.8.8.8"]),
            service("Ethernet", true, ServiceProxy::default(), &[]),
            service("Thunderbolt Bridge", false, ServiceProxy::default(), &[]),
        ];
        let mut now = before.clone();
        let mut backup = DnsBackup::default();
        let plan = plan_dns_apply(&mut backup, &now);
        assert_eq!(plan.len(), 2);
        applied(&mut now, plan);
        assert!(dns_is_ours(&now[0].dns) && dns_is_ours(&now[1].dns) && now[2].dns.is_empty());
        assert!(plan_dns_apply(&mut backup, &now).is_empty());

        let plan = plan_dns_restore(&backup, &now);
        applied(&mut now, plan);
        for (a, b) in now.iter().zip(&before) {
            assert_eq!(a.dns, b.dns, "{}", a.name);
        }
    }

    #[test]
    fn foreign_dns_is_left_alone() {
        let mut backup = DnsBackup::default();
        let mut now = vec![service("Wi-Fi", true, ServiceProxy::default(), &["9.9.9.9"])];
        let plan = plan_dns_apply(&mut backup, &now);
        applied(&mut now, plan);
        now[0].dns = vec!["192.168.1.1".into()];
        assert!(plan_dns_restore(&backup, &now).is_empty());
    }

    #[test]
    fn markers_from_older_versions_still_read() {
        let proxy: ProxyBackup = serde_json::from_str(
            r#"{"port":7890,"services":{"Wi-Fi":{"web":{"enabled":false,"server":"","port":0},"secure":{"enabled":false,"server":"","port":0},"socks":{"enabled":false,"server":"","port":0},"bypass":[],"auto_url":null,"auto_discovery":false}}}"#,
        )
        .unwrap();
        assert_eq!(proxy.services["Wi-Fi"], ServiceProxy::default());
        let dns: DnsBackup = serde_json::from_str(r#"{"services":{"Wi-Fi":["1.1.1.1"]}}"#).unwrap();
        assert_eq!(dns.services["Wi-Fi"], vec!["1.1.1.1"]);
    }

    #[test]
    fn missing_or_broken_marker_means_nothing_to_restore() {
        let dir = std::env::temp_dir().join(format!("klick-netconf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(restore_leftovers(&dir), (false, false));
        std::fs::write(dir.join(PROXY_MARKER), b"{broken").unwrap();
        assert!(!proxy_clear(&dir.join(PROXY_MARKER)));
        assert!(!has_leftover(&dir.join(PROXY_MARKER)), "испорченную отметку не повторяем");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
