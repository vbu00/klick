//! Сборщик конфигурации: режим, тумблер, списки, Kill Switch и серверы → конфиг mihomo.
//!
//! Конфиг пишется в JSON: mihomo читает его как YAML, JSON — подмножество YAML 1.2.
//! Порядок правил: локальная сеть → Kill Switch → программы → сервисы, домены, IP →
//! готовые наборы → всё остальное. Срабатывает первое подходящее правило.

use crate::model::{Catalog, Route, Routing, Rule, Settings, Target};
use crate::os::Os;
use serde_json::{json, Map, Value};

/// Группа, через которую идёт всё «через VPN». Сервер в ней выбирается через API ядра.
pub const VPN_GROUP: &str = "klick-vpn";
/// Провайдер серверов активного подключения.
pub const PROVIDER: &str = "klick-servers";
/// Адрес для проверки связи через сервер.
pub const PROBE_URL: &str = "https://www.gstatic.com/generate_204";

pub const SET_BLOCKED_DOMAINS: &str = "klick-blocked-domains";
pub const SET_BLOCKED_IPS: &str = "klick-blocked-ips";
/// Общий список заблокированного и закрывшегося для РФ (itdoginfo/allow-domains, ~1200 доменов,
/// обновляется сообществом). У репозитория нет лицензии — в установщик не вшиваем: ядро качает
/// его само, через VPN, раз в сутки. Не скачался — работает встроенный набор выше.
pub const SET_BLOCKED_COMMUNITY: &str = "klick-blocked-community";
pub const BLOCKED_COMMUNITY_URL: &str = "https://raw.githubusercontent.com/itdoginfo/allow-domains/main/Russia/inside-clashx.lst";
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
    /// Для какой системы конфиг: путь к программам, вход управления и адаптер TUN устроены по-разному.
    pub os: Os,
    /// Вход управления ядра: именованный канал на Windows, Unix-сокет на macOS.
    pub controller: String,
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
    /// macOS: DNS-серверы самой системы (до подмены DNS на время VPN). Через них ядро ищет адреса
    /// серверов VPN — так же, как проверочное ядро, которое меряет задержку. Пусто — `system`.
    pub server_dns: &'a [String],
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
    // Проверочное ядро — тот же исполняемый файл mihomo. Его соединения в TUN попали бы в
    // туннель основного, и задержка «другого» подключения мерилась бы через
    // текущий сервер. Собственные соединения основного ядра в TUN не входят.
    let exe = layout.core_exe.trim_start_matches(r"\\?\");
    if !exe.is_empty() {
        all.push(format!("PROCESS-PATH-REGEX,{},DIRECT", exact_path_regex(exe)));
    }
    all.extend(rules(s, input.catalog, layout.os));
    cfg.insert("rules".into(), Value::from(all));
    cfg.insert("dns".into(), dns(s, input.catalog, layout.os, input.server_dns));
    cfg.insert("sniffer".into(), sniffer());
    if input.capture == Capture::Tun {
        cfg.insert("tun".into(), tun(layout));
    }
    Value::Object(cfg)
}

/// Адаптер TUN. На macOS адаптер обязан называться `utunN`, и номер выбирает само ядро:
/// своё имя ядро там отвергает. Служба находит адаптер по адресу 198.18.0.1.
fn tun(layout: &CoreLayout) -> Value {
    let mut tun = json!({
        "enable": true,
        "stack": "gvisor",
        "auto-route": true,
        "auto-detect-interface": true,
        "strict-route": true,
        "dns-hijack": ["any:53", "tcp://any:53"],
    });
    if layout.os == Os::Windows {
        tun["device"] = json!(layout.tun_device);
    }
    tun
}

/// macOS: ядро-страж Kill Switch. Пока TUN не держит основное ядро (VPN выключен или включён режим
/// «Системный прокси»), страж забирает трафик в адаптер и не выпускает программы из Kill Switch,
/// остальное отпускает напрямую. Своего DNS у стража нет: имена по-прежнему спрашивает система,
/// поэтому страж не трогает настройки DNS и не меняет, куда ходят остальные программы.
/// На Windows то же делают постоянные фильтры WFP, и страж не нужен.
pub fn compile_guard(settings: &Settings, layout: &CoreLayout) -> Value {
    let mut rules = lan_rules();
    if ks_active(settings) {
        for p in settings.kill_switch.programs.iter().filter(|p| p.enabled) {
            rules.push(format!("PROCESS-PATH-REGEX,{},REJECT", folder_regex(layout.os, &p.folder)));
        }
        // Не узнали программу — считаем её защищённой: лучше не пустить чужую, чем выпустить свою.
        rules.push(format!("{UNKNOWN_PROCESS},REJECT"));
    }
    rules.push("MATCH,DIRECT".into());
    json!({
        "mode": "rule",
        "log-level": layout.log_level,
        "ipv6": true,
        "find-process-mode": "always",
        "geo-auto-update": false,
        "geodata-mode": false,
        controller_key(layout.os): layout.controller,
        "secret": layout.secret,
        "profile": { "store-selected": false, "store-fake-ip": false },
        "dns": { "enable": false },
        "tun": {
            "enable": true,
            "stack": "gvisor",
            "auto-route": true,
            "auto-detect-interface": true,
        },
        "rules": rules,
    })
}

/// Соединение, для которого ядро не узнало программу. На macOS ядро узнаёт её по внутренним структурам
/// системы; если на новой версии macOS они поменяются, программу не узнать — и Kill Switch не должен
/// превратиться в «пропустить напрямую».
pub const UNKNOWN_PROCESS: &str = "NOT,((PROCESS-PATH-REGEX,.+))";

/// В Kill Switch есть включённые программы.
pub fn ks_active(s: &Settings) -> bool {
    s.kill_switch.enabled && s.kill_switch.programs.iter().any(|p| p.enabled)
}

fn controller_key(os: Os) -> &'static str {
    match os {
        Os::Windows => "external-controller-pipe",
        Os::MacOs => "external-controller-unix",
    }
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
        controller_key(layout.os): layout.controller,
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
            SET_BLOCKED_COMMUNITY: {
                "type": "http",
                "behavior": "classical",
                "format": "text",
                "url": BLOCKED_COMMUNITY_URL,
                "path": "sets/blocked-community.lst",
                "interval": 86400,
                // С GitHub из России бывает плохо — качаем через VPN.
                "proxy": VPN_GROUP,
            },
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
pub fn folder_regex(os: Os, folder: &str) -> String {
    match os {
        Os::Windows => windows_folder_regex(folder),
        Os::MacOs => crate::macos::folder_regex(folder),
    }
}

/// Windows. Запятая разделяет части правила mihomo, поэтому она кодируется как `\x2c`.
/// У программ из Microsoft Store версия зашита в имя папки пакета
/// (`Claude_2.9939.2.0_x64__pzs8sxrjxfjjc`): её заменяем шаблоном, чтобы правило пережило обновление.
fn windows_folder_regex(folder: &str) -> String {
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

pub(crate) fn push_escaped(re: &mut String, text: &str) {
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

/// Локальная сеть и сам компьютер — всегда напрямую.
fn lan_rules() -> Vec<String> {
    let mut out = Vec::new();
    for cidr in LAN_V4.iter().chain(LAN_V6.iter()) {
        out.push(ip_rule(cidr, "DIRECT"));
    }
    for zone in ["local", "lan", "localhost"] {
        out.push(format!("DOMAIN-SUFFIX,{zone},DIRECT"));
    }
    out
}

pub fn rules(s: &Settings, catalog: &Catalog, os: Os) -> Vec<String> {
    let mut out = lan_rules();

    if s.kill_switch.enabled {
        for p in s.kill_switch.programs.iter().filter(|p| p.enabled) {
            out.push(format!("PROCESS-PATH-REGEX,{},{VPN_GROUP}", folder_regex(os, &p.folder)));
        }
    }
    // macOS: программа не узнана — только через VPN, никогда напрямую (на Windows то же держит WFP).
    if os == Os::MacOs && ks_active(s) {
        out.push(format!("{UNKNOWN_PROCESS},{VPN_GROUP}"));
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
    programs.sort_by_key(|(folder, _)| std::cmp::Reverse(os.depth(folder)));
    for (folder, route) in programs {
        out.push(format!("PROCESS-PATH-REGEX,{},{}", folder_regex(os, folder), policy(route)));
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
                out.push(format!("RULE-SET,{SET_BLOCKED_COMMUNITY},{VPN_GROUP}"));
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

/// Адреса серверов VPN. На Windows — DoT Яндекса, DoH Cloudflare и запасной обычный DNS Яндекса.
/// На macOS сначала DNS системы: проверочное ядро (задержка серверов) ищет адреса через него, и если
/// основное ядро ищет иначе, задержка «зелёная», а подключение сервер не находит. Зашифрованный DNS —
/// запасной: ядро спрашивает всех сразу и берёт первый ответ, а DNS роутера отвечает раньше, чем
/// успевает установиться соединение TLS; не отвечает — выручает зашифрованный.
fn proxy_server_dns(os: Os, server_dns: &[String]) -> Value {
    // Обычный UDP Яндекса — запасной на случай, когда DoT и DoH в сети закрыты: иначе сервер,
    // заданный доменом, не найдётся.
    let common = ["tls://77.88.8.8:853", "https://1.1.1.1/dns-query", "77.88.8.8"];
    match os {
        Os::Windows => json!(common),
        Os::MacOs => {
            let mut list: Vec<String> = if server_dns.is_empty() { vec!["system".into()] } else { server_dns.to_vec() };
            list.extend(common.iter().map(|u| u.to_string()));
            json!(list)
        }
    }
}

fn dns(s: &Settings, catalog: &Catalog, os: Os, server_dns: &[String]) -> Value {
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
    let mut fake_ip_filter = vec![
        "*.lan", "*.local", "*.localdomain", "localhost",
        "+.msftconnecttest.com", "+.msftncsi.com",
        "time.windows.com", "time.nist.gov", "+.pool.ntp.org", "+.time.edu.cn",
        "+.stun.*.*", "+.stun.*.*.*", "stun.l.google.com",
    ];
    if os == Os::MacOs {
        // Проверка «есть ли интернет» и страницы входа в сетях Wi-Fi, время системы.
        fake_ip_filter.extend(["captive.apple.com", "+.captive.apple.com", "time.apple.com", "+.time.apple.com"]);
    }
    json!({
        "enable": true,
        "ipv6": false,
        "enhanced-mode": "fake-ip",
        "fake-ip-range": "198.18.0.1/16",
        "fake-ip-filter": fake_ip_filter,
        "default-nameserver": ["77.88.8.8", "1.1.1.1"],
        "nameserver": nameserver,
        "nameserver-policy": policy,
        "direct-nameserver": ru,
        // Адреса самих серверов — напрямую.
        "proxy-server-nameserver": proxy_server_dns(os, server_dns),
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
    use crate::model::{KsProgram, Rule, Service};

    fn layout() -> CoreLayout {
        CoreLayout {
            os: Os::Windows,
            controller: r"\\.\pipe\klick-core".into(),
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
            folder_regex(Os::Windows, r"C:\Program Files\WindowsApps\Claude_2.9939.2.0_x64__pzs8sxrjxfjjc\app"),
            r"(?i)^C:\\Program Files\\WindowsApps\\Claude_[^\\]+__pzs8sxrjxfjjc\\app\\.+$"
        );
        assert_eq!(folder_regex(Os::Windows, r"C:\Games\My_Game_1.0_final"), r"(?i)^C:\\Games\\My_Game_1\.0_final\\.+$");
    }

    #[test]
    fn folder_regex_escapes_windows_paths() {
        assert_eq!(
            folder_regex(Os::Windows, r"C:\Program Files (x86)\Game, Inc"),
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
        let r = rules(&s, &catalog(), Os::Windows);
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
        let r = rules(&s, &catalog(), Os::Windows);
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
        let r = rules(&s, &catalog(), Os::Windows);
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
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(!r.iter().any(|x| x.contains(SET_RU_DOMAINS)));
        assert!(r.contains(&"GEOIP,RU,DIRECT".to_string()));
        assert_eq!(rule_providers(&s, &sets()), json!({}));
        assert!(dns(&s, &catalog(), Os::Windows, &[])["nameserver-policy"].as_object().unwrap().is_empty());

        s.russia_direct.ips = false;
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(!r.iter().any(|x| x.starts_with("GEOIP")));
        assert!(r.iter().any(|x| x == "IP-CIDR,192.168.0.0/16,DIRECT,no-resolve"), "локальная сеть напрямую всегда");
        assert_eq!(r.last().unwrap(), "MATCH,klick-vpn");
    }

    #[test]
    fn disabled_kill_switch_adds_no_rules() {
        let mut s = Settings::default();
        s.kill_switch.enabled = false;
        s.kill_switch.programs.push(r"C:\Games\Roblox".into());
        assert!(!rules(&s, &catalog(), Os::Windows).iter().any(|r| r.contains("Roblox")));
        s.kill_switch.enabled = true;
        s.kill_switch.programs[0].enabled = false;
        assert!(!rules(&s, &catalog(), Os::Windows).iter().any(|r| r.contains("Roblox")), "выключенная программа не защищается");
    }

    #[test]
    fn tun_section_only_in_tun_capture() {
        let s = Settings::default();
        let (c, l, st) = (catalog(), layout(), sets());
        let input = |capture| CompileInput { settings: &s, catalog: &c, layout: &l, capture, provider_path: "providers/a.txt", sets: &st, server_dns: &[] };
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
        let cfg = compile(&CompileInput { settings: &s, catalog: &c, layout: &l, capture: Capture::Tun, provider_path: "providers/a.txt", sets: &st, server_dns: &[] });
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
        let d = dns(&s, &catalog(), Os::Windows, &[]);
        assert_eq!(d["nameserver"][0], "tls://77.88.8.8:853");
        assert_eq!(d["nameserver-policy"]["+.claude.ai"][0], "https://1.1.1.1/dns-query#klick-vpn");
        assert!(d["nameserver-policy"][format!("rule-set:{SET_BLOCKED_DOMAINS}")].is_array());
    }

    #[test]
    fn switched_off_rules_and_preset_do_nothing() {
        let mut s = Settings::default();
        s.lists.selected.push(Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: false });
        s.lists.selected.push(Rule { target: Target::Domain("chatgpt.com".into()), route: Route::Vpn, enabled: true });
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(!r.iter().any(|x| x.contains("claude.ai")), "выключенное правило не работает");
        assert!(r.iter().any(|x| x == "DOMAIN-SUFFIX,chatgpt.com,klick-vpn"));
        assert!(dns(&s, &catalog(), Os::Windows, &[])["nameserver-policy"].get("+.claude.ai").is_none());

        s.blocked_preset = false;
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(!r.iter().any(|x| x.contains(SET_BLOCKED_DOMAINS) || x.contains(SET_BLOCKED_IPS) || x.contains(SET_BLOCKED_COMMUNITY)));
        assert_eq!(r.last().unwrap(), "MATCH,DIRECT");
        assert_eq!(rule_providers(&s, &sets()), json!({}));
        assert!(dns(&s, &catalog(), Os::Windows, &[])["nameserver-policy"].get(format!("rule-set:{SET_BLOCKED_DOMAINS}")).is_none());
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

    fn mac_layout() -> CoreLayout {
        CoreLayout {
            os: Os::MacOs,
            controller: "/Library/Application Support/klick/run/core.sock".into(),
            core_exe: "/Library/PrivilegedHelperTools/klick/mihomo".into(),
            ..layout()
        }
    }

    #[test]
    fn macos_config_uses_unix_socket_and_system_tun_name() {
        let s = Settings::default();
        let (c, l, st) = (catalog(), mac_layout(), sets());
        let cfg = compile(&CompileInput { settings: &s, catalog: &c, layout: &l, capture: Capture::Tun, provider_path: "providers/a.txt", sets: &st, server_dns: &[] });
        assert_eq!(cfg["external-controller-unix"], "/Library/Application Support/klick/run/core.sock");
        assert!(cfg.get("external-controller-pipe").is_none());
        assert!(cfg["tun"].get("device").is_none(), "на macOS имя utunN выбирает ядро");
        assert_eq!(cfg["tun"]["auto-route"], true);
        let filter = cfg["dns"]["fake-ip-filter"].as_array().unwrap();
        assert!(filter.iter().any(|f| f == "captive.apple.com"));
        let windows = compile(&CompileInput { settings: &s, catalog: &c, layout: &layout(), capture: Capture::Tun, provider_path: "providers/a.txt", sets: &st, server_dns: &[] });
        assert!(!windows["dns"]["fake-ip-filter"].as_array().unwrap().iter().any(|f| f == "captive.apple.com"));
        assert_eq!(windows["tun"]["device"], "klick");
    }

    #[test]
    fn macos_rules_use_bundle_paths() {
        let mut s = Settings::default();
        s.kill_switch.programs.push("/Applications/Telegram.app".into());
        s.lists.selected = vec![
            Rule { target: Target::Program("/Users/a/Games".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Program("/Users/a/Games/Roblox.app".into()), route: Route::Vpn, enabled: true },
        ];
        let r = rules(&s, &catalog(), Os::MacOs);
        assert!(r.contains(&r"PROCESS-PATH-REGEX,(?i)^(?:/Applications/Telegram\.app|(?:/private)?/var/folders/.+/AppTranslocation/[^/]+/d/Telegram\.app)/.+$,klick-vpn".to_string()));
        assert!(position(&r, "Roblox") < position(&r, "Games/.+"), "вложенная папка раньше общей");
    }

    #[test]
    fn macos_finds_vpn_servers_through_system_dns_first() {
        let windows = proxy_server_dns(Os::Windows, &["192.168.1.1".into()]);
        assert_eq!(windows, json!(["tls://77.88.8.8:853", "https://1.1.1.1/dns-query", "77.88.8.8"]));
        let mac = proxy_server_dns(Os::MacOs, &["192.168.1.1".into()]);
        assert_eq!(mac, json!(["192.168.1.1", "tls://77.88.8.8:853", "https://1.1.1.1/dns-query", "77.88.8.8"]));
        assert_eq!(proxy_server_dns(Os::MacOs, &[])[0], "system");
    }

    #[test]
    fn macos_tester_core_goes_direct_not_through_tunnel() {
        let s = Settings::default();
        let (c, l, st) = (catalog(), mac_layout(), sets());
        let cfg = compile(&CompileInput { settings: &s, catalog: &c, layout: &l, capture: Capture::Tun, provider_path: "providers/a.txt", sets: &st, server_dns: &[] });
        assert_eq!(cfg["rules"][0], r"PROCESS-PATH-REGEX,(?i)^/Library/PrivilegedHelperTools/klick/mihomo$,DIRECT");
    }

    #[test]
    fn guard_blocks_only_kill_switch_programs() {
        let mut s = Settings::default();
        s.kill_switch.programs.push("/Applications/Telegram.app".into());
        s.kill_switch.programs.push(KsProgram { folder: "/Applications/Discord.app".into(), enabled: false });
        let g = compile_guard(&s, &mac_layout());
        let rules: Vec<&str> = g["rules"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
        assert!(rules.contains(&r"PROCESS-PATH-REGEX,(?i)^(?:/Applications/Telegram\.app|(?:/private)?/var/folders/.+/AppTranslocation/[^/]+/d/Telegram\.app)/.+$,REJECT"));
        assert!(!rules.iter().any(|r| r.contains("Discord")), "выключенная программа не блокируется");
        assert!(rules.iter().position(|r| r.contains("192.168.0.0/16")) < rules.iter().position(|r| r.contains("Telegram")), "локальная сеть можно");
        assert_eq!(rules.last(), Some(&"MATCH,DIRECT"));
        assert_eq!(g["dns"]["enable"], false, "страж не трогает DNS");
        assert!(g["tun"].get("dns-hijack").is_none());
        assert!(g.get("proxy-providers").is_none() && g.get("mixed-port").is_none());
        assert_eq!(g["external-controller-unix"], "/Library/Application Support/klick/run/core.sock");

        assert!(rules.contains(&"NOT,((PROCESS-PATH-REGEX,.+)),REJECT"), "не узнали программу — не выпускать");
        assert!(rules.iter().position(|r| r.contains("Telegram")) < rules.iter().position(|r| r.starts_with("NOT,")));

        s.kill_switch.enabled = false;
        let off = compile_guard(&s, &mac_layout());
        assert!(!off["rules"].as_array().unwrap().iter().any(|r| r.as_str().unwrap().contains("REJECT")));
    }

    #[test]
    fn unknown_programs_go_through_vpn_only_on_macos_with_kill_switch() {
        let mut s = Settings::default();
        let unknown = format!("{UNKNOWN_PROCESS},{VPN_GROUP}");
        assert!(!rules(&s, &catalog(), Os::MacOs).contains(&unknown), "пустой Kill Switch ничего не меняет");
        s.kill_switch.programs.push("/Applications/Telegram.app".into());
        let r = rules(&s, &catalog(), Os::MacOs);
        let at = r.iter().position(|x| *x == unknown).expect("правило для неузнанных");
        assert!(position(&r, "Telegram") < at && at < position(&r, "MATCH"));
        assert!(!rules(&s, &catalog(), Os::Windows).iter().any(|x| x.starts_with("NOT,")), "на Windows это держит WFP");
    }
}
