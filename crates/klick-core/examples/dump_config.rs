//! Печатает пример конфига для проверки ядром:
//! `cargo run -p klick-core --example dump_config -- tun|proxy|tester|guard selected|all_vpn [windows|macos]`.

use klick_core::compile::{compile, compile_guard, compile_tester, Capture, CompileInput, CoreLayout, SetFiles};
use klick_core::{Catalog, Os, Route, Routing, Rule, Service, Settings, Target};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let capture = args.get(1).map(String::as_str).unwrap_or("proxy");
    let routing = match args.get(2).map(String::as_str) {
        Some("all_vpn") => Routing::AllVpn,
        _ => Routing::Selected,
    };
    let os = match args.get(3).map(String::as_str) {
        Some("windows") => Os::Windows,
        Some("macos") => Os::MacOs,
        _ => Os::CURRENT,
    };
    let layout = CoreLayout {
        os,
        controller: match os {
            Os::Windows => r"\\.\pipe\klick-check".into(),
            Os::MacOs => "/tmp/klick-check.sock".into(),
        },
        secret: "check".into(),
        mixed_port: 27890,
        check_vpn_port: 27891,
        check_direct_port: 27892,
        tun_device: "klick".into(),
        log_level: "info".into(),
        core_exe: String::new(),
    };
    let mut settings = Settings { routing, ..Settings::default() };
    let rules = match os {
        Os::Windows => {
            settings.kill_switch.programs.push(r"C:\Program Files (x86)\Roblox, Inc".into());
            vec![
                Rule { target: Target::Service("telegram".into()), route: Route::Vpn, enabled: true },
                Rule { target: Target::Program(r"C:\Users\user\AppData\Local\Discord".into()), route: Route::Direct, enabled: true },
                Rule { target: Target::Program(r"C:\Program Files\WindowsApps\Claude_2.9939.2.0_x64__pzs8sxrjxfjjc\app".into()), route: Route::Vpn, enabled: true },
                Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: true },
                Rule { target: Target::Ip("1.2.3.0/24".into()), route: Route::Block, enabled: true },
            ]
        }
        Os::MacOs => {
            settings.kill_switch.programs.push("/Applications/Roblox, Inc.app".into());
            vec![
                Rule { target: Target::Service("telegram".into()), route: Route::Vpn, enabled: true },
                Rule { target: Target::Program("/Applications/Discord.app".into()), route: Route::Direct, enabled: true },
                Rule { target: Target::Program("/opt/homebrew/Cellar/node".into()), route: Route::Vpn, enabled: true },
                Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: true },
                Rule { target: Target::Ip("1.2.3.0/24".into()), route: Route::Block, enabled: true },
            ]
        }
    };
    settings.lists.selected = rules.clone();
    settings.lists.all_vpn = rules;
    let catalog = Catalog {
        services: vec![Service {
            id: "telegram".into(),
            name: "Telegram".into(),
            domains: vec!["telegram.org".into(), "t.me".into()],
            cidrs: vec!["149.154.160.0/20".into(), "2001:b28:f23d::/48".into()],
        }],
    };
    let sets = SetFiles {
        blocked_domains: "sets/blocked-domains.txt".into(),
        blocked_ips: "sets/blocked-ips.txt".into(),
        ru_domains: "sets/ru-domains.txt".into(),
    };
    let cfg = match capture {
        "tester" => compile_tester(&layout, "providers/test.txt"),
        "guard" => compile_guard(&settings, &layout),
        other => compile(&CompileInput {
            settings: &settings,
            catalog: &catalog,
            layout: &layout,
            capture: if other == "tun" { Capture::Tun } else { Capture::Proxy },
            provider_path: "providers/test.txt",
            sets: &sets,
            // Как на Mac с роутером: DNS системы первым.
            server_dns: &["192.168.1.1".to_string()],
        }),
    };
    println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
}
