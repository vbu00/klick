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

/// Поправки к серверам из любого источника (подписка Clash, ссылки, файлы) — ядро применяет их к каждому
/// серверу провайдера, только если сервер не задал своё значение.
const SERVER_FIXES: [&str; 5] = [
    // Hysteria2: одно рукопожатие на все соединения. Без срока каждое новое соединение после сна или смены
    // сети ждёт рукопожатия первого под замком, и страница открывается по очереди, по нескольку секунд.
    r#"(select(.type == "hysteria2" and .handshake-timeout == null) | .handshake-timeout) = 10"#,
    // WireGuard и AmneziaWG: без keepalive NAT роутера и оператора забывает сопоставление, и сервер молчит.
    r#"(select(.type == "wireguard" and .persistent-keepalive == null) | .persistent-keepalive) = 25"#,
    // Ядро 1.19.33 перевело WireGuard на новый стек mips; остаёмся на проверенном gVisor.
    r#"(select(.type == "wireguard" and .ip-stack.mode == null) | .ip-stack.mode) = "gvisor""#,
    // Имена сайтов для WireGuard — через туннель: иначе сайт программы из списка «через VPN» ищется
    // российским DNS напрямую (утечка, а заблокированный сайт может получить негодный адрес).
    r#"(select(.type == "wireguard" and .dns == null) | .dns) = ["1.1.1.1", "8.8.8.8"]"#,
    r#"(select(.type == "wireguard" and .remote-dns-resolve == null) | .remote-dns-resolve) = true"#,
];

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
    /// Вход управления ядра: именованный канал на Windows, Unix-сокет на macOS, `127.0.0.1:порт` на iOS.
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
    let user = user_sets(s, input.catalog, layout.os);
    cfg.insert("rule-providers".into(), rule_providers(s, input.sets, user.as_deref()));
    let mut all = Vec::new();
    // Проверочное ядро — тот же исполняемый файл mihomo. Его соединения в TUN попали бы в
    // туннель основного, и задержка «другого» подключения мерилась бы через
    // текущий сервер. Собственные соединения основного ядра в TUN не входят.
    let exe = layout.core_exe.trim_start_matches(r"\\?\");
    if !exe.is_empty() {
        all.push(format!("PROCESS-PATH-REGEX,{},DIRECT", exact_path_regex(exe)));
    }
    all.extend(match &user {
        Some(sets) => compose(s, layout.os, set_rules(sets)),
        None => rules(s, input.catalog, layout.os),
    });
    cfg.insert("rules".into(), Value::from(all));
    cfg.insert("dns".into(), dns(s, input.catalog, layout.os, input.server_dns, user.as_deref()));
    cfg.insert("sniffer".into(), sniffer());
    // iOS: адаптер поднимает расширение VPN по дескриптору туннеля от системы, а не по конфигу —
    // поэтому и перечитывание конфига его не трогает.
    if input.capture == Capture::Tun && !layout.os.mobile() {
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

/// iOS: путь сокета в общей папке приложения длиннее предела в 104 байта (в симуляторе — заметно),
/// поэтому вход управления — порт на 127.0.0.1 с паролем `secret`.
fn controller_key(os: Os) -> &'static str {
    match os {
        Os::Windows => "external-controller-pipe",
        Os::MacOs => "external-controller-unix",
        Os::Ios | Os::Android => "external-controller",
    }
}

/// Проверочное ядро: только серверы и управление, без портов и перехвата.
/// Нужно, чтобы мерить задержку и читать список серверов при выключенном VPN.
pub fn compile_tester(layout: &CoreLayout, provider_path: &str) -> Value {
    let mut cfg = base(layout, provider_path);
    cfg.insert("rules".into(), json!(["MATCH,DIRECT"]));
    Value::Object(cfg)
}

/// Проверочное ядро с отдельным входом для каждого сервера, который проверяется параллельно.
pub fn compile_probe(layout: &CoreLayout, provider_path: &str, slot_ports: &[u16]) -> Value {
    let mut cfg = base(layout, provider_path);
    let groups: Vec<Value> = slot_ports
        .iter()
        .enumerate()
        .map(|(i, _)| json!({ "name": format!("klick-probe-{i}"), "type": "select", "use": [PROVIDER], "empty-fallback": "REJECT" }))
        .collect();
    let listeners: Vec<Value> = slot_ports
        .iter()
        .enumerate()
        .map(|(i, port)| json!({ "name": format!("klick-probe-listener-{i}"), "type": "http", "port": port, "listen": "127.0.0.1", "proxy": format!("klick-probe-{i}") }))
        .collect();
    cfg.insert("allow-lan".into(), json!(false));
    cfg.insert("proxy-groups".into(), Value::Array(groups));
    cfg.insert("listeners".into(), Value::Array(listeners));
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
        // iOS не говорит, чьё соединение: искать программу бесполезно.
        "find-process-mode": if layout.os.mobile() { "off" } else { "always" },
        "geo-auto-update": false,
        "geodata-mode": false,
        controller_key(layout.os): layout.controller,
        "secret": layout.secret,
        "profile": { "store-selected": false, "store-fake-ip": true },
        "proxy-providers": {
            PROVIDER: { "type": "file", "path": provider_path, "health-check": { "enable": false }, "override": { "override-expr": SERVER_FIXES } }
        },
        // Серверов нет (провайдер не загрузился) — пустая группа у ядра значит «напрямую»: утечка мимо VPN.
        // `empty-fallback: REJECT` — тогда соединения отклоняются, а страж скажет, что сервер не отвечает.
        "proxy-groups": [
            { "name": VPN_GROUP, "type": "select", "use": [PROVIDER], "empty-fallback": "REJECT" }
        ],
    });
    match v {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

fn rule_providers(s: &Settings, sets: &SetFiles, user: Option<&[UserSet]>) -> Value {
    let file = |behavior: &str, path: &str| json!({ "type": "file", "behavior": behavior, "format": "text", "path": path });
    let mut out = preset_providers(s, sets, file);
    if let (Some(map), Some(user)) = (out.as_object_mut(), user) {
        for u in user {
            map.insert(u.name.clone(), file(u.behavior, &u.path));
        }
    }
    out
}

fn preset_providers(s: &Settings, sets: &SetFiles, file: impl Fn(&str, &str) -> Value) -> Value {
    let blocked = || {
        json!({
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
        })
    };
    match s.routing {
        Routing::Selected if s.blocked_preset => blocked(),
        Routing::Selected => json!({}),
        // «Россия напрямую» — и заблокированное тоже нужно: оно идёт через VPN раньше российских правил.
        Routing::AllVpn if russia_direct(s) => {
            let mut out = blocked();
            if s.russia_direct.domains {
                out[SET_RU_DOMAINS] = file("domain", &sets.ru_domains);
            }
            out
        }
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
        Os::MacOs | Os::Ios | Os::Android => crate::macos::folder_regex(folder),
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

/// Все правила с пользовательскими списками прямо в конфиге (так — когда файлами правил нельзя, см. `user_sets`).
pub fn rules(s: &Settings, catalog: &Catalog, os: Os) -> Vec<String> {
    compose(s, os, inline_user_rules(s, catalog, os))
}

/// Пользовательские списки строками правил. Ядро берёт первое совпавшее правило, поэтому внутри списка
/// точное идёт раньше общего: папка поглубже, домен подлиннее, подсеть поуже. При равной точности свои
/// сайты и подсети — раньше сервисов каталога. Сортировка устойчивая: в остальном порядок как в списке.
fn inline_user_rules(s: &Settings, catalog: &Catalog, os: Os) -> Vec<String> {
    let (mut programs, mut domains, mut ips) = user_targets(s, catalog);
    programs.sort_by_key(|(folder, _)| std::cmp::Reverse(os.depth(folder)));
    domains.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    ips.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut out = Vec::new();
    for (folder, route) in programs {
        out.push(format!("PROCESS-PATH-REGEX,{},{}", folder_regex(os, &folder), policy(route)));
    }
    for (_, _, domain, route) in domains {
        out.push(format!("DOMAIN-SUFFIX,{domain},{}", policy(route)));
    }
    for (_, _, cidr, route) in ips {
        out.push(ip_rule(&cidr, policy(route)));
    }
    out
}

/// Программы, домены и подсети включённых правил текущего положения тумблера.
fn user_targets(s: &Settings, catalog: &Catalog) -> (Vec<(String, Route)>, Vec<Ranked>, Vec<Ranked>) {
    let list: Vec<&Rule> = s.rules().collect();
    let programs = list
        .iter()
        .filter_map(|r| match &r.target {
            Target::Program(folder) => Some((folder.clone(), r.route)),
            _ => None,
        })
        .collect();
    let (mut domains, mut ips) = (Vec::new(), Vec::new());
    for rule in &list {
        collect_targets(rule, catalog, &mut domains, &mut ips);
    }
    (programs, domains, ips)
}

/// Свой список в файле правил ядра: имя набора, вид (`classical`, `domain`, `ipcidr`), файл, маршрут, строки.
#[derive(Clone, Debug, PartialEq)]
pub struct UserSet {
    pub name: String,
    pub behavior: &'static str,
    pub path: String,
    pub policy: &'static str,
    pub no_resolve: bool,
    pub body: String,
}

/// Свои списки — файлами правил ядра, по виду и маршруту, чтобы их правка не перечитывала конфиг:
/// перечитанный конфиг пересоздаёт серверы провайдера, и ядро закрывает все соединения через VPN
/// (звонок обрывается, когда человек добавляет сайт). Файл правил ядро перечитывает, ничего не обрывая.
/// Внутри файла порядок не важен — маршрут один; между маршрутами он постоянный (блок, напрямую, VPN)
/// и «точное раньше общего» не соблюдает. Поэтому, если вложенные записи ведут разными маршрутами
/// (`mail.example.com` напрямую внутри `example.com` через VPN, папка внутри папки), — `None`:
/// правила остаются в конфиге, как раньше.
pub fn user_sets(s: &Settings, catalog: &Catalog, os: Os) -> Option<Vec<UserSet>> {
    let (programs, domains, ips) = user_targets(s, catalog);
    let domains: Vec<(String, Route)> = domains.into_iter().map(|(_, _, d, r)| (d, r)).collect();
    let ips: Vec<(String, Route)> = ips.into_iter().map(|(_, _, c, r)| (c, r)).collect();
    if crossed(&programs, folder_inside) || crossed(&domains, domain_inside) || crossed(&ips, cidr_inside) {
        return None;
    }
    let kinds: [(&str, &'static str, bool, &[(String, Route)]); 3] =
        [("programs", "classical", false, &programs), ("domains", "domain", false, &domains), ("ips", "ipcidr", true, &ips)];
    let mut out = Vec::new();
    for (kind, behavior, no_resolve, items) in kinds {
        for (route, route_name) in [(Route::Block, "block"), (Route::Direct, "direct"), (Route::Vpn, "vpn")] {
            let mut lines: Vec<String> = items
                .iter()
                .filter(|(_, r)| *r == route)
                .map(|(v, _)| match kind {
                    "programs" => format!("PROCESS-PATH-REGEX,{}", folder_regex(os, v)),
                    "domains" => format!("+.{v}"),
                    _ => v.clone(),
                })
                .collect();
            lines.sort();
            lines.dedup();
            if lines.is_empty() {
                continue;
            }
            let name = format!("klick-user-{kind}-{route_name}");
            out.push(UserSet { path: format!("user/{name}.txt"), name, behavior, policy: policy(route), no_resolve, body: lines.join("\n") + "\n" });
        }
    }
    Some(out)
}

fn set_rules(sets: &[UserSet]) -> Vec<String> {
    sets.iter().map(|u| format!("RULE-SET,{},{}{}", u.name, u.policy, if u.no_resolve { ",no-resolve" } else { "" })).collect()
}

/// Есть ли пара вложенных записей с разными маршрутами.
fn crossed(items: &[(String, Route)], inside: fn(&str, &str) -> bool) -> bool {
    items.iter().enumerate().any(|(i, (a, ra))| items[i + 1..].iter().any(|(b, rb)| ra != rb && (inside(a, b) || inside(b, a))))
}

/// Папка `a` — это `b` или лежит внутри неё (без учёта регистра).
fn folder_inside(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim_end_matches(['\\', '/']).to_lowercase(), b.trim_end_matches(['\\', '/']).to_lowercase());
    a == b || a.strip_prefix(b.as_str()).is_some_and(|rest| rest.starts_with(['\\', '/']))
}

/// Домен `a` — это `b` или его поддомен.
fn domain_inside(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
    a == b || a.ends_with(&format!(".{b}"))
}

/// Подсеть `a` лежит внутри `b`. Непонятную запись считаем вложенной: лучше остаться при старом порядке.
fn cidr_inside(a: &str, b: &str) -> bool {
    let parse = |c: &str| -> Option<(u128, u8, bool)> {
        let (ip, len) = c.split_once('/').map_or((c, None), |(i, l)| (i, l.parse::<u8>().ok()));
        match ip.parse::<std::net::IpAddr>().ok()? {
            // IPv4 — в старших 32 битах: длина префикса та же.
            std::net::IpAddr::V4(v4) => Some((u128::from(u32::from(v4)) << 96, len.unwrap_or(32), false)),
            std::net::IpAddr::V6(v6) => Some((u128::from(v6), len.unwrap_or(128), true)),
        }
    };
    let (Some((ia, la, va)), Some((ib, lb, vb))) = (parse(a), parse(b)) else { return true };
    if va != vb || la < lb || lb > 128 {
        return false;
    }
    let mask = if lb == 0 { 0 } else { u128::MAX << (128 - u32::from(lb)) };
    ia & mask == ib & mask
}

/// «Россию напрямую» включили хоть как-то — доменами или адресами.
fn russia_direct(s: &Settings) -> bool {
    s.russia_direct.domains || s.russia_direct.ips
}

/// Набор «Заблокированное в РФ»: свой список доменов и подсетей и общий список сообщества — через VPN.
fn blocked_rules() -> [String; 3] {
    [
        format!("RULE-SET,{SET_BLOCKED_DOMAINS},{VPN_GROUP}"),
        format!("RULE-SET,{SET_BLOCKED_IPS},{VPN_GROUP},no-resolve"),
        format!("RULE-SET,{SET_BLOCKED_COMMUNITY},{VPN_GROUP}"),
    ]
}

/// Правила целиком: локальная сеть, Kill Switch, пользовательские (строками или файлами), наборы, остальное.
fn compose(s: &Settings, os: Os, user: Vec<String>) -> Vec<String> {
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
    out.extend(user);

    match s.routing {
        Routing::Selected => {
            if s.blocked_preset {
                out.extend(blocked_rules());
            }
            out.push("MATCH,DIRECT".into());
        }
        Routing::AllVpn => {
            // Заблокированное в РФ и то, что само не пускает из России (игры Supercell, Spotify…), — через
            // VPN, даже если адрес российский: у их CDN есть узлы в России, российский DNS отдаёт их, и
            // «российские адреса напрямую» уводили часть трафика мимо VPN — сервис видел российский IP.
            if russia_direct(s) {
                out.extend(blocked_rules());
            }
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
        // iOS: DNS системы расширению не виден (и пока VPN включён, он — адрес внутри туннеля).
        Os::Windows | Os::Ios | Os::Android => json!(common),
        Os::MacOs => {
            let mut list: Vec<String> = if server_dns.is_empty() { vec!["system".into()] } else { server_dns.to_vec() };
            list.extend(common.iter().map(|u| u.to_string()));
            json!(list)
        }
    }
}

fn dns(s: &Settings, catalog: &Catalog, os: Os, server_dns: &[String], user: Option<&[UserSet]>) -> Value {
    let foreign: Vec<String> = DNS_FOREIGN.iter().map(|u| format!("{u}#{VPN_GROUP}")).collect();
    let ru: Vec<String> = DNS_RU.iter().map(|u| u.to_string()).collect();
    let mut policy = Map::new();
    let nameserver = match s.routing {
        Routing::Selected => {
            if s.blocked_preset {
                policy.insert(format!("rule-set:{SET_BLOCKED_DOMAINS}"), json!(foreign));
            }
            // Списки файлами: те же домены — из файла правил, и правка списка не меняет конфиг.
            match user {
                Some(sets) => {
                    if let Some(vpn) = sets.iter().find(|u| u.behavior == "domain" && u.policy == VPN_GROUP) {
                        policy.insert(format!("rule-set:{}", vpn.name), json!(foreign));
                    }
                }
                None => {
                    for d in vpn_domains(s, catalog) {
                        policy.insert(format!("+.{d}"), json!(foreign));
                    }
                }
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
    if os != Os::Windows {
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
        // Заблокированное (игры Supercell, у которых узлы CDN в России) — через VPN раньше российских правил.
        assert_eq!(r[n - 6..n - 3], blocked_rules());
    }

    #[test]
    fn russia_switches_control_direct_rules() {
        let mut s = Settings::default();
        s.routing = Routing::AllVpn;
        s.russia_direct.domains = false;
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(!r.iter().any(|x| x.contains(SET_RU_DOMAINS)));
        assert!(r.contains(&"GEOIP,RU,DIRECT".to_string()));
        let providers = rule_providers(&s, &sets(), None);
        assert!(providers.get(SET_RU_DOMAINS).is_none());
        assert!(providers.get(SET_BLOCKED_COMMUNITY).is_some(), "российские адреса напрямую — заблокированное раньше них через VPN");
        assert!(dns(&s, &catalog(), Os::Windows, &[], None)["nameserver-policy"].as_object().unwrap().is_empty());

        s.russia_direct.ips = false;
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(!r.iter().any(|x| x.starts_with("GEOIP")));
        assert!(!r.iter().any(|x| x.starts_with("RULE-SET")), "всё через VPN — наборы не нужны");
        assert_eq!(rule_providers(&s, &sets(), None), json!({}));
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
    fn probe_core_has_one_rejecting_group_and_listener_per_slot() {
        let cfg = compile_probe(&layout(), "providers/a.txt", &[12001, 12002]);
        assert_eq!(cfg["proxy-providers"][PROVIDER]["path"], "providers/a.txt");
        assert_eq!(cfg["proxy-providers"][PROVIDER]["override"]["override-expr"].as_array().unwrap().len(), SERVER_FIXES.len());
        assert_eq!(cfg["proxy-groups"].as_array().unwrap().len(), 2);
        assert_eq!(cfg["proxy-groups"][0], json!({ "name": "klick-probe-0", "type": "select", "use": [PROVIDER], "empty-fallback": "REJECT" }));
        assert_eq!(cfg["proxy-groups"][1]["name"], "klick-probe-1");
        assert_eq!(cfg["listeners"][0]["port"], 12001);
        assert_eq!(cfg["listeners"][0]["proxy"], "klick-probe-0");
        assert_eq!(cfg["listeners"][1]["port"], 12002);
        assert_eq!(cfg["listeners"][1]["proxy"], "klick-probe-1");
        assert_eq!(cfg["rules"], json!(["MATCH,DIRECT"]));
        assert_eq!(cfg["allow-lan"], false);
    }

    #[test]
    fn dns_asks_blocked_names_through_vpn() {
        let mut s = Settings::default();
        s.lists.selected.push(Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: true });
        let d = dns(&s, &catalog(), Os::Windows, &[], None);
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
        assert!(dns(&s, &catalog(), Os::Windows, &[], None)["nameserver-policy"].get("+.claude.ai").is_none());

        s.blocked_preset = false;
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(!r.iter().any(|x| x.contains(SET_BLOCKED_DOMAINS) || x.contains(SET_BLOCKED_IPS) || x.contains(SET_BLOCKED_COMMUNITY)));
        assert_eq!(r.last().unwrap(), "MATCH,DIRECT");
        assert_eq!(rule_providers(&s, &sets(), None), json!({}));
        assert!(dns(&s, &catalog(), Os::Windows, &[], None)["nameserver-policy"].get(format!("rule-set:{SET_BLOCKED_DOMAINS}")).is_none());
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
    fn ios_config_leaves_tun_to_the_extension() {
        let s = Settings::default();
        let (c, st) = (catalog(), sets());
        let l = CoreLayout { os: Os::Ios, controller: "127.0.0.1:17900".into(), core_exe: String::new(), ..layout() };
        let cfg = compile(&CompileInput { settings: &s, catalog: &c, layout: &l, capture: Capture::Tun, provider_path: "providers/a.txt", sets: &st, server_dns: &[] });
        assert!(cfg.get("tun").is_none(), "адаптер держит расширение VPN");
        assert_eq!(cfg["external-controller"], "127.0.0.1:17900");
        assert_eq!(cfg["secret"], "s");
        assert_eq!(cfg["find-process-mode"], "off");
        assert!(!cfg["rules"][0].as_str().unwrap().starts_with("PROCESS-PATH"), "своего exe у ядра на iOS нет");
        assert_eq!(cfg["dns"]["proxy-server-nameserver"], json!(["tls://77.88.8.8:853", "https://1.1.1.1/dns-query", "77.88.8.8"]));
        assert!(cfg["dns"]["fake-ip-filter"].as_array().unwrap().iter().any(|f| f == "captive.apple.com"));
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

    fn compiled(s: &Settings) -> Value {
        let (c, l, st) = (catalog(), layout(), sets());
        compile(&CompileInput { settings: s, catalog: &c, layout: &l, capture: Capture::Tun, provider_path: "providers/a.txt", sets: &st, server_dns: &[] })
    }

    fn discord_and_telegram() -> Settings {
        let mut s = Settings::default();
        s.kill_switch.programs.push(r"C:\Games\Roblox".into());
        s.lists.selected = vec![
            Rule { target: Target::Program(r"C:\Users\Иван\AppData\Local\Discord".into()), route: Route::Vpn, enabled: true },
            Rule { target: Target::Program(r"C:\Games\Valorant".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Domain("discord.com".into()), route: Route::Vpn, enabled: true },
            Rule { target: Target::Service("telegram".into()), route: Route::Vpn, enabled: true },
            Rule { target: Target::Ip("10.20.0.0/16".into()), route: Route::Direct, enabled: true },
        ];
        s
    }

    #[test]
    fn user_lists_go_to_rule_files_by_kind_and_route() {
        let s = discord_and_telegram();
        let sets = user_sets(&s, &catalog(), Os::Windows).expect("вложенности с разными маршрутами нет");
        let names: Vec<&str> = sets.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(
            names,
            ["klick-user-programs-direct", "klick-user-programs-vpn", "klick-user-domains-vpn", "klick-user-ips-direct", "klick-user-ips-vpn"]
        );
        assert_eq!(sets[1].body, "PROCESS-PATH-REGEX,(?i)^C:\\\\Users\\\\Иван\\\\AppData\\\\Local\\\\Discord\\\\.+$\n");
        assert_eq!(sets[2].body, "+.discord.com\n+.t.me\n+.telegram.org\n");
        assert_eq!(sets[4].body, "149.154.160.0/20\n2001:b28:f23d::/48\n");

        let cfg = compiled(&s);
        let r: Vec<String> = cfg["rules"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let ks = position(&r, "Roblox");
        let first = position(&r, "RULE-SET,klick-user-programs-direct,DIRECT");
        let ips = position(&r, "RULE-SET,klick-user-ips-vpn,klick-vpn,no-resolve");
        assert!(ks < first && first < ips && ips < position(&r, SET_BLOCKED_DOMAINS));
        assert!(!r.iter().any(|x| x.contains("discord.com")), "сами домены — в файле, не в конфиге");
        assert_eq!(cfg["rule-providers"]["klick-user-domains-vpn"], json!({ "type": "file", "behavior": "domain", "format": "text", "path": "user/klick-user-domains-vpn.txt" }));
        assert_eq!(cfg["dns"]["nameserver-policy"]["rule-set:klick-user-domains-vpn"][0], "https://1.1.1.1/dns-query#klick-vpn");
    }

    #[test]
    fn adding_a_site_changes_only_the_rule_file() {
        // Главное: правка списка не меняет конфиг ядра — значит, его не надо перечитывать и рвать соединения.
        let before = discord_and_telegram();
        let mut after = before.clone();
        after.lists.selected.push(Rule { target: Target::Domain("youtube.com".into()), route: Route::Vpn, enabled: true });
        after.lists.selected.push(Rule { target: Target::Program(r"C:\Program Files\Steam".into()), route: Route::Vpn, enabled: true });
        assert_eq!(compiled(&before), compiled(&after));
        let (b, a) = (user_sets(&before, &catalog(), Os::Windows).unwrap(), user_sets(&after, &catalog(), Os::Windows).unwrap());
        assert_ne!(b, a);
        assert!(a.iter().any(|u| u.body.contains("+.youtube.com")));
        // Первый сайт с новым маршрутом — новый файл: тогда конфиг меняется (один раз).
        after.lists.selected.push(Rule { target: Target::Domain("sber.ru".into()), route: Route::Direct, enabled: true });
        assert_ne!(compiled(&before), compiled(&after));
    }

    #[test]
    fn nested_entries_with_different_routes_stay_in_the_config() {
        let mut s = Settings::default();
        s.lists.selected = vec![
            Rule { target: Target::Domain("ru".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Domain("kinopoisk.ru".into()), route: Route::Vpn, enabled: true },
        ];
        assert!(user_sets(&s, &catalog(), Os::Windows).is_none());
        let r = rules(&s, &catalog(), Os::Windows);
        assert!(position(&r, "DOMAIN-SUFFIX,kinopoisk.ru") < position(&r, "DOMAIN-SUFFIX,ru,"));
        assert!(compiled(&s)["rules"].as_array().unwrap().iter().any(|x| x == "DOMAIN-SUFFIX,kinopoisk.ru,klick-vpn"));

        // Вложенные с одним маршрутом — можно файлами: порядок внутри файла не важен.
        s.lists.selected[0].route = Route::Vpn;
        assert!(user_sets(&s, &catalog(), Os::Windows).is_some());
        // Папка внутри папки, подсеть внутри подсети, свой сайт поверх сервиса — с разными маршрутами нельзя.
        s.lists.selected = vec![
            Rule { target: Target::Program(r"C:\Games".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Program(r"C:\games\Roblox\".into()), route: Route::Vpn, enabled: true },
        ];
        assert!(user_sets(&s, &catalog(), Os::Windows).is_none());
        s.lists.selected = vec![
            Rule { target: Target::Ip("149.154.160.0/22".into()), route: Route::Direct, enabled: true },
            Rule { target: Target::Service("telegram".into()), route: Route::Vpn, enabled: true },
        ];
        assert!(user_sets(&s, &catalog(), Os::Windows).is_none());
        s.lists.selected = vec![
            Rule { target: Target::Domain("telegram.org".into()), route: Route::Block, enabled: true },
            Rule { target: Target::Service("telegram".into()), route: Route::Vpn, enabled: true },
        ];
        assert!(user_sets(&s, &catalog(), Os::Windows).is_none());
    }

    #[test]
    fn nesting_checks() {
        assert!(folder_inside(r"C:\Games\Roblox", r"c:\games\"));
        assert!(!folder_inside(r"C:\GamesX", r"C:\Games"));
        assert!(domain_inside("a.b.example.com", "example.com") && domain_inside("Example.com", "example.com"));
        assert!(!domain_inside("badexample.com", "example.com"));
        assert!(cidr_inside("149.154.160.0/22", "149.154.160.0/20"));
        assert!(!cidr_inside("149.154.160.0/20", "149.154.160.0/22"));
        assert!(!cidr_inside("10.0.0.0/8", "11.0.0.0/8"));
        assert!(cidr_inside("2001:b28:f23d:1::/64", "2001:b28:f23d::/48"));
        assert!(!cidr_inside("10.0.0.0/8", "::/0"), "IPv4 и IPv6 не вкладываются");
        assert!(cidr_inside("1.2.3.4", "0.0.0.0/0"));
    }
}
