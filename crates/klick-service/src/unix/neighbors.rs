//! Соседи на Mac: обход блокировок (zapret/tpws, SpoofDPI, ByeDPI), другие VPN и прокси-ядра.
//! Они перехватывают тот же трафик, что и kl!ck, поэтому окно подсказывает, что с ними сделать.

use crate::sys::{self, TUN_ADDR};
use std::net::IpAddr;
use std::path::Path;

pub use klick_proto::NeighborView as Neighbor;

/// Имя процесса (без учёта регистра) → вид и название.
const KNOWN: [(&str, &str, &str); 44] = [
    ("tpws", "dpi_bypass", "zapret"),
    ("nfqws", "dpi_bypass", "zapret"),
    ("spoofdpi", "dpi_bypass", "SpoofDPI"),
    ("ciadpi", "dpi_bypass", "ByeDPI"),
    ("byedpi", "dpi_bypass", "ByeDPI"),
    ("goodbyedpi", "dpi_bypass", "GoodbyeDPI"),
    ("wireguard-go", "vpn", "WireGuard"),
    ("wireguard", "vpn", "WireGuard"),
    ("openvpn", "vpn", "OpenVPN"),
    ("tunnelblick", "vpn", "Tunnelblick"),
    ("viscosity", "vpn", "Viscosity"),
    ("amneziavpn", "vpn", "AmneziaVPN"),
    ("amneziawg", "vpn", "AmneziaWG"),
    ("outline", "vpn", "Outline"),
    // Служба WARP работает и тогда, когда сам WARP выключен в строке меню: трафик она не трогает,
    // но включённый WARP перехватывает всё, в том числе соединения ядра kl!ck с сервером.
    ("cloudflarewarp", "vpn", "Cloudflare WARP"),
    ("cloudflare warp", "vpn", "Cloudflare WARP"),
    ("mullvad-daemon", "vpn", "Mullvad"),
    ("mullvad vpn", "vpn", "Mullvad"),
    ("protonvpn", "vpn", "Proton VPN"),
    ("proton vpn", "vpn", "Proton VPN"),
    ("nordvpn", "vpn", "NordVPN"),
    ("expressvpn", "vpn", "ExpressVPN"),
    ("windscribe", "vpn", "Windscribe"),
    ("tailscaled", "vpn", "Tailscale"),
    ("tailscale", "vpn", "Tailscale"),
    ("adguard vpn", "vpn", "AdGuard VPN"),
    ("v2raytun", "proxy_core", "v2RayTun"),
    ("streisand", "proxy_core", "Streisand"),
    ("foxray", "proxy_core", "FoXray"),
    ("shadowrocket", "proxy_core", "Shadowrocket"),
    ("karing", "proxy_core", "Karing"),
    ("sing-box", "proxy_core", "sing-box"),
    ("xray", "proxy_core", "Xray"),
    ("v2ray", "proxy_core", "V2Ray"),
    ("v2rayu", "proxy_core", "V2rayU"),
    ("v2rayn", "proxy_core", "v2rayN"),
    ("hiddify", "proxy_core", "Hiddify"),
    ("happ", "proxy_core", "Happ"),
    ("clashx", "proxy_core", "ClashX"),
    ("clashx pro", "proxy_core", "ClashX Pro"),
    ("clash verge", "proxy_core", "Clash Verge"),
    ("flclash", "proxy_core", "FlClash"),
    ("stash", "proxy_core", "Stash"),
    ("surge", "proxy_core", "Surge"),
];

/// Что сейчас работает рядом. `own_core` — наше ядро, его не считаем.
pub fn scan(own_core: &Path) -> Vec<Neighbor> {
    let mut out = Vec::new();
    for pid in sys::pids() {
        let Some(path) = sys::process_path(pid) else { continue };
        let name = Path::new(&path).file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        if let Some((_, kind, title)) = KNOWN.iter().find(|(exe, _, _)| *exe == name) {
            push(&mut out, Neighbor { kind: (*kind).into(), name: (*title).into(), conflicts_with_tun: *kind == "dpi_bypass" || *kind == "vpn", conflicts_with_proxy: false });
        } else if name == "mihomo" || name.starts_with("clash") || name.starts_with("verge-mihomo") {
            if Path::new(&path) != own_core {
                push(&mut out, Neighbor { kind: "proxy_core".into(), name: name.clone(), conflicts_with_tun: false, conflicts_with_proxy: false });
            }
        }
    }
    for adapter in vpn_adapters() {
        push(&mut out, Neighbor { kind: "vpn_adapter".into(), name: adapter, conflicts_with_tun: true, conflicts_with_proxy: false });
    }
    out
}

fn push(out: &mut Vec<Neighbor>, n: Neighbor) {
    if !out.contains(&n) {
        out.push(n);
    }
}

/// Поднятые туннели чужих VPN. У macOS всегда есть несколько `utun` для своих служб (iCloud, Handoff),
/// но у них только адреса fe80::; туннель VPN получает адрес IPv4. Встроенные VPN — `ppp` и `ipsec`.
fn vpn_adapters() -> Vec<String> {
    let mut out: Vec<String> = sys::interfaces()
        .into_iter()
        .filter(|i| i.up && matches!(i.addr, IpAddr::V4(v4) if v4 != TUN_ADDR))
        .filter(|i| ["utun", "ppp", "ipsec", "tun", "tap", "wg"].iter().any(|p| i.name.starts_with(p)))
        .map(|i| i.name)
        .collect();
    out.sort();
    out.dedup();
    out
}
