//! Конфиг mihomo. Пишется JSON'ом: YAML — надмножество JSON, mihomo читает
//! его как есть, а у нас нет риска сломать отступы.
//!
//! Что взято из опыта SingBoxGUI/KlutzBOX: стек TUN — gvisor (со стеком
//! system при второй VPN в системе пакеты к собственному серверу уходили
//! обратно в туннель); адреса самих серверов резолвятся не через туннель.

use serde_json::{json, Value};

use crate::state::{Action, Mode, Profile, Settings};

pub const GROUP: &str = "PROXY";

/// Частные сети — «Локальная сеть напрямую».
const LAN: [&str; 7] = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8", "169.254.0.0/16", "100.64.0.0/10", "224.0.0.0/4"];

pub struct Built {
    pub config: Value,
    pub rules: usize,
}

pub fn build(profile: &Profile, settings: &Settings, controller_port: u16, secret: &str) -> Result<Built, String> {
    if profile.proxies.is_empty() {
        return Err(format!("В «{}» нет ни одного сервера.", profile.name));
    }
    let names: Vec<String> = profile.proxies.iter().filter_map(|p| p.get("name").and_then(Value::as_str).map(String::from)).collect();
    let active = profile.active_proxy().and_then(|p| p.get("name")).and_then(Value::as_str).unwrap_or(&names[0]).to_string();
    // Выбранный сервер — первым: у select-группы по умолчанию выбран первый.
    let mut members = vec![active.clone()];
    members.extend(names.iter().filter(|n| **n != active).cloned());

    let rules = rules(settings);
    let rule_count = rules.len();
    let tun = settings.mode == Mode::Tun;

    let config = json!({
        "mixed-port": settings.proxy_port,
        "allow-lan": false,
        "bind-address": "127.0.0.1",
        "mode": settings.route_mode.as_str(),
        "log-level": "info",
        "ipv6": false,
        "unified-delay": true,
        "tcp-concurrent": true,
        "find-process-mode": "always",
        "external-controller": format!("127.0.0.1:{controller_port}"),
        "secret": secret,
        "geodata-mode": false,
        "geo-auto-update": false,
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
            "fake-ip-filter": ["*.lan", "*.local", "+.msftconnecttest.com", "+.msftncsi.com", "time.windows.com", "+.stun.*.*"],
            // Запросы идут по тем же правилам: заблокированное — через
            // туннель, .ru — напрямую.
            "respect-rules": true,
            "default-nameserver": ["77.88.8.8", "1.1.1.1"],
            "nameserver": ["https://1.1.1.1/dns-query", "https://8.8.8.8/dns-query"],
            // Адреса самих серверов — напрямую, иначе сервер, заданный
            // доменом, не подключится: для этого нужен уже работающий туннель.
            "proxy-server-nameserver": ["https://77.88.8.8/dns-query", "https://1.1.1.1/dns-query"],
            "direct-nameserver": ["https://77.88.8.8/dns-query", "77.88.8.8"],
        },
        "proxies": profile.proxies,
        "proxy-groups": [{ "name": GROUP, "type": "select", "proxies": members }],
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

/// Порядок важен — mihomo берёт первое совпавшее: программы важнее сайтов,
/// свои правила важнее быстрых исключений.
pub fn rules(s: &Settings) -> Vec<String> {
    let mut r = vec![];
    for a in &s.apps {
        let exe = a.exe.trim();
        if !exe.is_empty() && !exe.contains(',') {
            r.push(format!("PROCESS-NAME,{exe},{}", target(a.action)));
        }
    }
    for site in &s.sites {
        let p = site.pattern.trim().trim_start_matches('.');
        if !p.is_empty() && !p.contains(',') {
            r.push(format!("DOMAIN-SUFFIX,{p},{}", target(site.action)));
        }
    }
    if s.presets.lan {
        r.push(format!("DOMAIN-SUFFIX,local,DIRECT"));
        r.push(format!("DOMAIN-SUFFIX,lan,DIRECT"));
        for net in LAN {
            r.push(format!("IP-CIDR,{net},DIRECT,no-resolve"));
        }
        r.push("IP-CIDR6,fc00::/7,DIRECT,no-resolve".into());
        r.push("IP-CIDR6,fe80::/10,DIRECT,no-resolve".into());
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
    use crate::state::{AppRule, Kind, Presets, RouteMode, SiteRule};

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
        let b = build(&prof(), &Settings::default(), 9090, "s").unwrap();
        assert_eq!(b.config["proxy-groups"][0]["proxies"], json!(["DE", "NL"]));
        assert_eq!(b.config["tun"]["enable"], true);
        assert_eq!(b.config["tun"]["stack"], "gvisor");
        assert_eq!(b.config["dns"]["enhanced-mode"], "fake-ip");
    }

    #[test]
    fn порядок_правил() {
        let s = Settings {
            mode: Mode::Proxy,
            route_mode: RouteMode::Global,
            apps: vec![AppRule { name: "Telegram".into(), exe: "Telegram.exe".into(), action: Action::Proxy }],
            sites: vec![SiteRule { pattern: ".ru".into(), action: Action::Block }, SiteRule { pattern: "gosuslugi.ru".into(), action: Action::Direct }],
            presets: Presets { ru: true, lan: false, geoip: true },
            ..Default::default()
        };
        let r = rules(&s);
        assert_eq!(r[0], "PROCESS-NAME,Telegram.exe,PROXY");
        assert_eq!(r[1], "DOMAIN-SUFFIX,ru,REJECT");
        assert_eq!(r[2], "DOMAIN-SUFFIX,gosuslugi.ru,DIRECT");
        assert!(r.contains(&"DOMAIN-SUFFIX,xn--p1ai,DIRECT".to_string()));
        assert!(!r.iter().any(|x| x.starts_with("IP-CIDR")));
        assert_eq!(r[r.len() - 2], "GEOIP,RU,DIRECT");
        assert_eq!(r.last().unwrap(), "MATCH,PROXY");
        let c = build(&prof(), &s, 1, "").unwrap().config;
        assert_eq!(c["mode"], "global");
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
        let s = Settings { presets: Presets { ru: true, lan: true, geoip: true }, apps: vec![AppRule { name: "Telegram".into(), exe: "Telegram.exe".into(), action: Action::Proxy }], ..Default::default() };
        for (name, cfg) in [("full", build(&p, &s, 9090, "s").unwrap().config), ("probe", probe(&p.proxies, 9091, "s"))] {
            let f = dir.join(format!("{name}.yaml"));
            std::fs::write(&f, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
            let out = std::process::Command::new(&exe).arg("-t").arg("-d").arg(&dir).arg("-f").arg(&f).output().unwrap();
            let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
            println!("{name}: {}", text.trim());
            assert!(out.status.success() && text.contains("successful"), "{name}: {text}");
        }
    }

    #[test]
    fn пустое_подключение() {
        let mut p = prof();
        p.proxies.clear();
        assert!(build(&p, &Settings::default(), 1, "").is_err());
    }
}
