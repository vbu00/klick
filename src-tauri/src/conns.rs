//! Страница «Соединение»: что сейчас в сети и куда идёт, что не открылось
//! и как компьютер видят сайты.
//!
//! Всё — из самого ядра: список соединений mihomo (программа, адрес,
//! правило, цепочка) и строки его лога о неудачах. Внешние запросы — только
//! проверка IP, и только по кнопке.

use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::RU_ZONES;
use crate::state::{DefaultRoute, Settings};

/// Проверка IP через VPN — ядро всегда ведёт этот адрес через сервер.
pub const IP_VPN_HOST: &str = "ipinfo.io";
/// Проверка IP напрямую — ядро всегда ведёт этот адрес мимо VPN.
pub const IP_DIRECT_HOST: &str = "ipwho.is";

// ─────────── Сейчас в сети ───────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostRow {
    pub host: String,
    /// vpn | direct
    pub route: &'static str,
    pub why: String,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppRow {
    pub exe: String,
    pub name: String,
    pub path: String,
    /// vpn | direct | mixed | ks
    pub route: &'static str,
    pub conns: usize,
    /// Байт/с вверх и вниз вместе.
    pub speed: u64,
    pub why: String,
    pub hosts: Vec<HostRow>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub host: String,
    pub app: String,
    /// Куда шло: vpn | direct | block
    pub route: &'static str,
    pub why: String,
    pub ago: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub on: bool,
    pub apps: Vec<AppRow>,
    pub failures: Vec<Failure>,
    /// Разных адресов сейчас: через VPN, напрямую; блок — за 10 минут.
    pub n_vpn: usize,
    pub n_direct: usize,
    pub n_block: usize,
}

/// Прошлый замер: байты каждого соединения — для скорости.
static PREV: Lazy<Mutex<(Option<Instant>, HashMap<String, u64>)>> = Lazy::new(|| Mutex::new((None, HashMap::new())));
/// Человеческие имена программ по пути exe.
static NAMES: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn display_name(path: &str, exe: &str) -> String {
    if path.is_empty() {
        return exe.trim_end_matches(".exe").to_string();
    }
    let mut names = NAMES.lock().unwrap();
    names
        .entry(path.to_lowercase())
        .or_insert_with(|| crate::procs::file_description(path).unwrap_or_else(|| exe.trim_end_matches(".exe").to_string()))
        .clone()
}

fn is_ks(s: &Settings, exe: &str) -> bool {
    s.kill_switch && s.ks_apps.iter().any(|a| a.on && crate::route::exe_of(&a.path, &a.exe).eq_ignore_ascii_case(exe))
}

/// Почему соединение пошло так — словами, по правилу, которое сработало.
pub fn why(s: &Settings, rule: &str, payload: &str, exe: &str) -> String {
    let p = payload.to_lowercase();
    match rule {
        "ProcessPathRegex" => "замер задержки kl!ck".into(),
        "ProcessName" if is_ks(s, exe) => "только через VPN (Kill Switch)".into(),
        "ProcessName" => "правило программы".into(),
        "Domain" if p == IP_VPN_HOST || p == IP_DIRECT_HOST => "проверка IP kl!ck".into(),
        "DomainSuffix" if ["local", "lan", "home.arpa"].contains(&p.as_str()) => "локальная сеть".into(),
        "DomainSuffix" if p == "msftconnecttest.com" || p == "msftncsi.com" => "проверка сети Windows".into(),
        "DomainSuffix" if RU_ZONES.contains(&p.as_str()) => "набор «Россия напрямую»".into(),
        "DomainSuffix" | "Domain" => "правило сайта".into(),
        "IPCIDR" | "IPCIDR6" if p.ends_with("/32") || p.ends_with("/128") => "правило сайта (адрес)".into(),
        "IPCIDR" | "IPCIDR6" => "локальная сеть".into(),
        "GeoIP" => "набор «Россия напрямую» (российский адрес)".into(),
        "RuleSet" => "набор «Заблокированные в РФ»".into(),
        "Match" if !s.routing => "маршрутизация выключена".into(),
        "Match" => match s.default_route {
            DefaultRoute::Proxy => "всё остальное — «VPN для всего»".into(),
            DefaultRoute::Direct => "всё остальное — «VPN для выбранного»".into(),
        },
        "" => "без правила".into(),
        other => format!("правило {other}"),
    }
}

struct Conn {
    exe: String,
    path: String,
    host: String,
    route: &'static str,
    why: String,
    by_program: bool,
    bytes: u64,
    id: String,
}

fn parse(s: &Settings, v: &Value) -> Vec<Conn> {
    let str_at = |c: &Value, p: &str| c.pointer(p).and_then(Value::as_str).unwrap_or("").to_string();
    v.get("connections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let exe = str_at(c, "/metadata/process");
            let mut host = str_at(c, "/metadata/host");
            if host.is_empty() {
                host = str_at(c, "/metadata/sniffHost");
            }
            if host.is_empty() {
                let ip = str_at(c, "/metadata/destinationIP");
                let net = str_at(c, "/metadata/network").to_uppercase();
                host = if net == "UDP" { format!("{ip} · UDP") } else { ip };
            }
            let chains: Vec<String> = c.get("chains").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let route = if chains.iter().any(|x| x == "DIRECT") { "direct" } else { "vpn" };
            let rule = str_at(c, "/rule");
            let payload = str_at(c, "/rulePayload");
            // Наш же зонд задержки и проверки — не показываем.
            if rule == "ProcessPathRegex" {
                return None;
            }
            let bytes = c.get("upload").and_then(Value::as_u64).unwrap_or(0) + c.get("download").and_then(Value::as_u64).unwrap_or(0);
            Some(Conn {
                why: why(s, &rule, &payload, &exe),
                by_program: rule == "ProcessName",
                exe: if exe.is_empty() { "неизвестная программа".into() } else { exe },
                path: str_at(c, "/metadata/processPath"),
                host: host.trim_start_matches("www.").to_string(),
                route,
                bytes,
                id: str_at(c, "/id"),
            })
        })
        .collect()
}

pub fn view(s: &Settings, raw: Option<&Value>) -> View {
    let mut out = View { on: raw.is_some(), failures: failures(), ..Default::default() };
    out.n_block = out.failures.iter().filter(|f| f.route == "block").count();
    let Some(raw) = raw else { return out };
    let conns = parse(s, raw);

    // Скорость — по разнице байт с прошлого замера.
    let now = Instant::now();
    let mut prev = PREV.lock().unwrap();
    let dt = prev.0.map(|t| now.duration_since(t).as_secs_f64()).filter(|d| *d > 0.2);
    let speed_of = |c: &Conn| -> u64 {
        match (dt, prev.1.get(&c.id)) {
            (Some(dt), Some(before)) => ((c.bytes.saturating_sub(*before)) as f64 / dt) as u64,
            _ => 0,
        }
    };

    let mut groups: Vec<AppRow> = vec![];
    let mut hosts_vpn = std::collections::HashSet::new();
    let mut hosts_direct = std::collections::HashSet::new();
    for c in &conns {
        if c.route == "vpn" { hosts_vpn.insert(c.host.clone()); } else { hosts_direct.insert(c.host.clone()); }
        let key = c.exe.to_lowercase();
        let i = match groups.iter().position(|g| g.exe.to_lowercase() == key) {
            Some(i) => i,
            None => {
                groups.push(AppRow { exe: c.exe.clone(), name: display_name(&c.path, &c.exe), path: c.path.clone(), route: c.route, conns: 0, speed: 0, why: String::new(), hosts: vec![] });
                groups.len() - 1
            }
        };
        let g = &mut groups[i];
        g.conns += 1;
        g.speed += speed_of(c);
        if g.route != c.route {
            g.route = "mixed";
        }
        match g.hosts.iter_mut().find(|h| h.host == c.host && h.route == c.route) {
            Some(h) => h.count += 1,
            None => g.hosts.push(HostRow { host: c.host.clone(), route: c.route, why: c.why.clone(), count: 1 }),
        }
        if c.by_program && g.why.is_empty() {
            g.why = if is_ks(s, &c.exe) {
                "Только через VPN (Kill Switch): без VPN — без сети.".into()
            } else {
                format!("Правило программы: {}.", if c.route == "vpn" { "через VPN" } else { "напрямую" })
            };
        }
    }
    for g in &mut groups {
        if is_ks(s, &g.exe) {
            g.route = "ks";
        }
        if g.why.is_empty() {
            g.why = "Своего правила нет — каждый адрес идёт по своему правилу.".into();
        }
        g.hosts.sort_by(|a, b| b.count.cmp(&a.count));
    }
    groups.sort_by(|a, b| b.speed.cmp(&a.speed).then(b.conns.cmp(&a.conns)));
    *prev = (Some(now), conns.iter().map(|c| (c.id.clone(), c.bytes)).collect());
    out.n_vpn = hosts_vpn.len();
    out.n_direct = hosts_direct.len();
    out.apps = groups;
    out
}

// ─────────── Не открывается? ───────────

struct Failed {
    host: String,
    app: String,
    route: &'static str,
    why: String,
    at: Instant,
}

static FAILED: Lazy<Mutex<VecDeque<Failed>>> = Lazy::new(|| Mutex::new(VecDeque::new()));
const KEEP: Duration = Duration::from_secs(600);

/// Причина неудачи из текста ошибки mihomo — коротко и по-русски.
fn reason(err: &str) -> String {
    let e = err.to_lowercase();
    if e.contains("timeout") || e.contains("deadline") {
        "не ответил вовремя".into()
    } else if e.contains("refused") {
        "в соединении отказано".into()
    } else if e.contains("reset") || e.contains("forcibly closed") || e.contains("eof") {
        "соединение сброшено".into()
    } else if e.contains("dns") || e.contains("no such host") || e.contains("resolve") {
        "не нашёлся адрес (DNS)".into()
    } else if e.contains("unreachable") {
        "адрес недоступен".into()
    } else {
        let short: String = err.trim().chars().take(80).collect();
        format!("ошибка: {short}")
    }
}

/// «127.0.0.1:50165(curl.exe)» → «curl.exe».
fn proc_of(src: &str) -> String {
    src.split_once('(').map(|(_, p)| p.trim_end_matches(')').to_string()).unwrap_or_default()
}

/// Строка лога mihomo → неудача, если это она:
/// `[TCP] dial PROXY (match Match/) 127.0.0.1:1(app.exe) --> host:443 error: …`
/// `[TCP] 127.0.0.1:1(app.exe) --> host:443 match DomainSuffix(x) using REJECT`
pub fn parse_failure(text: &str) -> Option<(String, String, &'static str, String)> {
    let t = text.trim();
    let body = t.strip_prefix("[TCP] ").or_else(|| t.strip_prefix("[UDP] "))?;
    if let Some(rest) = body.strip_prefix("dial ") {
        let (target, rest) = rest.split_once(' ')?;
        let rest = rest.split_once(") ").map(|(_, r)| r)?;
        let (src, rest) = rest.split_once(" --> ")?;
        let (dst, err) = rest.split_once(" error: ")?;
        let host = dst.rsplit_once(':').map(|(h, _)| h).unwrap_or(dst).trim_start_matches("www.").to_string();
        let route = if target == "DIRECT" { "direct" } else { "vpn" };
        return Some((host, proc_of(src), route, reason(err)));
    }
    if body.ends_with("using REJECT") || body.ends_with("using REJECT-DROP") {
        let (src, rest) = body.split_once(" --> ")?;
        let dst = rest.split(' ').next()?;
        let host = dst.rsplit_once(':').map(|(h, _)| h).unwrap_or(dst).trim_start_matches("www.").to_string();
        return Some((host, proc_of(src), "block", "заблокировано вашим правилом".into()));
    }
    None
}

/// Каждая строка лога ядра проходит здесь.
pub fn on_log(text: &str) {
    let Some((host, app, route, why)) = parse_failure(text) else { return };
    // Своё (замер, скачивание наборов) — не про пользователя.
    if app.eq_ignore_ascii_case("mihomo.exe") || app.eq_ignore_ascii_case("klick.exe") {
        return;
    }
    let mut f = FAILED.lock().unwrap();
    f.retain(|x| !(x.host == host && x.app == app) && x.at.elapsed() < KEEP);
    f.push_back(Failed { host, app, route, why, at: Instant::now() });
    while f.len() > 50 {
        f.pop_front();
    }
}

pub fn failures() -> Vec<Failure> {
    let f = FAILED.lock().unwrap();
    f.iter()
        .rev()
        .filter(|x| x.at.elapsed() < KEEP)
        .take(8)
        .map(|x| Failure { host: x.host.clone(), app: x.app.clone(), route: x.route, why: x.why.clone(), ago: x.at.elapsed().as_secs() })
        .collect()
}

pub fn clear_failures() {
    FAILED.lock().unwrap().clear();
}

// ─────────── Как тебя видят сайты ───────────

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpInfo {
    pub ip: String,
    pub place: String,
    pub org: String,
    /// Похоже на адрес хостинга (по названию сети) — сайты могут понять, что это VPN.
    pub hosting: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpView {
    pub vpn: Option<IpInfo>,
    pub vpn_error: Option<String>,
    pub direct: Option<IpInfo>,
    pub direct_error: Option<String>,
    /// IPv6-адрес, если компьютер ходит по IPv6 напрямую (мимо VPN).
    pub ipv6: Option<String>,
}

fn looks_hosting(org: &str) -> bool {
    let o = org.to_lowercase();
    ["hosting", "host", "cloud", "server", "vps", "data center", "datacenter", "digitalocean", "hetzner", "ovh", "amazon", "aws", "linode", "vultr", "contabo", "m247", "leaseweb", "choopa", "colocation", "aeza", "timeweb", "selectel"]
        .iter()
        .any(|k| o.contains(k))
}

/// Запрос через системный curl (ureq в kl!ck собран без TLS). `proxy` —
/// порт ядра: через него запрос идёт через VPN. Без него — мимо системного
/// прокси и переменных окружения.
fn get(url: &str, proxy: Option<u16>) -> Result<String, String> {
    let mut cmd = crate::sys::command("curl.exe");
    cmd.args(["-sS", "--fail", "--max-time", "8", "--proto", "=https", "-A", "klick"]);
    match proxy {
        Some(port) => cmd.args(["-x", &format!("http://127.0.0.1:{port}")]),
        None => cmd.args(["--noproxy", "*"]),
    };
    let out = cmd.arg(url).output().map_err(|e| format!("не удалось запустить curl: {e}"))?;
    if !out.status.success() {
        let err = crate::sys::decode_console(&out.stderr);
        return Err(err.trim().trim_start_matches("curl: ").to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn json(url: &str, proxy: Option<u16>) -> Result<Value, String> {
    serde_json::from_str(&get(url, proxy)?).map_err(|_| "ответил непонятно".to_string())
}

fn via_vpn(port: u16) -> Result<IpInfo, String> {
    let v = json(&format!("https://{IP_VPN_HOST}/json"), Some(port))?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    // org: «AS210546 CHSL ONE LTD» — номер сети не нужен.
    let org = s("org").split_once(' ').map(|(_, n)| n.to_string()).unwrap_or_else(|| s("org"));
    Ok(IpInfo { ip: s("ip"), place: [s("country"), s("city")].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(", "), hosting: looks_hosting(&org), org })
}

fn direct() -> Result<IpInfo, String> {
    let v = json(&format!("https://{IP_DIRECT_HOST}/"), None)?;
    let s = |p: &str| v.pointer(p).and_then(Value::as_str).unwrap_or("").to_string();
    let org = if s("/connection/org").is_empty() { s("/connection/isp") } else { s("/connection/org") };
    Ok(IpInfo { ip: s("/ip"), place: [s("/country_code"), s("/city")].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(", "), hosting: looks_hosting(&org), org })
}

fn ipv6() -> Option<String> {
    let t = get("https://api6.ipify.org", None).ok()?;
    let t = t.trim().to_string();
    t.contains(':').then_some(t)
}

/// `proxy_port` — порт ядра, если VPN включён.
pub fn check_ip(proxy_port: Option<u16>) -> IpView {
    std::thread::scope(|sc| {
        let v = sc.spawn(|| proxy_port.map(via_vpn));
        let d = sc.spawn(direct);
        let six = sc.spawn(ipv6);
        let v = v.join().ok().flatten();
        let d = d.join().unwrap_or_else(|_| Err("не получилось".into()));
        let (vpn, vpn_error) = match v {
            Some(Ok(i)) => (Some(i), None),
            Some(Err(e)) => (None, Some(e)),
            None => (None, None),
        };
        let (direct, direct_error) = match d {
            Ok(i) => (Some(i), None),
            Err(e) => (None, Some(e)),
        };
        IpView { vpn, vpn_error, direct, direct_error, ipv6: six.join().ok().flatten() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Настоящие сервисы: cargo test --lib ip_вживую -- --ignored --nocapture
    #[test]
    #[ignore]
    fn ip_вживую() {
        let v = check_ip(None);
        let d = v.direct.unwrap_or_else(|| panic!("ipwho.is: {:?}", v.direct_error));
        println!("напрямую: страна/город «{}», сеть «{}», хостинг {}, ip {}", d.place, d.org, d.hosting, if d.ip.is_empty() { "нет" } else { "есть" });
        let i = json(&format!("https://{IP_VPN_HOST}/json"), None).unwrap();
        println!("ipinfo без прокси: страна «{}», сеть «{}»", i["country"], i["org"]);
    }

    #[test]
    fn неудачи_из_лога() {
        let (h, a, r, w) = parse_failure("[TCP] dial PROXY (match DomainSuffix/youtube.com) 127.0.0.1:60494(curl.exe) --> www.youtube.com:443 error: dial tcp: i/o timeout").unwrap();
        assert_eq!((h.as_str(), a.as_str(), r, w.as_str()), ("youtube.com", "curl.exe", "vpn", "не ответил вовремя"));
        let (h, a, r, _) = parse_failure("[TCP] 127.0.0.1:60435(chrome.exe) --> ads.example.net:443 match DomainSuffix(ads.example.net) using REJECT").unwrap();
        assert_eq!((h.as_str(), a.as_str(), r), ("ads.example.net", "chrome.exe", "block"));
        let (_, _, r, w) = parse_failure("[UDP] dial DIRECT (match Match/) 127.0.0.1:5(game.exe) --> 1.2.3.4:27015 error: connection refused").unwrap();
        assert_eq!((r, w.as_str()), ("direct", "в соединении отказано"));
        assert!(parse_failure("[TCP] 127.0.0.1:1(x.exe) --> ya.ru:443 match Match using DIRECT").is_none());
        assert!(parse_failure("Start initial configuration").is_none());
    }

    #[test]
    fn соединения_по_программам() {
        let s = Settings { routing: true, ..Default::default() };
        let raw: Value = serde_json::json!({"connections": [
            {"id": "1", "upload": 10, "download": 100, "rule": "Match", "rulePayload": "", "chains": ["NL", "PROXY"],
             "metadata": {"process": "Discord.exe", "processPath": "", "host": "gateway.discord.gg", "network": "tcp", "destinationIP": "1.1.1.1"}},
            {"id": "2", "upload": 1, "download": 1, "rule": "DomainSuffix", "rulePayload": "ru", "chains": ["DIRECT"],
             "metadata": {"process": "chrome.exe", "processPath": "", "host": "www.ozon.ru", "network": "tcp", "destinationIP": "2.2.2.2"}},
            {"id": "3", "upload": 1, "download": 1, "rule": "Match", "rulePayload": "", "chains": ["NL", "PROXY"],
             "metadata": {"process": "chrome.exe", "processPath": "", "host": "", "sniffHost": "", "network": "udp", "destinationIP": "3.3.3.3"}},
            {"id": "4", "upload": 1, "download": 1, "rule": "ProcessPathRegex", "rulePayload": "x", "chains": ["DIRECT"],
             "metadata": {"process": "mihomo.exe", "processPath": "", "host": "t.example", "network": "tcp", "destinationIP": "4.4.4.4"}}
        ]});
        let v = view(&s, Some(&raw));
        assert_eq!(v.apps.len(), 2, "зонд kl!ck не показываем");
        let chrome = v.apps.iter().find(|a| a.exe == "chrome.exe").unwrap();
        assert_eq!(chrome.route, "mixed");
        assert!(chrome.hosts.iter().any(|h| h.host == "ozon.ru" && h.why == "набор «Россия напрямую»"));
        assert!(chrome.hosts.iter().any(|h| h.host == "3.3.3.3 · UDP"));
        assert_eq!((v.n_vpn, v.n_direct), (2, 1));
    }
}
