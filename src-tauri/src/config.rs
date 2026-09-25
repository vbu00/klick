//! Конфиг mihomo. Пишется JSON'ом: YAML — надмножество JSON, mihomo читает
//! его как есть, а у нас нет риска сломать отступы.
//!
//! Что взято из опыта SingBoxGUI/KlutzBOX: стек TUN — gvisor (со стеком
//! system при второй VPN в системе пакеты к собственному серверу уходили
//! обратно в туннель); адреса самих серверов резолвятся не через туннель.

use serde_json::{json, Value};

use std::net::IpAddr;

use crate::state::{Action, DefaultRoute, Mode, Profile, Settings};

pub const GROUP: &str = "PROXY";

/// Частные сети — «Локальная сеть напрямую».
const LAN: [&str; 7] = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8", "169.254.0.0/16", "100.64.0.0/10", "224.0.0.0/4"];

pub struct Built {
    pub config: Value,
    pub rules: usize,
}

/// `core_exe` — путь к нашему mihomo.exe: его зонд задержки идёт напрямую.
pub fn build(profile: &Profile, settings: &Settings, controller_port: u16, secret: &str, core_exe: &str) -> Result<Built, String> {
    if profile.proxies.is_empty() {
        return Err(format!("В «{}» нет ни одного сервера.", profile.name));
    }
    let names: Vec<String> = profile.proxies.iter().filter_map(|p| p.get("name").and_then(Value::as_str).map(String::from)).collect();
    let active = profile.active_proxy().and_then(|p| p.get("name")).and_then(Value::as_str).unwrap_or(&names[0]).to_string();
    // Выбранный сервер — первым: у select-группы по умолчанию выбран первый.
    let mut members = vec![active.clone()];
    members.extend(names.iter().filter(|n| **n != active).cloned());

    let rules = rules(settings, core_exe);
    let rule_count = rules.len();
    let tun = settings.mode == Mode::Tun;

    let config = json!({
        // Порт открыт и в TUN: кто-то прописал 127.0.0.1:7890 в программе
        // руками. Занят — mihomo пишет ошибку в лог и работает дальше.
        "mixed-port": settings.proxy_port,
        "allow-lan": false,
        "bind-address": "127.0.0.1",
        "mode": "rule",
        "log-level": "info",
        // IPv6 выключен: с ним TUN забирает ::/0, и если сервер IPv6 не умеет,
        // сайты, чей адрес программа узнала сама (свой DoH), виснут — TUN
        // принимает соединение, и браузер не откатывается на IPv4.
        "ipv6": false,
        "unified-delay": true,
        "tcp-concurrent": true,
        "find-process-mode": "always",
        "external-controller": format!("127.0.0.1:{controller_port}"),
        "secret": secret,
        "geodata-mode": false,
        // Свежая база GeoIP — забота mihomo, пока пресет включён (см. geo.rs).
        "geo-auto-update": settings.presets.geoip,
        "geo-update-interval": crate::geo::INTERVAL_HOURS,
        "geox-url": { "mmdb": crate::geo::MMDB_URL },
        "profile": { "store-selected": false, "store-fake-ip": false },
        "tun": {
            "enable": tun,
            "stack": "gvisor",
            "auto-route": true,
            "auto-detect-interface": true,
            "strict-route": true,
            "dns-hijack": ["any:53"],
            "mtu": 9000,
        },
        "dns": {
            "enable": true,
            "ipv6": false,
            "listen": "127.0.0.1:1053",
            "enhanced-mode": if tun { "fake-ip" } else { "redir-host" },
            "fake-ip-range": "198.18.0.1/16",
            "fake-ip-filter": ["*.lan", "*.local", "+.localdomain", "+.msftconnecttest.com", "+.msftncsi.com", "time.windows.com", "+.stun.*.*", "+.home.arpa"],
            // Запросы идут по тем же правилам: заблокированное — через
            // туннель, .ru — напрямую.
            "respect-rules": true,
            "default-nameserver": ["77.88.8.8", "1.1.1.1"],
            "nameserver": ["https://1.1.1.1/dns-query", "https://8.8.8.8/dns-query"],
            // Имена в домашней сети (router.lan, nas.home.arpa) знает только
            // DNS роутера — Cloudflare о них не слышал. «system» — DNS
            // сетевого адаптера, как без VPN.
            "nameserver-policy": { "+.lan": "system", "+.home.arpa": "system", "+.localdomain": "system", "+.local": "system" },
            // Адреса самих серверов — напрямую, иначе сервер, заданный
            // доменом, не подключится: для этого нужен уже работающий туннель.
            // Яндекс по голому IP DoH не отдаёт (пустой ответ), а DoT — да.
            // Обычный UDP — запасной: DoT и DoH закрыты в иных сетях.
            "proxy-server-nameserver": ["tls://77.88.8.8:853", "https://1.1.1.1/dns-query", "77.88.8.8"],
            "direct-nameserver": ["tls://77.88.8.8:853", "77.88.8.8"],
        },
        // Домен из самого соединения (SNI, Host): правила по сайтам работают,
        // даже если программа резолвила имя сама — своим DoH, через hosts или
        // зашитым адресом. Адрес назначения не подменяем, только сверяем правила.
        "sniffer": {
            "enable": true,
            "force-dns-mapping": true,
            "parse-pure-ip": true,
            "override-destination": false,
            "sniff": { "HTTP": { "ports": [80] }, "TLS": { "ports": [443] }, "QUIC": { "ports": [443] } },
        },
        "proxies": profile.proxies,
        "proxy-groups": [
            { "name": GROUP, "type": "select", "proxies": members },
            // Режим у нас всегда rule, но если mihomo когда-нибудь окажется в
            // global (через API), встроенная GLOBAL без выбранного участника
            // уходит в DIRECT. Своя группа — страховка.
            { "name": "GLOBAL", "type": "select", "proxies": [GROUP] },
        ],
        "rules": rules,
    });
    Ok(Built { config, rules: rule_count })
}

fn target(a: Action) -> &'static str {
    match a {
        Action::Proxy => GROUP,
        Action::Direct => "DIRECT",
        Action::Block => "REJECT",
    }
}

/// Регулярка «ровно этот путь, без учёта регистра» для PROCESS-PATH-REGEX.
fn path_regex(path: &str) -> String {
    let mut r = String::from("(?i)^");
    for c in path.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            r.push('\\');
        }
        r.push(c);
    }
    r.push('$');
    r
}

/// Порядок важен — mihomo берёт первое совпавшее: программы важнее сайтов,
/// свои правила важнее быстрых исключений.
///
/// «Что по умолчанию» решает последнее правило: всё через VPN (правила —
/// исключения) или только выбранное через VPN (остальное напрямую). Во
/// втором случае быстрые исключения .ru / GeoIP не нужны: напрямую и так
/// всё, что не выбрано.
///
/// Kill Switch сюда не попадает: он о том, что делать, когда VPN нет, а
/// куда идёт трафик при включённом — решают только эти правила.
pub fn rules(s: &Settings, core_exe: &str) -> Vec<String> {
    let only_chosen = s.default_route == DefaultRoute::Direct;
    let mut r = vec![];
    // Замер задержки другого подключения (временный mihomo из нашей же
    // папки) — напрямую, а не через текущий сервер. Собственные соединения
    // основного ядра в TUN не попадают. По полному пути, а не по имени: у
    // других клиентов ядро тоже бывает mihomo.exe.
    let core_exe = core_exe.trim_start_matches(r"\\?\");
    if !core_exe.is_empty() && !core_exe.contains(',') {
        r.push(format!("PROCESS-PATH-REGEX,{},DIRECT", path_regex(core_exe)));
    }
    // Локальная сеть — всегда напрямую и раньше своих правил: программе
    // «через VPN» локальные адреса (соседи в торренте, принтер) тоже нужны.
    r.push("DOMAIN-SUFFIX,local,DIRECT".into());
    r.push("DOMAIN-SUFFIX,lan,DIRECT".into());
    r.push("DOMAIN-SUFFIX,home.arpa,DIRECT".into());
    for net in LAN {
        r.push(format!("IP-CIDR,{net},DIRECT,no-resolve"));
    }
    r.push("IP-CIDR6,fc00::/7,DIRECT,no-resolve".into());
    r.push("IP-CIDR6,fe80::/10,DIRECT,no-resolve".into());
    // Проверка «есть ли интернет» Windows — напрямую: сервер лёг — значок
    // сети не покажет «нет интернета» у всего компьютера, и программы,
    // которые на него смотрят, не уйдут в офлайн.
    r.push("DOMAIN-SUFFIX,msftconnecttest.com,DIRECT".into());
    r.push("DOMAIN-SUFFIX,msftncsi.com,DIRECT".into());
    for a in &s.apps {
        let exe = a.exe.trim();
        if !exe.is_empty() && !exe.contains(',') {
            r.push(format!("PROCESS-NAME,{exe},{}", target(a.action)));
        }
    }
    for site in &s.sites {
        let p = site.pattern.trim().trim_start_matches('.');
        if p.is_empty() || p.contains(',') {
            continue;
        }
        // Адрес вместо домена: DOMAIN-SUFFIX на него никогда не сработал бы.
        match p.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => r.push(format!("IP-CIDR,{ip}/32,{},no-resolve", target(site.action))),
            Ok(IpAddr::V6(ip)) => r.push(format!("IP-CIDR6,{ip}/128,{},no-resolve", target(site.action))),
            Err(_) => r.push(format!("DOMAIN-SUFFIX,{p},{}", target(site.action))),
        }
    }
    if only_chosen {
        r.push("MATCH,DIRECT".into());
        return r;
    }
    if s.presets.ru {
        // .рф и .дети — в punycode.
        for zone in ["ru", "su", "xn--p1ai", "xn--d1acj3b"] {
            r.push(format!("DOMAIN-SUFFIX,{zone},DIRECT"));
        }
    }
    if s.presets.geoip {
        r.push("GEOIP,RU,DIRECT".into());
    }
    r.push(format!("MATCH,{GROUP}"));
    r
}

/// Конфиг для проверки задержки без подключения: те же серверы, никакого
/// TUN, прокси и DNS — только API.
pub fn probe(proxies: &[Value], controller_port: u16, secret: &str) -> Value {
    let names: Vec<Value> = proxies.iter().filter_map(|p| p.get("name").cloned()).collect();
    json!({
        "log-level": "warning",
        "ipv6": false,
        "external-controller": format!("127.0.0.1:{controller_port}"),
        "secret": secret,
        "geo-auto-update": false,
        "profile": { "store-selected": false },
        "proxies": proxies,
        "proxy-groups": [{ "name": GROUP, "type": "select", "proxies": names }],
        "rules": [format!("MATCH,{GROUP}")],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppRule, Kind, KsApp, KsSite, Presets, SiteRule};

    const CORE: &str = r"C:\Program Files\kl!ck\bin\mihomo.exe";

    /// Правила без служебных (зонд, локальная сеть, проверка сети Windows).
    fn own(r: &[String]) -> Vec<String> {
        r.iter().filter(|x| !x.contains("PROCESS-PATH-REGEX") && !x.ends_with("DIRECT,no-resolve") && !["local", "lan", "home.arpa", "msftconnecttest.com", "msftncsi.com"].iter().any(|d| *x == &format!("DOMAIN-SUFFIX,{d},DIRECT"))).cloned().collect()
    }

    fn prof() -> Profile {
        Profile {
            id: "p".into(),
            kind: Kind::Sub,
            name: "Sub".into(),
            url: None,
            proxies: vec![
                json!({"name": "NL", "type": "vless", "server": "1.1.1.1", "port": 443, "uuid": "u"}),
                json!({"name": "DE", "type": "vless", "server": "2.2.2.2", "port": 443, "uuid": "u"}),
            ],
            active: Some("DE".into()),
            info: None,
            updated_at: None,
            update_hours: None,
        }
    }

    #[test]
    fn группа_начинается_с_выбранного() {
        let b = build(&prof(), &Settings::default(), 9090, "s", CORE).unwrap();
        assert_eq!(b.config["proxy-groups"][0]["proxies"], json!(["DE", "NL"]));
        assert_eq!(b.config["tun"]["enable"], true);
        assert_eq!(b.config["tun"]["stack"], "gvisor");
        assert_eq!(b.config["dns"]["enhanced-mode"], "fake-ip");
        assert_eq!(b.config["mode"], "rule");
        assert_eq!(b.config["ipv6"], false, "с IPv6 сайты виснут, если сервер его не умеет");
        assert_eq!(b.config["mixed-port"], 7890, "порт нужен и в TUN");
        assert_eq!(b.config["sniffer"]["enable"], true);
        // Страховка на случай mode: global через API.
        assert_eq!(b.config["proxy-groups"][1], json!({"name": "GLOBAL", "type": "select", "proxies": ["PROXY"]}));
    }

    #[test]
    fn служебные_правила_первыми() {
        let r = rules(&Settings::default(), &format!(r"\\?\{CORE}"));
        assert_eq!(r[0], r"PROCESS-PATH-REGEX,(?i)^C:\\Program Files\\kl!ck\\bin\\mihomo\.exe$,DIRECT");
        assert!(r.contains(&"IP-CIDR,192.168.0.0/16,DIRECT,no-resolve".to_string()), "локальная сеть — всегда");
        assert!(r.contains(&"DOMAIN-SUFFIX,msftconnecttest.com,DIRECT".to_string()));
        assert_eq!(r.last().unwrap(), "MATCH,PROXY");
    }

    #[test]
    fn kill_switch_не_меняет_маршруты() {
        let base = Settings {
            apps: vec![AppRule { name: "Steam".into(), exe: "steam.exe".into(), action: Action::Direct }],
            sites: vec![SiteRule { pattern: "1.2.3.4".into(), action: Action::Proxy }],
            ..Default::default()
        };
        let with_ks = Settings {
            kill_switch: true,
            ks_apps: vec![KsApp { name: "Roblox".into(), exe: "RobloxPlayerBeta.exe".into(), path: r"C:\Roblox\RobloxPlayerBeta.exe".into(), on: true }],
            ks_sites: vec![KsSite { pattern: "sberbank.ru".into(), on: true }],
            ..base.clone()
        };
        assert_eq!(rules(&with_ks, CORE), rules(&base, CORE));
        assert_eq!(own(&rules(&base, CORE))[..2], ["PROCESS-NAME,steam.exe,DIRECT", "IP-CIDR,1.2.3.4/32,PROXY,no-resolve"]);
    }

    #[test]
    fn порядок_правил() {
        let s = Settings {
            mode: Mode::Proxy,
            apps: vec![AppRule { name: "Telegram".into(), exe: "Telegram.exe".into(), action: Action::Proxy }],
            sites: vec![SiteRule { pattern: ".ru".into(), action: Action::Block }, SiteRule { pattern: "gosuslugi.ru".into(), action: Action::Direct }],
            presets: Presets { ru: true, geoip: true },
            ..Default::default()
        };
        let r = own(&rules(&s, CORE));
        assert_eq!(r[0], "PROCESS-NAME,Telegram.exe,PROXY");
        assert_eq!(r[1], "DOMAIN-SUFFIX,ru,REJECT");
        assert_eq!(r[2], "DOMAIN-SUFFIX,gosuslugi.ru,DIRECT");
        assert!(r.contains(&"DOMAIN-SUFFIX,xn--p1ai,DIRECT".to_string()));
        assert_eq!(r[r.len() - 2], "GEOIP,RU,DIRECT");
        assert_eq!(r.last().unwrap(), "MATCH,PROXY");
        let c = build(&prof(), &s, 1, "", CORE).unwrap().config;
        assert_eq!(c["mixed-port"], 7890);
        assert_eq!(c["tun"]["enable"], false);
        assert_eq!(c["dns"]["enhanced-mode"], "redir-host");
    }

    /// mihomo сам проверяет конфиг (`-t`): cargo test --lib mihomo_ -- --ignored
    #[test]
    #[ignore]
    fn mihomo_принимает_конфиг() {
        let exe = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin").join("mihomo.exe");
        let dir = std::env::temp_dir().join("klick-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(exe.with_file_name("Country.mmdb"), dir.join("Country.mmdb")).unwrap();
        let mut p = prof();
        p.proxies = crate::links::parse_any(concat!(
            "vless://1b2c3d4e-0000-4000-8000-000000000001@203.0.113.5:443?type=tcp&security=reality&pbk=Q2sDgDCjyNy_9Ydp1s7yI3i7WEKtMP3vOIZ2Ahj1OxM&fp=chrome&sni=www.vk.ru&sid=ab12&flow=xtls-rprx-vision#R
",
            "trojan://pw@t.example:443?type=grpc&serviceName=svc&sni=t.example#T
",
            "hy2://secret@hy.example:8443?sni=hy.example&obfs=salamander&obfs-password=x#H
",
            "ss://2022-blake3-aes-128-gcm:MTIzNDU2Nzg5MDEyMzQ1Ng%3D%3D@5.6.7.8:443#S
",
        )).unwrap();
        p.active = Some("T".into());
        let s = Settings { presets: Presets { ru: true, geoip: true }, apps: vec![AppRule { name: "Telegram".into(), exe: "Telegram.exe".into(), action: Action::Proxy }], ..Default::default() };
        for (name, cfg) in [("full", build(&p, &s, 9090, "s", exe.to_str().unwrap()).unwrap().config), ("probe", probe(&p.proxies, 9091, "s"))] {
            let f = dir.join(format!("{name}.yaml"));
            std::fs::write(&f, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
            let out = std::process::Command::new(&exe).arg("-t").arg("-d").arg(&dir).arg("-f").arg(&f).output().unwrap();
            let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
            println!("{name}: {}", text.trim());
            assert!(out.status.success() && text.contains("successful"), "{name}: {text}");
        }
    }

    #[test]
    fn только_выбранное_через_vpn() {
        let s = Settings {
            default_route: DefaultRoute::Direct,
            apps: vec![AppRule { name: "Discord".into(), exe: "Discord.exe".into(), action: Action::Proxy }, AppRule { name: "CS2".into(), exe: "cs2.exe".into(), action: Action::Direct }],
            sites: vec![SiteRule { pattern: "youtube.com".into(), action: Action::Proxy }],
            presets: Presets { ru: true, geoip: true },
            ..Default::default()
        };
        let r = own(&rules(&s, CORE));
        assert_eq!(r, vec!["PROCESS-NAME,Discord.exe,PROXY", "PROCESS-NAME,cs2.exe,DIRECT", "DOMAIN-SUFFIX,youtube.com,PROXY", "MATCH,DIRECT"]);
    }

    #[test]
    fn пустое_подключение() {
        let mut p = prof();
        p.proxies.clear();
        assert!(build(&p, &Settings::default(), 1, "", CORE).is_err());
    }
}
