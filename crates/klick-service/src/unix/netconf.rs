//! Системный прокси и DNS на macOS.
//!
//! Настройки сети в macOS общие для всех пользователей, живут у каждой сетевой службы
//! (Wi-Fi, Ethernet, USB-модем, Thunderbolt Bridge…) и меняются только с правами администратора.
//! Поэтому, в отличие от Windows, прокси ставит служба (она работает от root), а не окно.
//! Настройки переживают выход из программы и перезагрузку: всё, что kl!ck поменял, записано
//! в отметку на диске, и служба возвращает прежнее при отключении, после сбоя и при запуске.
//! Все сетевые службы читаются и меняются одной транзакцией SystemConfiguration (`scprefs.rs`).
//! Резерв хранится по ID службы, а возвращается всем службам всех размещений (Location): смена размещения
//! во время VPN не оставляет в прежнем наш DNS и прокси. Отметка удаляется, только когда возврат подтверждён.
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

/// Резервы лежат по ID службы (имя человек может поменять, а у размещений имена повторяются).
/// Отметки прошлых версий хранят по имени — `saved` и `slot` их понимают.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ProxyBackup {
    port: u16,
    services: BTreeMap<String, ServiceProxy>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DnsBackup {
    /// Пустой список — DNS приходил от роутера (DHCP), своих адресов не было.
    services: BTreeMap<String, Vec<String>>,
}

/// Сетевая служба Mac и её настройки, которые трогает kl!ck.
#[derive(Clone, Debug, Default)]
struct Service {
    /// Постоянный ID службы (UUID в настройках сети).
    id: String,
    name: String,
    enabled: bool,
    /// Служба входит в текущее размещение (Location): только ей ставим настройки.
    in_current: bool,
    proxy: ServiceProxy,
    /// Свои адреса DNS; пустой список — DNS от роутера (DHCP).
    dns: Vec<String>,
}

impl Service {
    /// Служба, которой сейчас ставим настройки.
    fn active(&self) -> bool {
        self.enabled && self.in_current
    }
}

/// Ключ резерва службы — её ID; запись из старой отметки (по имени) переносим под ID.
fn slot<T>(map: &mut BTreeMap<String, T>, s: &Service) -> String {
    if !map.contains_key(&s.id) {
        if let Some(old) = map.remove(&s.name) {
            map.insert(s.id.clone(), old);
        }
    }
    s.id.clone()
}

fn saved<'a, T>(map: &'a BTreeMap<String, T>, s: &Service) -> Option<&'a T> {
    map.get(&s.id).or_else(|| map.get(&s.name))
}

/// Что записать сетевой службе.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
    Proxy(ServiceProxy),
    Dns(Vec<String>),
}

/// Индекс службы в прочитанном списке и её новые настройки.
type Plan = Vec<(usize, Change)>;

/// Чем кончилась попытка поставить или вернуть настройки. Различаем «менять было нечего» и «не смог»:
/// второе нельзя считать успехом, пока по сети идёт DNS 198.18.0.2 или прокси на остановленный порт.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Настройки записаны и применены.
    Done,
    /// Менять было нечего: настройки уже такие или возвращать нечего.
    Nothing,
    /// Не записалось (настройки заняты, нет служб…). Отметка осталась — повторить позже.
    Failed,
}

impl Outcome {
    /// Состояние системы известно и нужное: не `Failed`.
    pub fn ok(self) -> bool {
        self != Outcome::Failed
    }

    /// Настройки действительно поменялись.
    pub fn done(self) -> bool {
        self == Outcome::Done
    }
}

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

/// Отпечаток набора сетевых служб (ID текущего размещения и его служб) для `netwatch`:
/// смена размещения с теми же интерфейсом и IP иначе не видна. `None` — прочитать не удалось.
pub fn network_set() -> Option<String> {
    #[cfg(target_os = "macos")]
    return sc::network_set();
    #[cfg(not(target_os = "macos"))]
    None
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

/// `false` — отметка не записалась: настройки при этом менять нельзя, вернуть их будет не по чему.
fn write_marker<T: Serialize>(path: &Path, v: &T) -> bool {
    let written = serde_json::to_vec_pretty(v).map_err(anyhow::Error::from).and_then(|bytes| crate::storage::write_atomic(path, &bytes));
    if let Err(e) = &written {
        tracing::warn!("отметка {} не записалась, настройки сети не меняю: {e:#}", path.display());
    }
    written.is_ok()
}

/// Итог попытки поставить настройки. `any` — были службы, которым их ставят.
fn applied(result: Result<bool, String>, any: bool, marker_ok: bool, what: &str) -> Outcome {
    match result {
        Ok(_) if !marker_ok => Outcome::Failed,
        Ok(_) if !any => {
            tracing::warn!("{what}: нет включённых сетевых служб");
            Outcome::Failed
        }
        Ok(true) => Outcome::Done,
        Ok(false) => Outcome::Nothing,
        Err(e) => {
            tracing::warn!("{what}: {e}");
            Outcome::Failed
        }
    }
}

/// Итог попытки вернуть настройки. Отметку удаляем, только когда возврат подтверждён:
/// настройки записаны или вернуть было нечего. При ошибке она остаётся — повторим.
fn restored(result: Result<bool, String>, marker: &Path, what: &str) -> Outcome {
    match result {
        Ok(changed) => {
            let _ = std::fs::remove_file(marker);
            if changed {
                Outcome::Done
            } else {
                Outcome::Nothing
            }
        }
        Err(e) => {
            tracing::warn!("{what} не вернулся, верну при следующей попытке: {e}");
            Outcome::Failed
        }
    }
}

/// Поставить прокси kl!ck всем включённым сетевым службам. Повторный вызов (сменилась сеть,
/// появился USB-модем) досылает прокси новым службам и не затирает сохранённые прежние настройки.
pub fn proxy_apply(spec: &SystemProxy, marker: &Path) -> Outcome {
    let (mut any, mut marker_ok) = (false, true);
    let result = edit(|services| {
        any = services.iter().any(Service::active);
        let before: ProxyBackup = read_marker(marker).unwrap_or_default();
        let mut backup = before.clone();
        let plan = plan_proxy_apply(&mut backup, spec, services);
        // Сначала отметка, потом настройки: при сбое посередине служба знает, что возвращать.
        if backup != before {
            marker_ok = write_marker(marker, &backup);
        }
        if marker_ok {
            plan
        } else {
            Plan::new()
        }
    });
    applied(result, any, marker_ok, "системный прокси")
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
    for (i, s) in services.iter().enumerate().filter(|(_, s)| s.active()) {
        // Прокси уже наш, а отметки нет (служба упала до записи): вернуть потом — значит выключить.
        let key = slot(&mut backup.services, s);
        backup.services.entry(key).or_insert_with(|| if s.proxy.ours(spec.port) { ServiceProxy::default() } else { s.proxy.clone() });
        if s.proxy != want {
            plan.push((i, Change::Proxy(want.clone())));
        }
    }
    plan
}

/// Снять прокси kl!ck: вернуть то, что было. Службы, где прокси уже сменили на чужой, не трогаем.
/// Не получилось — отметка остаётся, служба попробует снова (смена сети, плановая проверка, запуск).
pub fn proxy_clear(marker: &Path) -> Outcome {
    let Some(backup) = read_marker::<ProxyBackup>(marker) else { return Outcome::Nothing };
    restored(edit(|services| plan_proxy_clear(&backup, services)), marker, "системный прокси")
}

/// Возвращаем службам ВСЕХ размещений, которым ставили (есть резерв) и где прокси всё ещё наш.
/// Наш прокси без резерва не трогаем: его мог поставить человек сам (например, свой Clash на том же порту).
fn plan_proxy_clear(backup: &ProxyBackup, services: &[Service]) -> Plan {
    services
        .iter()
        .enumerate()
        .filter(|(_, s)| s.proxy.ours(backup.port))
        .filter_map(|(i, s)| Some((i, Change::Proxy(saved(&backup.services, s)?.clone()))))
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
pub fn dns_apply(marker: &Path) -> Outcome {
    let (mut any, mut marker_ok) = (false, true);
    let result = edit(|services| {
        any = services.iter().any(Service::active);
        let before: DnsBackup = read_marker(marker).unwrap_or_default();
        let mut backup = before.clone();
        let plan = plan_dns_apply(&mut backup, services);
        if backup != before {
            marker_ok = write_marker(marker, &backup);
        }
        if marker_ok {
            plan
        } else {
            Plan::new()
        }
    });
    let outcome = applied(result, any, marker_ok, "DNS на время VPN");
    if outcome.done() {
        flush_dns_cache();
    }
    outcome
}

fn plan_dns_apply(backup: &mut DnsBackup, services: &[Service]) -> Plan {
    let mut plan = Vec::new();
    for (i, s) in services.iter().enumerate().filter(|(_, s)| s.active()) {
        let ours = dns_is_ours(&s.dns);
        let key = slot(&mut backup.services, s);
        backup.services.entry(key).or_insert_with(|| if ours { Vec::new() } else { s.dns.clone() });
        if !ours {
            plan.push((i, Change::Dns(vec![DNS_ADDR.to_string()])));
        }
    }
    plan
}

/// Вернуть DNS, который стоял до VPN. Где DNS уже сменили на чужой, не трогаем.
pub fn dns_restore(marker: &Path) -> Outcome {
    let Some(backup) = read_marker::<DnsBackup>(marker) else { return Outcome::Nothing };
    let outcome = restored(edit(|services| plan_dns_restore(&backup, services)), marker, "DNS");
    if outcome.done() {
        flush_dns_cache();
    }
    outcome
}

/// Всем службам ВСЕХ размещений, где DNS всё ещё наш. Адрес 198.18.0.2 есть только у kl!ck, поэтому
/// без резерва (отметку потеряли) — DNS от роутера: оставить его значило бы оставить службу без имён.
fn plan_dns_restore(backup: &DnsBackup, services: &[Service]) -> Plan {
    services
        .iter()
        .enumerate()
        .filter(|(_, s)| dns_is_ours(&s.dns))
        .map(|(i, s)| (i, Change::Dns(saved(&backup.services, s).cloned().unwrap_or_default())))
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

/// Что вернули по отметкам.
pub struct Restored {
    pub proxy: Outcome,
    pub dns: Outcome,
}

impl Restored {
    /// Хоть что-то не вернулось: отметка осталась, службе нужно повторить.
    pub fn failed(&self) -> bool {
        !self.proxy.ok() || !self.dns.ok()
    }
}

/// После сбоя службы или при удалении: вернуть прокси и DNS по отметкам.
pub fn restore_leftovers(data: &Path) -> Restored {
    Restored { proxy: proxy_clear(&data.join(PROXY_MARKER)), dns: dns_restore(&data.join(DNS_MARKER)) }
}

/// Отметки, которые остались: возврат по ним не подтверждён, стирать их вместе с данными нельзя.
pub fn pending_markers(data: &Path) -> Vec<&'static str> {
    [PROXY_MARKER, DNS_MARKER].into_iter().filter(|m| has_leftover(&data.join(m))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(server: &str, port: u16, enabled: bool) -> Entry {
        Entry { enabled, server: server.into(), port }
    }

    fn service(name: &str, enabled: bool, proxy: ServiceProxy, dns: &[&str]) -> Service {
        Service { id: format!("id-{name}"), name: name.into(), enabled, in_current: true, proxy, dns: dns.iter().map(|s| s.to_string()).collect() }
    }

    fn spec() -> SystemProxy {
        SystemProxy { host: "127.0.0.1".into(), port: 7890, bypass: BYPASS.iter().map(|s| s.to_string()).collect() }
    }

    fn write(services: &mut [Service], plan: Plan) {
        for (i, change) in plan {
            match change {
                Change::Proxy(p) => services[i].proxy = p,
                Change::Dns(d) => services[i].dns = d,
            }
        }
    }

    /// Составить план по текущим службам и записать его.
    fn step(now: &mut [Service], plan: impl FnOnce(&[Service]) -> Plan) {
        let plan = plan(now);
        write(now, plan);
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("klick-netconf-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
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
        write(&mut now, plan);
        assert!(now[0].proxy.ours(7890) && now[1].proxy.ours(7890) && !now[2].proxy.ours(7890));
        assert_eq!(now[0].proxy.auto_url, None);
        assert!(!now[0].proxy.auto_discovery);

        // Повторный вызов (сменилась сеть) ничего не меняет и прежние настройки не затирает.
        assert!(plan_proxy_apply(&mut backup, &spec(), &now).is_empty());
        assert_eq!(backup.services["id-Wi-Fi"], user);

        let plan = plan_proxy_clear(&backup, &now);
        write(&mut now, plan);
        for (a, b) in now.iter().zip(&before) {
            assert_eq!(a.proxy, b.proxy, "{}", a.name);
        }
    }

    #[test]
    fn foreign_proxy_is_left_alone() {
        let mut backup = ProxyBackup::default();
        let mut now = vec![service("Wi-Fi", true, ServiceProxy::default(), &[])];
        let plan = plan_proxy_apply(&mut backup, &spec(), &now);
        write(&mut now, plan);
        // Пока VPN работал, человек поставил свой прокси.
        now[0].proxy.web = entry("192.168.1.2", 8080, true);
        assert!(plan_proxy_clear(&backup, &now).is_empty());
    }

    #[test]
    fn our_proxy_without_marker_is_turned_off_later() {
        let mut backup = ProxyBackup::default();
        let mut now = vec![service("Wi-Fi", true, ServiceProxy::default(), &[])];
        let plan = plan_proxy_apply(&mut ProxyBackup::default(), &spec(), &now);
        write(&mut now, plan);
        // Служба упала после записи настроек, отметки нет: вернуть — значит выключить.
        assert!(plan_proxy_apply(&mut backup, &spec(), &now).is_empty());
        assert_eq!(backup.services["id-Wi-Fi"], ServiceProxy::default());
    }

    #[test]
    fn proxy_nobody_saved_for_is_not_touched() {
        // 127.0.0.1:7890 в службе другого размещения мог поставить сам человек: без резерва не трогаем.
        let mut own = service("Wi-Fi", true, ServiceProxy::default(), &[]);
        own.proxy.web = entry("127.0.0.1", 7890, true);
        let backup = ProxyBackup { port: 7890, services: BTreeMap::new() };
        assert!(plan_proxy_clear(&backup, &[own]).is_empty());
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
        write(&mut now, plan);
        assert!(dns_is_ours(&now[0].dns) && dns_is_ours(&now[1].dns) && now[2].dns.is_empty());
        assert!(plan_dns_apply(&mut backup, &now).is_empty());

        let plan = plan_dns_restore(&backup, &now);
        write(&mut now, plan);
        for (a, b) in now.iter().zip(&before) {
            assert_eq!(a.dns, b.dns, "{}", a.name);
        }
    }

    #[test]
    fn foreign_dns_is_left_alone() {
        let mut backup = DnsBackup::default();
        let mut now = vec![service("Wi-Fi", true, ServiceProxy::default(), &["9.9.9.9"])];
        let plan = plan_dns_apply(&mut backup, &now);
        write(&mut now, plan);
        now[0].dns = vec!["192.168.1.1".into()];
        assert!(plan_dns_restore(&backup, &now).is_empty());
    }

    /// Размещение A -> VPN -> размещение B -> VPN выключен -> снова A: в A не должно остаться нашего.
    #[test]
    fn location_switch_during_vpn_does_not_leave_ours_behind() {
        let mut a = service("Wi-Fi", true, ServiceProxy::default(), &["1.1.1.1"]);
        a.id = "A-wifi".into();
        let mut b = service("Wi-Fi", true, ServiceProxy::default(), &[]);
        b.id = "B-wifi".into();
        b.in_current = false;
        let before = [a.clone(), b.clone()];
        let mut now = vec![a, b];
        let (mut dns, mut proxy) = (DnsBackup::default(), ProxyBackup::default());

        // VPN включили в A.
        step(&mut now, |s| plan_dns_apply(&mut dns, s));
        step(&mut now, |s| plan_proxy_apply(&mut proxy, &spec(), s));
        assert!(dns_is_ours(&now[0].dns) && dns.services.len() == 1, "B ещё не трогали");

        // Переключились на B: служба досылает настройки новому размещению.
        now[0].in_current = false;
        now[1].in_current = true;
        step(&mut now, |s| plan_dns_apply(&mut dns, s));
        step(&mut now, |s| plan_proxy_apply(&mut proxy, &spec(), s));
        assert!(dns_is_ours(&now[1].dns) && dns.services.len() == 2);

        // VPN выключили в B: возвращается всё, и в A тоже.
        step(&mut now, |s| plan_dns_restore(&dns, s));
        step(&mut now, |s| plan_proxy_clear(&proxy, s));
        for (x, y) in now.iter().zip(&before) {
            assert_eq!((&x.dns, &x.proxy), (&y.dns, &y.proxy), "{}", x.id);
        }
    }

    #[test]
    fn renamed_service_is_still_restored() {
        let mut backup = DnsBackup::default();
        let mut now = vec![service("Wi-Fi", true, ServiceProxy::default(), &["1.1.1.1"])];
        let plan = plan_dns_apply(&mut backup, &now);
        write(&mut now, plan);
        now[0].name = "Домашний Wi-Fi".into();
        let plan = plan_dns_restore(&backup, &now);
        write(&mut now, plan);
        assert_eq!(now[0].dns, vec!["1.1.1.1"]);
    }

    #[test]
    fn our_dns_without_backup_goes_back_to_dhcp() {
        let now = vec![service("Wi-Fi", true, ServiceProxy::default(), &[DNS_ADDR])];
        assert_eq!(plan_dns_restore(&DnsBackup::default(), &now), vec![(0, Change::Dns(Vec::new()))]);
    }

    #[test]
    fn marker_by_name_from_older_version_is_migrated() {
        let mut backup = DnsBackup::default();
        backup.services.insert("Wi-Fi".into(), vec!["1.1.1.1".into()]);
        let now = vec![service("Wi-Fi", true, ServiceProxy::default(), &[DNS_ADDR])];
        // Восстановление находит резерв по имени…
        assert_eq!(plan_dns_restore(&backup, &now), vec![(0, Change::Dns(vec!["1.1.1.1".into()]))]);
        // …а повторная установка переносит его под ID, ничего не теряя.
        assert!(plan_dns_apply(&mut backup, &now).is_empty());
        assert_eq!(backup.services.get("id-Wi-Fi"), Some(&vec!["1.1.1.1".to_string()]));
        assert!(!backup.services.contains_key("Wi-Fi"));
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
        let dir = temp("broken");
        let r = restore_leftovers(&dir);
        assert_eq!((r.proxy, r.dns), (Outcome::Nothing, Outcome::Nothing));
        assert!(!r.failed());
        std::fs::write(dir.join(PROXY_MARKER), b"{broken").unwrap();
        assert_eq!(proxy_clear(&dir.join(PROXY_MARKER)), Outcome::Nothing);
        assert!(!has_leftover(&dir.join(PROXY_MARKER)), "испорченную отметку не повторяем");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Отметка живёт, пока возврат не подтверждён: ошибка записи её не стирает, «нечего менять» стирает.
    #[test]
    fn marker_is_removed_only_when_restore_is_confirmed() {
        let dir = temp("settle");
        let marker = dir.join(DNS_MARKER);
        let run = |result: Result<bool, String>| {
            std::fs::write(&marker, b"{}").unwrap();
            let outcome = restored(result, &marker, "DNS");
            (outcome, has_leftover(&marker))
        };
        assert_eq!(run(Err("настройки заняты".into())), (Outcome::Failed, true));
        assert_eq!(run(Ok(false)), (Outcome::Nothing, false));
        assert_eq!(run(Ok(true)), (Outcome::Done, false));
        assert!(Outcome::Nothing.ok() && Outcome::Done.ok() && !Outcome::Failed.ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_outcome_tells_nothing_from_failure() {
        assert_eq!(applied(Ok(true), true, true, "x"), Outcome::Done);
        assert_eq!(applied(Ok(false), true, true, "x"), Outcome::Nothing);
        assert_eq!(applied(Ok(false), false, true, "x"), Outcome::Failed, "нет служб — не успех");
        assert_eq!(applied(Ok(false), true, false, "x"), Outcome::Failed, "без отметки настройки не менялись");
        assert_eq!(applied(Err("e".into()), true, true, "x"), Outcome::Failed);
    }

    #[test]
    fn pending_markers_list_what_is_left() {
        let dir = temp("pending");
        assert!(pending_markers(&dir).is_empty());
        std::fs::write(dir.join(DNS_MARKER), b"{}").unwrap();
        assert_eq!(pending_markers(&dir), vec![DNS_MARKER]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
