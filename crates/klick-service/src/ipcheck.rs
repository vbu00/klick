//! «Как вас видят сайты»: две колонки, через VPN и напрямую, и проверка утечек.
//!
//! Запросы идут через служебные входы ядра с принудительным маршрутом, поэтому колонка
//! «напрямую» честная даже при «Всё через VPN». Проверка запускается только по команде.

use klick_proto::{ConnView, FailureView, IpColumn, IpReport};
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Что знает движок о текущем подключении.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Служебный вход «через VPN»; `None` — VPN выключен.
    pub vpn_port: Option<u16>,
    /// Служебный вход «напрямую».
    pub direct_port: Option<u16>,
    pub tun: bool,
    pub all_vpn: bool,
}

const IP_V4: &str = "https://api.ipify.org";
const IP_V6: &str = "https://api6.ipify.org";
/// Страна, город, провайдер и признак VPN; без ключа — ограниченное число запросов в сутки.
const PROXYCHECK: &str = "https://proxycheck.io/v2/";
const DOH_JSON: &str = "https://cloudflare-dns.com/dns-query";

fn client(port: Option<u16>) -> Option<reqwest::Client> {
    let b = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(6))
        .user_agent("klick");
    let b = match port {
        Some(p) => b.proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{p}")).ok()?),
        None => b.no_proxy(),
    };
    b.build().ok()
}

pub async fn run(plan: Plan) -> IpReport {
    let direct_port = if plan.vpn_port.is_some() { plan.direct_port } else { None };
    let vpn = async {
        match plan.vpn_port {
            Some(p) => Some(column(Some(p)).await),
            None => None,
        }
    };
    let (via_vpn, direct, dns_protected) = tokio::join!(vpn, column(direct_port), dns_check(plan.tun));
    let ipv6_leak = if plan.tun && plan.all_vpn && plan.vpn_port.is_some() {
        let system_v6 = match client(None) {
            Some(c) => text(&c, IP_V6, 5).await,
            None => None,
        };
        Some(system_v6.is_some() && system_v6 == direct.ipv6)
    } else {
        None
    };
    IpReport { via_vpn, direct, ipv6_leak, dns_protected, checked_at: crate::engine::unix_now() }
}

async fn column(port: Option<u16>) -> IpColumn {
    let mut col = IpColumn::default();
    let Some(c) = client(port) else {
        col.error = Some("ip.client".into());
        return col;
    };
    let (v4, v6) = tokio::join!(text(&c, IP_V4, 8), text(&c, IP_V6, 5));
    col.ipv4 = v4.filter(|s| s.parse::<std::net::Ipv4Addr>().is_ok());
    col.ipv6 = v6.filter(|s| s.parse::<std::net::Ipv6Addr>().is_ok());
    let Some(ip) = col.ipv4.clone().or_else(|| col.ipv6.clone()) else {
        col.error = Some("ip.unreachable".into());
        return col;
    };
    let url = format!("{PROXYCHECK}{ip}?vpn=1&asn=1");
    let (info, ptr) = tokio::join!(json(&c, &url), reverse(&c, &ip));
    match info {
        Ok(v) if v["status"].as_str() == Some("denied") => col.error = Some("ip.limit".into()),
        Ok(v) => fill(&mut col, &v[ip.as_str()]),
        Err(code) => col.error = Some(code.into()),
    }
    col.reverse_dns = ptr;
    col
}

/// Ответ proxycheck.io по одному адресу.
fn fill(col: &mut IpColumn, v: &Value) {
    let s = |x: &Value| x.as_str().filter(|s| !s.is_empty()).map(str::to_string);
    col.country = s(&v["country"]);
    col.country_code = s(&v["isocode"]);
    col.city = s(&v["city"]);
    col.provider = s(&v["provider"]).or_else(|| s(&v["organisation"]));
    col.asn = v["asn"].as_str().and_then(|a| a.trim_start_matches("AS").parse().ok());
    col.vpn_detected = v["proxy"].as_str().map(|p| p.eq_ignore_ascii_case("yes"));
    col.datacenter = v["type"].as_str().map(|t| t.eq_ignore_ascii_case("hosting"));
}

async fn json(c: &reqwest::Client, url: &str) -> Result<Value, &'static str> {
    let res = c.get(url).send().await.map_err(|e| if e.is_timeout() { "ip.timeout" } else { "ip.unreachable" })?;
    if !res.status().is_success() {
        return Err("ip.http_status");
    }
    res.json::<Value>().await.map_err(|_| "ip.bad_answer")
}

async fn text(c: &reqwest::Client, url: &str, secs: u64) -> Option<String> {
    let res = c.get(url).timeout(Duration::from_secs(secs)).send().await.ok()?;
    let t = res.text().await.ok()?;
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// Обратное DNS-имя через DNS-over-HTTPS по тому же маршруту, что и колонка.
async fn reverse(c: &reqwest::Client, ip: &str) -> Option<String> {
    let name = match ip.parse::<IpAddr>().ok()? {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            format!("{}.{}.{}.{}.in-addr.arpa", o[3], o[2], o[1], o[0])
        }
        IpAddr::V6(v6) => {
            let hex: String = v6.octets().iter().map(|b| format!("{b:02x}")).collect();
            let nibbles: Vec<String> = hex.chars().rev().map(|c| c.to_string()).collect();
            format!("{}.ip6.arpa", nibbles.join("."))
        }
    };
    let res = c.get(DOH_JSON).query(&[("name", name.as_str()), ("type", "PTR")]).header("accept", "application/dns-json").send().await.ok()?;
    let v: Value = res.json().await.ok()?;
    v["Answer"].as_array()?.iter().find_map(|a| a["data"].as_str()).map(|d| d.trim_end_matches('.').to_string())
}

/// В режиме VPN (TUN) DNS-запросы программ перехватывает ядро и отвечает подменными адресами 198.18.x.x.
async fn dns_check(tun: bool) -> Option<bool> {
    if !tun {
        return None;
    }
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host("www.gstatic.com:443").await.ok()?.collect();
    Some(addrs.iter().any(|a| matches!(a.ip(), IpAddr::V4(v4) if v4.octets()[0] == 198 && (v4.octets()[1] & 0xFE) == 18)))
}

/// Соединения из API ядра — в понятный окну вид.
pub fn connections(v: &Value) -> Vec<ConnView> {
    let list = v["connections"].as_array().cloned().unwrap_or_default();
    let mut out: Vec<ConnView> = list
        .iter()
        .map(|c| {
            let m = &c["metadata"];
            let s = |x: &Value| x.as_str().filter(|s| !s.is_empty()).map(str::to_string);
            let host = s(&m["host"]).or_else(|| s(&m["sniffHost"])).or_else(|| s(&m["destinationIP"])).unwrap_or_default();
            let chains: Vec<&str> = c["chains"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            let route = if chains.contains(&"REJECT") || chains.contains(&"REJECT-DROP") {
                "block"
            } else if chains.contains(&"DIRECT") {
                "direct"
            } else {
                "vpn"
            };
            ConnView {
                host,
                process: s(&m["process"]),
                process_path: s(&m["processPath"]),
                route: route.into(),
                rule: s(&c["rule"]).unwrap_or_default(),
                network: s(&m["network"]).unwrap_or_default(),
                upload: c["upload"].as_u64().unwrap_or(0),
                download: c["download"].as_u64().unwrap_or(0),
            }
        })
        .collect();
    out.sort_by(|a, b| (b.download + b.upload).cmp(&(a.download + a.upload)));
    out
}

/// Строка журнала ядра о неудачном соединении: `[TCP] dial klick-vpn (…) a:1 --> host:443 error: …`.
pub fn parse_failure(payload: &str, at: i64) -> Option<FailureView> {
    let dial = payload.find("dial ")?;
    let error = payload.split_once(" error: ")?.1.trim().to_string();
    let via = payload[dial + 5..].split_whitespace().next()?;
    let target = payload.split_once("--> ")?.1.split_whitespace().next()?;
    let host = match target.strip_prefix('[') {
        Some(v6) => v6.split(']').next()?.to_string(),
        None => target.rsplit_once(':').map(|(h, _)| h).unwrap_or(target).to_string(),
    };
    // Запросы самого ядра к DNS-серверам человеку в «Не открывается?» не нужны.
    if ["1.1.1.1", "8.8.8.8", "77.88.8.8", "cloudflare-dns.com", "dns.google"].contains(&host.as_str()) {
        return None;
    }
    let route = match via {
        "DIRECT" => "direct",
        "REJECT" | "REJECT-DROP" => "block",
        _ => "vpn",
    };
    Some(FailureView { host, route: route.into(), error, at })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn proxycheck_answer_is_mapped() {
        let v = json!({
            "asn": "AS60068", "provider": "Datacamp Limited", "organisation": "CDN77",
            "country": "Netherlands", "isocode": "NL", "city": "Amsterdam", "proxy": "yes", "type": "VPN"
        });
        let mut col = IpColumn::default();
        fill(&mut col, &v);
        assert_eq!(col.country_code.as_deref(), Some("NL"));
        assert_eq!(col.provider.as_deref(), Some("Datacamp Limited"));
        assert_eq!(col.asn, Some(60068));
        assert_eq!(col.vpn_detected, Some(true));
        assert_eq!(col.datacenter, Some(false));
    }

    #[test]
    fn failures_are_parsed() {
        let f = parse_failure("[TCP] dial klick-vpn (match RuleSet/klick-blocked-domains) 127.0.0.1:52931 --> www.youtube.com:443 error: REALITY authentication failed", 7).unwrap();
        assert_eq!(f.host, "www.youtube.com");
        assert_eq!(f.route, "vpn");
        assert_eq!(f.error, "REALITY authentication failed");
        let d = parse_failure("[UDP] dial DIRECT (match Match/) 1.2.3.4:5 --> [2001:db8::1]:443 error: timeout", 8).unwrap();
        assert_eq!(d.host, "2001:db8::1");
        assert_eq!(d.route, "direct");
        assert!(parse_failure("Mixed(http+socks) proxy listening at: 127.0.0.1:7890", 0).is_none());
        assert!(parse_failure("[TCP] dial klick-vpn (match Match/) 1.2.3.4:5 --> 8.8.8.8:443 error: refused", 9).is_none());
    }

    #[test]
    fn connections_are_mapped() {
        let v = json!({ "connections": [
            { "metadata": { "host": "", "sniffHost": "discord.com", "destinationIP": "1.1.1.1", "process": "Discord.exe", "network": "tcp" },
              "chains": ["nl-1", "klick-vpn"], "rule": "RuleSet", "upload": 10, "download": 90 },
            { "metadata": { "host": "ya.ru", "network": "tcp" }, "chains": ["DIRECT"], "rule": "Match", "upload": 1, "download": 1 }
        ]});
        let c = connections(&v);
        assert_eq!(c[0].host, "discord.com");
        assert_eq!(c[0].route, "vpn");
        assert_eq!(c[0].process.as_deref(), Some("Discord.exe"));
        assert_eq!(c[1].route, "direct");
    }
}
