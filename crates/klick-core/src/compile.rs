//! Сборщик конфигурации: режим, тумблер, списки, Kill Switch и серверы → конфиг mihomo.
//!
//! Конфиг пишется в JSON: mihomo читает его как YAML, JSON — подмножество YAML 1.2.
//! Порядок правил: локальная сеть → Kill Switch → программы → сервисы, домены, IP →
//! готовые наборы → всё остальное. Срабатывает первое подходящее правило.

use crate::model::{Catalog, Route, Routing, Rule, Settings, Target};
use serde_json::{json, Map, Value};

/// Группа, через которую идёт всё «через VPN». Сервер в ней выбирается через API ядра.
pub const VPN_GROUP: &str = "klick-vpn";
/// Провайдер серверов активного подключения.
pub const PROVIDER: &str = "klick-servers";
/// Адрес для проверки связи через сервер.
pub const PROBE_URL: &str = "https://www.gstatic.com/generate_204";

pub const SET_BLOCKED_DOMAINS: &str = "klick-blocked-domains";
pub const SET_BLOCKED_IPS: &str = "klick-blocked-ips";
pub const SET_RU_DOMAINS: &str = "klick-ru-domains";

/// Яндекс по голому IP DoH не отдаёт (пустой ответ, проверено), а DoT — да.
/// Обычный UDP — запасной и самый быстрый: ядро опрашивает все сразу.
const DNS_RU: [&str; 2] = ["tls://77.88.8.8:853", "77.88.8.8"];
const DNS_FOREIGN: [&str; 2] = ["https://1.1.1.1/dns-query", "https://8.8.8.8/dns-query"];

/// Как трафик попадает в ядро.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    /// Адаптер TUN плюс порт.
    Tun,
    /// Только порт: режим системного прокси.
    Proxy,
}

/// Имена и порты, которые выбирает служба.
#[derive(Clone, Debug)]
pub struct CoreLayout {
    pub controller_pipe: String,
    pub secret: String,
    pub mixed_port: u16,
    /// Служебный вход «через VPN» для проверки IP и скачиваний.
    pub check_vpn_port: u16,
    /// Служебный вход «напрямую».
    pub check_direct_port: u16,
    pub tun_device: String,
    pub log_level: String,
    /// Путь к mihomo.exe: соединения проверочного ядра (замер задержки при
    /// другом подключении) идут напрямую, а не через текущий туннель.
    pub core_exe: String,
}

/// Файлы готовых наборов относительно домашней папки ядра.
#[derive(Clone, Debug)]
pub struct SetFiles {
    pub blocked_domains: String,
    pub blocked_ips: String,
    pub ru_domains: String,
}

pub struct CompileInput<'a> {
    pub settings: &'a Settings,
    pub catalog: &'a Catalog,
    pub layout: &'a CoreLayout,
    pub capture: Capture,
    /// Файл серверов активного подключения относительно домашней папки ядра.
    pub provider_path: &'a str,
    pub sets: &'a SetFiles,
}

/// Полный конфиг для работающего VPN.
pub fn compile(input: &CompileInput) -> Value {
    let s = input.settings;
    let layout = input.layout;
    let mut cfg = base(layout, input.provider_path);

    cfg.insert("mixed-port".into(), json!(layout.mixed_port));
    cfg.insert("allow-lan".into(), json!(false));
    cfg.insert("bind-address".into(), json!("127.0.0.1"));
    cfg.insert(
        "listeners".into(),
        json!([
            { "name": "klick-check-vpn", "type": "http", "port": layout.check_vpn_port, "listen": "127.0.0.1", "proxy": VPN_GROUP },
            { "name": "klick-check-direct", "type": "http", "port": layout.check_direct_port, "listen": "127.0.0.1", "proxy": "DIRECT" },
        ]),
    );
    cfg.insert("rule-providers".into(), rule_providers(s, input.sets));
    let mut all = Vec::new();
    // Проверочное ядро — тот же mihomo.exe. Его соединения в TUN попали бы в
    // туннель основного, и задержка «другого» подключения мерилась бы через
    // текущий сервер. Собственные соединения основного ядра в TUN не входят.
    let exe = layout.core_exe.trim_start_matches(r"\\?\");
    if !exe.is_empty() {
        all.push(format!("PROCESS-PATH-REGEX,{},DIRECT", exact_path_regex(exe)));
    }
    all.extend(rules(s, input.catalog));
    cfg.insert("rules".into(), Value::from(all));
    cfg.insert("dns".into(), dns(s, input.catalog));
    cfg.insert("sniffer".into(), sniffer());
    if input.capture == Capture::Tun {
        cfg.insert(
            "tun".into(),
            json!({
                "enable": true,
                "stack": "gvisor",
                "device": layout.tun_device,
                "auto-route": true,
                "auto-detect-interface": true,
                "strict-route": true,
                "dns-hijack": ["any:53", "tcp://any:53"],
            }),
        );
    }
    Value::Object(cfg)
}

/// Проверочное ядро: только серверы и управление, без портов и перехвата.
/// Нужно, чтобы мерить задержку и читать список серверов при выключенном VPN.
pub fn compile_tester(layout: &CoreLayout, provider_path: &str) -> Value {
    let mut cfg = base(layout, provider_path);
    cfg.insert("rules".into(), json!(["MATCH,DIRECT"]));
    Value::Object(cfg)
}

fn base(layout: &CoreLayout, provider_path: &str) -> Map<String, Value> {
    let v = json!({
        "mode": "rule",
        "log-level": layout.log_level,
        "ipv6": true,
        "unified-delay": true,
        "tcp-concurrent": true,
        "find-process-mode": "always",
        "geo-auto-update": false,
        "geodata-mode": false,
        "external-controller-pipe": layout.controller_pipe,
        "secret": layout.secret,
        "profile": { "store-selected": false, "store-fake-ip": true },
        "proxy-providers": {
            PROVIDER: { "type": "file", "path": provider_path, "health-check": { "enable": false } }
        },
        "proxy-groups": [
            { "name": VPN_GROUP, "type": "select", "use": [PROVIDER] }
        ],
    });
    match v {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

fn rule_providers(s: &Settings, sets: &SetFiles) -> Value {
    let file = |behavior: &str, path: &str| json!({ "type": "file", "behavior": behavior, "format": "text", "path": path });
    match s.routing {
        Routing::Selected if s.blocked_preset => json!({
            SET_BLOCKED_DOMAINS: file("domain", &sets.blocked_domains),
            SET_BLOCKED_IPS: file("ipcidr", &sets.blocked_ips),
        }),
        Routing::Selected => json!({}),
        Routing::AllVpn if s.russia_direct.domains => json!({
            SET_RU_DOMAINS: file("domain", &sets.ru_domains),
        }),
        Routing::AllVpn => json!({}),
    }
}

fn policy(route: Route) -> &'static str {
    match route {
        Route::Vpn => VPN_GROUP,
        Route::Direct => "DIRECT",
        Route::Block => "REJECT",
    }
}

/// Регулярное выражение для всех файлов внутри папки программы.
/// Запятая разделяет части правила mihomo, поэтому она кодируется как `\x2c`.
/// У программ из Microsoft Store версия зашита в имя папки пакета
/// (`Claude_2.9939.2.0_x64__pzs8sxrjxfjjc`): её заменяем шаблоном, чтобы правило пережило обновление.
pub fn folder_regex(folder: &str) -> String {
    let mut re = String::from("(?i)^");
    for (i, segment) in folder.trim_end_matches('\\').split('\\').enumerate() {
        if i > 0 {
            re.push_str("\\\\");
        }
        match msix_package(segment) {
            Some((name, publisher)) => {
                push_escaped(&mut re, name);
                re.push_str("_[^\\\\]+__");
                push_escaped(&mut re, publisher);
            }
            None => push_escaped(&mut re, segment),
        }
    }
    re.push_str("\\\\.+$");
    re
}

/// Ровно этот путь, без учёта регистра.
pub fn exact_path_regex(path: &str) -> String {
    let mut re = String::from("(?i)^");
    push_escaped(&mut re, path);
    re.push('$');
    re
}

fn push_escaped(re: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                re.push('\\');
                re.push(ch);
            }
            ',' => re.push_str("\\x2c"),
            _ => re.push(ch),
        }
    }
}

/// Папка пакета Store `Имя_Версия_Архитектура__Издатель` → (имя, издатель).
fn msix_package(segment: &str) -> Option<(&str, &str)> {
    let (head, publisher) = segment.rsplit_once("__")?;
    if publisher.is_empty() || !publisher.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let (rest, arch) = head.rsplit_once('_')?;
    let (name, version) = rest.rsplit_once('_')?;
    let arch_ok = ["x64", "x86", "arm64", "arm", "neutral"].iter().any(|a| a.eq_ignore_ascii_case(arch));
    let version_ok = version.contains('.') && version.chars().all(|c| c.is_ascii_digit() || c == '.');
    (arch_ok && version_ok && !name.is_empty()).then_some((name, publisher))
}

fn ip_rule(cidr: &str, target: &str) -> String {
    let kind = if cidr.contains(':') { "IP-CIDR6" } else { "IP-CIDR" };
    format!("{kind},{cidr},{target},no-resolve")
}

const LAN_V4: [&str; 8] = [
    "127.0.0.0/8",
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "169.254.0.0/16",
    "100.64.0.0/10",
    "224.0.0.0/4",
    "255.255.255.255/32",
];
const LAN_V6: [&str; 4] = ["::1/128", "fc00::/7", "fe80::/10", "ff00::/8"];

pub fn rules(s: &Settings, catalog: &Catalog) -> Vec<String> {
    let mut out = Vec::new();

    for cidr in LAN_V4.iter().chain(LAN_V6.iter()) {
        out.push(ip_rule(cidr, "DIRECT"));
    }
    for zone in ["local", "lan", "localhost"] {
        out.push(format!("DOMAIN-SUFFIX,{zone},DIRECT"));
    }

    if s.kill_switch.enabled {
        for p in s.kill_switch.programs.iter().filter(|p| p.enabled) {
            out.push(format!("PROCESS-PATH-REGEX,{},{VPN_GROUP}", folder_regex(&p.folder)));
        }
    }

    // Ядро берёт первое совпавшее правило, поэтому внутри списка точное идёт раньше общего:
    // папка поглубже, домен подлиннее, подсеть поуже. При равной точности свои сайты и подсети —
    // раньше сервисов каталога. Сортировка устойчивая: в остальном порядок как в списке.
    let list: Vec<&Rule> = s.rules().collect();
    let mut programs: Vec<(&str, Route)> = list
        .iter()
        .filter_map(|r| match &r.target {
            Target::Program(folder) => Some((folder.as_str(), r.route)),
            _ => None,
        })
        .collect();
    programs.sort_by_key(|(folder, _)| std::cmp::Reverse(folder.trim_end_matches('\\').split('\\').count()));
    for (folder, route) in programs {
        out.push(format!("PROCESS-PATH-REGEX,{},{}", folder_regex(folder), policy(route)));
    }
    let (mut domains, mut ips) = (Vec::new(), Vec::new());
    for rule in &list {
        collect_targets(rule, catalog, &mut domains, &mut ips);
    }
    domains.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    ips.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, _, domain, route) in domains {
        out.push(format!("DOMAIN-SUFFIX,{domain},{}", policy(route)));
    }
    for (_, _, cidr, route) in ips {
        out.push(ip_rule(&cidr, policy(route)));
    }

    match s.routing {
        Routing::Selected => {
            if s.blocked_preset {
                out.push(format!("RULE-SET,{SET_BLOCKED_DOMAINS},{VPN_GROUP}"));
                out.push(format!("RULE-SET,{SET_BLOCKED_IPS},{VPN_GROUP},no-resolve"));
            }
            out.push("MATCH,DIRECT".into());
        }
        Routing::AllVpn => {
            if s.russia_direct.domains {
                out.push(format!("RULE-SET,{SET_RU_DOMAINS},DIRECT"));
            }
            if s.russia_direct.ips {
                out.push("GEOIP,RU,DIRECT".into());
            }
            out.push(format!("MATCH,{VPN_GROUP}"));
        }
    }
    out
}

/// Домен или подсеть правила с точностью: (частей домена или длина префикса, 0 — своё / 1 — из сервиса, значение, маршрут).
type Ranked = (usize, u8, String, Route);

fn collect_targets(rule: &Rule, catalog: &Catalog, domains: &mut Vec<Ranked>, ips: &mut Vec<Ranked>) {
    let labels = |d: &str| d.split('.').count();
    let prefix = |c: &str| c.rsplit_once('/').and_then(|(_, p)| p.parse().ok()).unwrap_or(0);
    match &rule.target {
        Target::Domain(d) => domains.push((labels(d), 0, d.clone(), rule.route)),
        Target::Ip(c) => ips.push((prefix(c), 0, c.clone(), rule.route)),
        Target::Service(id) => {
            if let Some(service) = catalog.find(id) {
                domains.extend(service.domains.iter().map(|d| (labels(d), 1, d.clone(), rule.route)));
                ips.extend(service.cidrs.iter().map(|c| (prefix(c), 1, c.clone(), rule.route)));
            }
        }
        Target::Program(_) => {}
    }
}

/// Домены из списка пользователя, которые идут через VPN: их имена тоже спрашиваем через VPN.
fn vpn_domains(s: &Settings, catalog: &Catalog) -> Vec<String> {
    let mut out = Vec::new();
    for rule in s.rules().filter(|r| r.route == Route::Vpn) {
        match &rule.target {
            Target::Domain(d) => out.push(d.clone()),
            Target::Service(id) => {
                if let Some(service) = catalog.find(id) {
                    out.extend(service.domains.iter().cloned());
                }
            }
            _ => {}
        }
    }
    out
}

fn dns(s: &Settings, catalog: &Catalog) -> Value {
    let foreign: Vec<String> = DNS_FOREIGN.iter().map(|u| format!("{u}#{VPN_GROUP}")).collect();
    let ru: Vec<String> = DNS_RU.iter().map(|u| u.to_string()).collect();
    let mut policy = Map::new();
    let nameserver = match s.routing {
        Routing::Selected => {
            if s.blocked_preset {
                policy.insert(format!("rule-set:{SET_BLOCKED_DOMAINS}"), json!(foreign));
            }
            for d in vpn_domains(s, catalog) {
                policy.insert(format!("+.{d}"), json!(foreign));
            }
            ru.clone()
        }
        Routing::AllVpn => {
            if s.russia_direct.domains {
                policy.insert(format!("rule-set:{SET_RU_DOMAINS}"), json!(ru));
            }
            foreign
        }
    };
    json!({
        "enable": true,
        "ipv6": false,
        "enhanced-mode": "fake-ip",
        "fake-ip-range": "198.18.0.1/16",
        "fake-ip-filter": [
            "*.lan", "*.local", "*.localdomain", "localhost",
            "+.msftconnecttest.com", "+.msftncsi.com",
            "time.windows.com", "time.nist.gov", "+.pool.ntp.org", "+.time.edu.cn",
            "+.stun.*.*", "+.stun.*.*.*", "stun.l.google.com"
        ],
        "default-nameserver": ["77.88.8.8", "1.1.1.1"],
        "nameserver": nameserver,
        "nameserver-policy": policy,
        "direct-nameserver": ru,
        // Адреса самих серверов — напрямую. Обычный UDP — запасной на случай,
        // когда DoT и DoH в сети закрыты: иначе сервер, заданный доменом, не найдётся.
        "proxy-server-nameserver": ["tls://77.88.8.8:853", "https://1.1.1.1/dns-query", "77.88.8.8"],
    })
}

fn sniffer() -> Value {
    json!({
        "enable": true,
        "force-dns-mapping": true,
        "parse-pure-ip": true,
        "override-destination": false,
        // TLS и QUIC — с подменой адреса на имя сайта. С `ipv6: true` туннель забирает и
        // IPv6, а браузер со своим DoH (Chrome, Edge при DNS 1.1.1.1) идёт на IPv6-адрес сайта:
        // сервер без IPv6 его не откроет, и страница висит — TUN уже принял соединение, и браузер
        // не откатывается на IPv4. Имя из SNI уходит на сервер вместо адреса, сервер находит сайт
        // сам. Утечки нет: IPv6 без имени (пиры торрентов, звонки) просто не пройдёт мимо VPN.
        "sniff": {
            "TLS": { "ports": [443, 8443], "override-destination": true },
            "HTTP": { "ports": [80, "8080-8880"], "override-destination": true },
            "QUIC": { "ports": [443, 8443], "override-destination": true }
        },
        "skip-domain": ["+.push.apple.com"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Rule, Service};

    fn layout() -> CoreLayout {
        CoreLayout {
            controller_pipe: r"\\.\pipe\klick-core".into(),
            secret: "s".into(),
            mixed_port: 7890,
            check_vpn_port: 17891,
            check_direct_port: 17892,
            tun_device: "klick".into(),
            log_level: "warning".into(),
            core_exe: r"C:\Program Files\kl!ck\resources\core\mihomo.exe".into(),
        }
    }

    fn sets() -> SetFiles {
        SetFiles {
            blocked_domains: "sets/blocked-domains.txt".into(),
            blocked_ips: "sets/blocked-ips.txt".into(),
            ru_domains: "sets/ru-domains.txt".into(),
        }
    }

    fn catalog() -> Catalog {
        Catalog {
            services: vec![Service {
                id: "telegram".into(),
                name: "Telegram".into(),
                domains: vec!["telegram.org".into(), "t.me".into()],
                cidrs: vec!["149.154.160.0/20".into(), "2001:b28:f23d::/48".into()],
            }],
        }
    }

    fn position(rules: &[String], needle: &str) -> usize {
        rules.iter().position(|r| r.contains(needle)).unwrap_or_else(|| panic!("нет правила {needle}"))
    }

    #[test]
    fn folder_regex_survives_store_updates() {
        assert_eq!(
            folder_regex(r"C:\Program Files\WindowsApps\Claude_2.9939.2.0_x64__pzs8sxrjxfjjc\app"),
            r"(?i)^C:\\Program Files\\WindowsApps\\Claude_[^\\]+__pzs8sxrjxfjjc\\app\\.+$"
        );
        assert_eq!(folder_regex(r"C:\Games\My_Game_1.0_final"), r"(?i)^C:\\Games\\My_Game_1\.0_final\\.+$");
    }

    #[test]
    fn folder_regex_escapes_windows_paths() {
        assert_eq!(
            folder_regex(r"C:\Program Files (x86)\Game, Inc"),
            r"(?i)^C:\\Program Files \(x86\)\\Game\x2c Inc\\.+$"
        );
    }

    #[test]
    fn selected_order_follows_the_ladder() {
        let mut s = Settings::default();
        s.kill_switch.programs.push(r"C:\Games\Roblox".into());
        s.lists.selected = vec![
            Rule { target: Target::Service("telegram".into()), route: Route::Vpn, enabled: true },
            Rule { target: Target::Program(r"C:\Apps\Discord".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: true },
        ];
        let r = rules(&s, &catalog());
        let lan = position(&r, "192.168.0.0/16");
        let ks = position(&r, "Roblox");
        let program = position(&r, "Discord");
        let domain = position(&r, "claude.ai");
        let service = position(&r, "telegram.org");
        let service_ip = position(&r, "149.154.160.0/20");
        let sets = position(&r, SET_BLOCKED_DOMAINS);
        assert!(lan < ks && ks < program && program < domain && domain < service && service < service_ip && service_ip < sets);
        assert_eq!(r.last().unwrap(), "MATCH,DIRECT");
        assert!(r.contains(&"IP-CIDR6,2001:b28:f23d::/48,klick-vpn,no-resolve".to_string()));
        assert!(r[ks].ends_with(",klick-vpn"));
        assert!(r[program].ends_with(",DIRECT"));
    }

    #[test]
    fn precise_rules_win_over_broad_ones() {
        let mut s = Settings::default();
        s.lists.selected = vec![
            Rule { target: Target::Domain("ru".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Service("telegram".into()), route: Route::Vpn, enabled: true },
            Rule { target: Target::Program(r"C:\Games".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Domain("kinopoisk.ru".into()), route: Route::Vpn, enabled: true },
            Rule { target: Target::Domain("telegram.org".into()), route: Route::Block, enabled: true },
            Rule { target: Target::Ip("149.154.160.0/22".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Program(r"C:\Games\Roblox".into()), route: Route::Vpn, enabled: true },
        ];
        let r = rules(&s, &catalog());
        assert!(position(&r, "Games\\\\Roblox") < position(&r, "Games\\\\.+"), "вложенная папка раньше общей");
        assert!(position(&r, "DOMAIN-SUFFIX,kinopoisk.ru") < position(&r, "DOMAIN-SUFFIX,ru,"), "сайт раньше зоны");
        assert_eq!(r[position(&r, "DOMAIN-SUFFIX,telegram.org")], "DOMAIN-SUFFIX,telegram.org,REJECT", "свой сайт раньше сервиса");
        assert!(position(&r, "149.154.160.0/22") < position(&r, "149.154.160.0/20"), "узкая подсеть раньше широкой");
    }

    #[test]
    fn all_vpn_ends_with_russia_and_vpn() {
        let mut s = Settings::default();
        s.routing = Routing::AllVpn;
        s.lists.selected.push(Rule { target: Target::Domain("youtube.com".into()), route: Route::Vpn, enabled: true });
        s.lists.all_vpn.push(Rule { target: Target::Domain("sber.ru".into()), route: Route::Direct, enabled: true });
        let r = rules(&s, &catalog());
        assert!(r.iter().any(|x| x == "DOMAIN-SUFFIX,sber.ru,DIRECT"));
        assert!(!r.iter().any(|x| x.contains("youtube.com")), "список другого положения не действует");
        let n = r.len();
        assert_eq!(r[n - 3], format!("RULE-SET,{SET_RU_DOMAINS},DIRECT"));
        assert_eq!(r[n - 2], "GEOIP,RU,DIRECT");
        assert_eq!(r[n - 1], "MATCH,klick-vpn");
    }

    #[test]
    fn russia_switches_control_direct_rules() {
        let mut s = Settings::default();
        s.routing = Routing::AllVpn;
        s.russia_direct.domains = false;
        let r = rules(&s, &catalog());
        assert!(!r.iter().any(|x| x.contains(SET_RU_DOMAINS)));
        assert!(r.contains(&"GEOIP,RU,DIRECT".to_string()));
        assert_eq!(rule_providers(&s, &sets()), json!({}));
        assert!(dns(&s, &catalog())["nameserver-policy"].as_object().unwrap().is_empty());

        s.russia_direct.ips = false;
        let r = rules(&s, &catalog());
        assert!(!r.iter().any(|x| x.starts_with("GEOIP")));
        assert!(r.iter().any(|x| x == "IP-CIDR,192.168.0.0/16,DIRECT,no-resolve"), "локальная сеть напрямую всегда");
        assert_eq!(r.last().unwrap(), "MATCH,klick-vpn");
    }

    #[test]
    fn disabled_kill_switch_adds_no_rules() {
        let mut s = Settings::default();
        s.kill_switch.enabled = false;
        s.kill_switch.programs.push(r"C:\Games\Roblox".into());
        assert!(!rules(&s, &catalog()).iter().any(|r| r.contains("Roblox")));
        s.kill_switch.enabled = true;
        s.kill_switch.programs[0].enabled = false;
        assert!(!rules(&s, &catalog()).iter().any(|r| r.contains("Roblox")), "выключенная программа не защищается");
    }

    #[test]
    fn tun_section_only_in_tun_capture() {
        let s = Settings::default();
        let (c, l, st) = (catalog(), layout(), sets());
        let input = |capture| CompileInput { settings: &s, catalog: &c, layout: &l, capture, provider_path: "providers/a.txt", sets: &st };
        let tun = compile(&input(Capture::Tun));
        let proxy = compile(&input(Capture::Proxy));
        assert_eq!(tun["tun"]["device"], "klick");
        assert!(proxy.get("tun").is_none());
        assert_eq!(proxy["mixed-port"], 7890);
        assert_eq!(proxy["bind-address"], "127.0.0.1");
        assert_eq!(proxy["listeners"][0]["proxy"], VPN_GROUP);
        assert_eq!(proxy["proxy-providers"][PROVIDER]["path"], "providers/a.txt");
    }

    #[test]
    fn tester_core_goes_direct_not_through_tunnel() {
        let s = Settings::default();
        let (c, mut l, st) = (catalog(), layout(), sets());
        l.core_exe = r"\\?\C:\Program Files\kl!ck\resources\core\mihomo.exe".into();
        let cfg = compile(&CompileInput { settings: &s, catalog: &c, layout: &l, capture: Capture::Tun, provider_path: "providers/a.txt", sets: &st });
        assert_eq!(cfg["rules"][0], r"PROCESS-PATH-REGEX,(?i)^C:\\Program Files\\kl!ck\\resources\\core\\mihomo\.exe$,DIRECT");
        let d = &cfg["dns"];
        assert_eq!(d["proxy-server-nameserver"][2], "77.88.8.8", "запасной UDP для адресов серверов");
        // IPv6 в туннеле: TLS и QUIC уходят на сервер по имени сайта, не по IPv6-адресу.
        assert_eq!(cfg["ipv6"], true);
        assert_eq!(cfg["sniffer"]["sniff"]["TLS"]["override-destination"], true);
        assert_eq!(cfg["sniffer"]["sniff"]["QUIC"]["override-destination"], true);
    }

    #[test]
    fn dns_asks_blocked_names_through_vpn() {
        let mut s = Settings::default();
        s.lists.selected.push(Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: true });
        let d = dns(&s, &catalog());
        assert_eq!(d["nameserver"][0], "tls://77.88.8.8:853");
        assert_eq!(d["nameserver-policy"]["+.claude.ai"][0], "https://1.1.1.1/dns-query#klick-vpn");
        assert!(d["nameserver-policy"][format!("rule-set:{SET_BLOCKED_DOMAINS}")].is_array());
    }

    #[test]
    fn switched_off_rules_and_preset_do_nothing() {
        let mut s = Settings::default();
        s.lists.selected.push(Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: false });
        s.lists.selected.push(Rule { target: Target::Domain("chatgpt.com".into()), route: Route::Vpn, enabled: true });
        let r = rules(&s, &catalog());
        assert!(!r.iter().any(|x| x.contains("claude.ai")), "выключенное правило не работает");
        assert!(r.iter().any(|x| x == "DOMAIN-SUFFIX,chatgpt.com,klick-vpn"));
        assert!(dns(&s, &catalog())["nameserver-policy"].get("+.claude.ai").is_none());

        s.blocked_preset = false;
        let r = rules(&s, &catalog());
        assert!(!r.iter().any(|x| x.contains(SET_BLOCKED_DOMAINS) || x.contains(SET_BLOCKED_IPS)));
        assert_eq!(r.last().unwrap(), "MATCH,DIRECT");
        assert_eq!(rule_providers(&s, &sets()), json!({}));
        assert!(dns(&s, &catalog())["nameserver-policy"].get(format!("rule-set:{SET_BLOCKED_DOMAINS}")).is_none());
    }

    #[test]
    fn old_settings_keep_rules_on_and_preset_on() {
        let s: Settings = serde_json::from_str(r#"{"lists":{"selected":[{"target":{"kind":"domain","value":"x.com"},"route":"vpn"}],"all_vpn":[]}}"#).unwrap();
        assert!(s.lists.selected[0].enabled);
        assert!(s.blocked_preset);
    }

    #[test]
    fn tester_has_no_inbounds() {
        let t = compile_tester(&layout(), "providers/a.txt");
        assert!(t.get("mixed-port").is_none());
        assert!(t.get("tun").is_none());
        assert!(t.get("listeners").is_none());
        assert_eq!(t["external-controller-pipe"], r"\\.\pipe\klick-core");
    }
}
