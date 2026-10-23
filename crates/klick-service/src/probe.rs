//! Локальная проверка серверов по команде окна.

use crate::core::{random_secret, CoreApi, CoreProcess};
use crate::ipcheck;
use crate::paths::{Paths, Profile};
use crate::storage;
use klick_core::compile::{self, CoreLayout, PROBE_URL, PROVIDER};
use klick_core::probe::{verdict, Dl, ProbeVerdict, Tcp};
use klick_core::sub::{self, ServerAddr};
use klick_core::Os;
use klick_proto::{ErrorInfo, ProbeNetwork, ServerProbe};
use reqwest::header::ACCEPT;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::{timeout_at, Instant};

const SLOT_COUNT: usize = 4;
const CORE_START_LIMIT: Duration = Duration::from_secs(10);
/// Серверы проверяются не дольше этого; что не успели — остаётся без итога.
const CHECKS_LIMIT: Duration = Duration::from_secs(90);
/// Вся проверка: запуск ядра, серверы, сеть пользователя и отправка панели.
const PROBE_LIMIT: Duration = Duration::from_secs(150);
const DELAY_LIMIT_MS: u32 = 8_000;
const TCP_TRIES: u8 = 3;
const TCP_TRY_LIMIT: Duration = Duration::from_secs(3);
const TCP_PAUSE: Duration = Duration::from_millis(200);
const DOWNLOAD_URL: &str = "https://speed.cloudflare.com/__down?bytes=131072";
const DOWNLOAD_BYTES: u64 = 131_072;
const DOWNLOAD_IDLE: Duration = Duration::from_secs(6);
const DOWNLOAD_LIMIT: Duration = Duration::from_secs(20);
const DOH_JSON: &str = "https://cloudflare-dns.com/dns-query";

pub(super) struct ShareTarget {
    pub url: String,
    pub version: u32,
    pub vpn_port: Option<u16>,
}

pub(super) struct ProbeOutcome {
    pub list: Vec<(String, String)>,
    pub servers: HashMap<String, ServerProbe>,
    pub network: ProbeNetwork,
    pub shared: Option<String>,
    pub attempted_share: bool,
    pub share: Option<(ShareTarget, PanelReport)>,
}

#[derive(Serialize)]
pub(super) struct PanelReport {
    v: u32,
    vpn: bool,
    direct_checked: bool,
    network: PanelNetwork,
    servers: Vec<PanelServer>,
}

#[derive(Serialize)]
struct PanelNetwork {
    #[serde(skip_serializing_if = "Option::is_none")]
    asn: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    net_type: Option<String>,
}

#[derive(Serialize)]
struct PanelServer {
    #[serde(rename = "type")]
    kind: String,
    host: String,
    port: u16,
    verdict: ProbeVerdict,
    flaky: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tcp_ok: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tcp_tries: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tcp_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delay_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dl_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dl_stalled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dns: Option<String>,
}

struct Download {
    verdict: Dl,
    bytes: u64,
}

struct TcpCheck {
    state: Tcp,
    ok: u8,
    median_ms: Option<u32>,
    flaky: bool,
}

pub(super) async fn run(
    paths: Paths,
    profile: Profile,
    conn_id: String,
    vpn: bool,
    tun: bool,
    direct_port: Option<u16>,
    share: Option<ShareTarget>,
    mut net_change: watch::Receiver<u64>,
    expected_net: u64,
) -> Result<ProbeOutcome, ErrorInfo> {
    let started = Instant::now();
    if *net_change.borrow() != expected_net {
        return Err(ErrorInfo::new("probe.network_changed"));
    }
    let provider_path = paths.provider_file(&conn_id);
    if !provider_path.is_file() {
        return Err(ErrorInfo::new("sub.no_servers"));
    }
    let provider_body = std::fs::read_to_string(&provider_path).map_err(|_| ErrorInfo::new("sub.no_servers"))?;
    let addresses = sub::server_addrs(&provider_body);
    let slot_ports = allocate_ports(SLOT_COUNT).await.map_err(|_| ErrorInfo::new("probe.start_failed"))?;
    let layout = CoreLayout {
        os: Os::CURRENT,
        controller: profile.probe_pipe,
        secret: random_secret(),
        mixed_port: 0,
        check_vpn_port: 0,
        check_direct_port: 0,
        tun_device: "klick".into(),
        log_level: "warning".into(),
        core_exe: String::new(),
    };
    let config = compile::compile_probe(&layout, &Paths::provider_rel(&conn_id), &slot_ports);
    let config_path = paths.core_home.join("probe.yaml");
    storage::write_atomic(&config_path, &serde_json::to_vec_pretty(&config).map_err(|_| ErrorInfo::new("probe.start_failed"))?)
        .map_err(|_| ErrorInfo::new("probe.start_failed"))?;
    let api = CoreApi::new(&layout.controller, &layout.secret);
    let start = tokio::select! {
        result = tokio::time::timeout(CORE_START_LIMIT, CoreProcess::start(&paths.core_exe, &paths.core_home, &config_path, api.clone(), false, |_| {})) => {
            result.map_err(|_| ErrorInfo::new("probe.start_failed"))?.map_err(|_| ErrorInfo::new("probe.start_failed"))?
        }
        changed = net_change.changed() => {
            let _ = changed;
            return Err(ErrorInfo::new("probe.network_changed"));
        }
    };

    let remaining = PROBE_LIMIT.saturating_sub(started.elapsed());
    let result = tokio::time::timeout(remaining, async {
        let checks = run_checks(&api, &addresses, &slot_ports, tun, vpn, direct_port, share, Instant::now() + CHECKS_LIMIT);
        let mut outcome = tokio::select! {
            result = checks => result?,
            changed = net_change.changed() => {
                let _ = changed;
                return Err(ErrorInfo::new("probe.network_changed"));
            }
        };
        // VPN (TUN) включили или выключили на ходу: проверки TCP и DNS мерились не в той сети.
        if *net_change.borrow() != expected_net || crate::sys::tun_up() != tun {
            return Err(ErrorInfo::new("probe.network_changed"));
        }
        if let Some((target, report)) = outcome.share.take() {
            if *net_change.borrow() != expected_net {
                return Err(ErrorInfo::new("probe.network_changed"));
            }
            outcome.attempted_share = true;
            outcome.shared = Some(tokio::select! {
                result = send_report(target, report) => result.to_string(),
                changed = net_change.changed() => {
                    let _ = changed;
                    return Err(ErrorInfo::new("probe.network_changed"));
                }
            });
        }
        Ok(outcome)
    })
    .await
    .map_err(|_| ErrorInfo::new("probe.timeout"))
    .and_then(|r| r);
    start.stop().await;
    result
}

async fn allocate_ports(count: usize) -> std::io::Result<Vec<u16>> {
    let mut ports = Vec::with_capacity(count);
    for _ in 0..count {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        ports.push(listener.local_addr()?.port());
    }
    Ok(ports)
}

async fn run_checks(
    api: &CoreApi,
    addresses: &[ServerAddr],
    slot_ports: &[u16],
    tun: bool,
    vpn: bool,
    direct_port: Option<u16>,
    share: Option<ShareTarget>,
    deadline: Instant,
) -> Result<ProbeOutcome, ErrorInfo> {
    let list = api.wait_provider(PROVIDER, 30, Duration::from_millis(100)).await.map_err(|_| ErrorInfo::new("sub.no_servers"))?;
    if list.is_empty() {
        return Err(ErrorInfo::new("sub.no_servers"));
    }

    let mut addresses_by_name: HashMap<String, Vec<ServerAddr>> = HashMap::new();
    for address in addresses {
        addresses_by_name.entry(address.name.clone()).or_default().push(address.clone());
    }
    let addresses_by_name = std::sync::Arc::new(std::sync::Mutex::new(addresses_by_name));
    let list = std::sync::Arc::new(list);
    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let done = std::sync::Arc::new(std::sync::Mutex::new(HashMap::with_capacity(list.len())));
    let mut jobs = JoinSet::new();
    for slot in 0..slot_ports.len().min(list.len()) {
        let (api, list, by_name, next, done, slot_port) = (api.clone(), list.clone(), addresses_by_name.clone(), next.clone(), done.clone(), slot_ports[slot]);
        jobs.spawn(async move {
            loop {
                let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((name, kind)) = list.get(index) else { break };
                let address = by_name.lock().unwrap().get_mut(name).and_then(|items| {
                    let address = items.first().cloned();
                    if !items.is_empty() {
                        items.remove(0);
                    }
                    address
                });
                let result = probe_server(&api, slot, slot_port, name, kind, address.as_ref(), tun, direct_port).await;
                done.lock().unwrap().insert(name.clone(), result);
            }
        });
    }
    // Мёртвый сервер отнимает до полуминуты, а при сильных блокировках мёртвых больше всего:
    // по истечении времени отдаём проверенные, а не теряем всё.
    let _ = timeout_at(deadline, async { while jobs.join_next().await.is_some() {} }).await;
    jobs.abort_all();
    let servers = std::mem::take(&mut *done.lock().unwrap());

    let mut network = ProbeNetwork { vpn, direct_checked: !tun, ..ProbeNetwork::default() };
    let panel = if let Some(target) = share {
        let panel = ipcheck::column(direct_port).await;
        network.asn = panel.asn;
        network.operator = panel.provider;
        network.country = panel.country_code;
        network.region = panel.region;
        network.net_type = panel.net_type;
        Some((target, panel_report(&network, addresses, &servers)))
    } else {
        None
    };
    Ok(ProbeOutcome { list: (*list).clone(), servers, network, shared: None, attempted_share: false, share: panel })
}

async fn probe_server(
    api: &CoreApi,
    slot: usize,
    slot_port: u16,
    name: &str,
    kind: &str,
    address: Option<&ServerAddr>,
    tun: bool,
    direct_port: Option<u16>,
) -> ServerProbe {
    probe_server_at_urls(api, slot, slot_port, name, kind, address, tun, direct_port, PROBE_URL, DOWNLOAD_URL).await
}

async fn probe_server_at_urls(
    api: &CoreApi,
    slot: usize,
    slot_port: u16,
    name: &str,
    kind: &str,
    address: Option<&ServerAddr>,
    tun: bool,
    direct_port: Option<u16>,
    delay_url: &str,
    download_url: &str,
) -> ServerProbe {
    let group = format!("klick-probe-{slot}");
    let selected = api.select(&group, name).await.is_ok();
    let (dns_verdict, connect_ips) = match address {
        Some(addr) if !tun && addr.host.parse::<IpAddr>().is_err() => {
            let (doh, system) = tokio::join!(doh_answers(&addr.host, direct_port), system_answers(&addr.host));
            let verdict = compare_dns(doh.as_deref(), system.as_deref());
            let ips = doh.filter(|ips| !ips.is_empty()).or(system);
            (verdict.map(str::to_string), ips)
        }
        Some(addr) if !tun => (None, addr.host.parse::<IpAddr>().ok().map(|ip| vec![ip])),
        _ => (None, None),
    };
    let tcp = if !tun && is_tcp_kind(kind) {
        address.and_then(|addr| {
            let ips = connect_ips.clone().or_else(|| addr.host.parse::<IpAddr>().ok().map(|ip| vec![ip]));
            ips.map(|ips| (addr.port, ips))
        })
    } else {
        None
    };
    let tcp_future = async { match tcp { Some((port, ips)) => Some(tcp_check(&ips, port).await), None => None } };
    let delay_future = async {
        if selected { api.proxy_delay(&group, delay_url, DELAY_LIMIT_MS).await.ok().flatten() } else { None }
    };
    let download_future = async {
        if !selected {
            return Vec::new();
        }
        let mut out = vec![download(Some(slot_port), download_url, DOWNLOAD_BYTES, DOWNLOAD_IDLE, DOWNLOAD_LIMIT).await];
        if out[0].verdict != Dl::Complete {
            out.push(download(Some(slot_port), download_url, DOWNLOAD_BYTES, DOWNLOAD_IDLE, DOWNLOAD_LIMIT).await);
        }
        out
    };
    let (tcp, delay, downloads) = tokio::join!(tcp_future, delay_future, download_future);
    let dls: Vec<Dl> = downloads.iter().map(|d| d.verdict).collect();
    let tcp_state = tcp.as_ref().map(|t| t.state);
    let (verdict, mut flaky) = verdict(tcp_state, delay.is_some(), &dls);
    if tcp.as_ref().is_some_and(|t| t.flaky) {
        flaky = true;
    }
    let bytes = downloads.iter().map(|d| d.bytes).max();
    let stalled = (!downloads.is_empty()).then(|| downloads.iter().any(|d| matches!(d.verdict, Dl::Stalled(_))));
    ServerProbe {
        verdict,
        flaky,
        tcp_ok: tcp.as_ref().map(|t| t.ok),
        tcp_tries: tcp.as_ref().map(|_| TCP_TRIES),
        tcp_ms: tcp.and_then(|t| t.median_ms),
        delay_ms: delay,
        dl_bytes: bytes,
        dl_stalled: stalled,
        dns: dns_verdict,
        host: address.map(|a| a.host.clone()).unwrap_or_default(),
        port: address.map(|a| a.port).unwrap_or_default(),
    }
}

fn is_tcp_kind(kind: &str) -> bool {
    matches!(kind, "vless" | "vmess" | "trojan" | "ss" | "anytls" | "http" | "socks" | "socks5")
}

async fn download(port: Option<u16>, url: &str, expected: u64, idle: Duration, total: Duration) -> Download {
    let failed = |bytes| Download { verdict: Dl::Failed, bytes };
    let mut builder = reqwest::Client::builder().connect_timeout(TCP_TRY_LIMIT).timeout(total + Duration::from_secs(1));
    builder = match port {
        Some(port) => match reqwest::Proxy::all(format!("http://127.0.0.1:{port}")) {
            Ok(proxy) => builder.proxy(proxy),
            Err(_) => return failed(0),
        },
        None => builder.no_proxy(),
    };
    let Ok(client) = builder.build() else { return failed(0) };
    let deadline = Instant::now() + total;
    let response = match timeout_at(deadline, client.get(url).send()).await {
        Ok(Ok(response)) if response.status().is_success() => response,
        _ => return failed(0),
    };
    let mut response = response;
    let mut bytes = 0u64;
    loop {
        if bytes >= expected {
            return Download { verdict: Dl::Complete, bytes };
        }
        let idle_deadline = Instant::now() + idle;
        let until = idle_deadline.min(deadline);
        match timeout_at(until, response.chunk()).await {
            Ok(Ok(Some(chunk))) => {
                bytes = bytes.saturating_add(chunk.len() as u64).min(expected);
            }
            Ok(Ok(None)) if bytes >= expected => return Download { verdict: Dl::Complete, bytes },
            Ok(Ok(None)) | Ok(Err(_)) => return failed(bytes),
            Err(_) if idle_deadline <= deadline && bytes > 0 => return Download { verdict: Dl::Stalled(bytes), bytes },
            Err(_) => return failed(bytes),
        }
    }
}

async fn tcp_check(ips: &[IpAddr], port: u16) -> TcpCheck {
    let mut oks = Vec::new();
    let mut timed_out = 0u8;
    let mut refused = 0u8;
    for attempt in 0..TCP_TRIES {
        let Some(ip) = ips.get(usize::from(attempt) % ips.len().max(1)) else { break };
        let start = Instant::now();
        match tokio::time::timeout(TCP_TRY_LIMIT, TcpStream::connect(SocketAddr::new(*ip, port))).await {
            Ok(Ok(stream)) => {
                oks.push(start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32);
                drop(stream);
            }
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::TimedOut => timed_out += 1,
            Ok(Err(_)) => refused += 1,
            Err(_) => timed_out += 1,
        }
        if attempt + 1 < TCP_TRIES {
            tokio::time::sleep(TCP_PAUSE).await;
        }
    }
    let state = if !oks.is_empty() {
        Tcp::Open
    } else if timed_out == TCP_TRIES {
        Tcp::Timeout
    } else {
        Tcp::Refused
    };
    oks.sort_unstable();
    let ok = oks.len() as u8;
    let flaky = (ok > 0 && ok < TCP_TRIES) || (timed_out > 0 && refused > 0);
    TcpCheck { state, ok, median_ms: oks.get(oks.len() / 2).copied(), flaky }
}

async fn doh_answers(host: &str, port: Option<u16>) -> Option<Vec<IpAddr>> {
    let client = ipcheck::client(port)?;
    let mut out = Vec::new();
    for record_type in ["A", "AAAA"] {
        let response = client
            .get(DOH_JSON)
            .query(&[("name", host), ("type", record_type)])
            .header(ACCEPT, "application/dns-json")
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let value = response.json::<Value>().await.ok()?;
        if value["Status"].as_u64()? == 3 {
            return Some(Vec::new());
        }
        if value["Status"].as_u64()? != 0 {
            return None;
        }
        if let Some(answers) = value["Answer"].as_array() {
            for answer in answers {
                if answer["type"].as_u64().is_some_and(|kind| (kind == 1 && record_type == "A") || (kind == 28 && record_type == "AAAA")) {
                    if let Some(ip) = answer["data"].as_str().and_then(|s| s.parse().ok()) {
                        out.push(ip);
                    }
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    Some(out)
}

async fn system_answers(host: &str) -> Option<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = tokio::net::lookup_host((host, 0)).await.ok()?.map(|addr| addr.ip()).collect();
    ips.sort_unstable();
    ips.dedup();
    Some(ips)
}

fn compare_dns(doh: Option<&[IpAddr]>, system: Option<&[IpAddr]>) -> Option<&'static str> {
    let doh = doh?;
    if doh.is_empty() {
        return Some("nxdomain");
    }
    let system = system?;
    if system.is_empty() {
        return Some("nxdomain");
    }
    if doh.iter().chain(system).any(|ip| is_bogus(*ip)) {
        return Some("bogus");
    }
    let doh: HashSet<IpAddr> = doh.iter().copied().collect();
    let system: HashSet<IpAddr> = system.iter().copied().collect();
    Some(if doh == system { "same" } else { "different" })
}

fn is_bogus(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            octets[0] == 0 || ip.is_loopback() || ip.is_private() || (octets[0] == 198 && matches!(octets[1], 18 | 19))
        }
        IpAddr::V6(ip) => ip.is_unspecified() || ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00,
    }
}

fn panel_report(network: &ProbeNetwork, addresses: &[ServerAddr], servers: &HashMap<String, ServerProbe>) -> PanelReport {
    let server_reports = addresses
        .iter()
        .filter_map(|addr| {
            let result = servers.get(&addr.name)?;
            Some(PanelServer {
                kind: addr.kind.clone(),
                host: addr.host.clone(),
                port: addr.port,
                verdict: result.verdict,
                flaky: result.flaky,
                tcp_ok: result.tcp_ok,
                tcp_tries: result.tcp_tries,
                tcp_ms: result.tcp_ms,
                delay_ms: result.delay_ms,
                dl_bytes: result.dl_bytes,
                dl_stalled: result.dl_stalled,
                dns: result.dns.clone(),
            })
        })
        .collect();
    PanelReport {
        v: 1,
        vpn: network.vpn,
        direct_checked: network.direct_checked,
        network: PanelNetwork {
            asn: network.asn,
            operator: network.operator.clone(),
            country: network.country.clone(),
            region: network.region.clone(),
            net_type: network.net_type.clone(),
        },
        servers: server_reports,
    }
}

pub(super) async fn send_report(target: ShareTarget, report: PanelReport) -> &'static str {
    if target.version < 1 {
        return "off";
    }
    // При включённом VPN — только через него: прямой повтор показал бы панели настоящий адрес.
    match post_report(&target.url, target.vpn_port, &report).await {
        PostResult::Accepted => "sent",
        PostResult::PanelOff => "off",
        PostResult::Later => "later",
        PostResult::Failed => "failed",
    }
}

enum PostResult {
    Accepted,
    PanelOff,
    Later,
    Failed,
}

async fn post_report(url: &str, via_port: Option<u16>, report: &PanelReport) -> PostResult {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none());
    builder = match via_port {
        Some(port) => match reqwest::Proxy::all(format!("http://127.0.0.1:{port}")) {
            Ok(proxy) => builder.proxy(proxy),
            Err(_) => return PostResult::Failed,
        },
        None => builder.no_proxy(),
    };
    let Ok(client) = builder.build() else { return PostResult::Failed };
    let result = client
        .post(url)
        .header(reqwest::header::USER_AGENT, format!("klick/{}", env!("CARGO_PKG_VERSION")))
        .json(report)
        .send()
        .await;
    let Ok(response) = result else { return PostResult::Failed };
    match response.status().as_u16() {
        200 => PostResult::Accepted,
        403 => match response.text().await {
            Ok(body) if body.trim() == "probe_off" || body.contains("\"probe_off\"") => PostResult::PanelOff,
            _ => PostResult::Failed,
        },
        429 => PostResult::Later,
        _ => PostResult::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klick_core::sub::ServerAddr;
    use std::path::PathBuf;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    async fn http_body_server(bytes: Vec<u8>, expected: usize, close: bool, hold: bool) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut part = [0_u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let count = stream.read(&mut part).await.unwrap();
                if count == 0 {
                    return;
                }
                request.extend_from_slice(&part[..count]);
            }
            let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {expected}\r\nConnection: close\r\n\r\n");
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&bytes).await.unwrap();
            stream.flush().await.unwrap();
            if hold {
                tokio::time::sleep(Duration::from_secs(1)).await;
            } else if !close {
                stream.shutdown().await.unwrap();
            }
        });
        (format!("http://{address}/data"), task)
    }

    async fn fixture_download(bytes: Vec<u8>, expected: usize, close: bool, hold: bool) -> Download {
        let (url, task) = http_body_server(bytes, expected, close, hold).await;
        let result = download(None, &url, expected as u64, Duration::from_millis(80), Duration::from_secs(1)).await;
        task.abort();
        result
    }

    #[tokio::test]
    async fn downloads_distinguish_stall_complete_and_close() {
        let expected = 32 * 1024;
        let partial = fixture_download(vec![7; 4096], expected, false, true).await;
        assert_eq!(partial.verdict, Dl::Stalled(4096));

        let complete = fixture_download(vec![1; expected], expected, false, false).await;
        assert_eq!(complete.verdict, Dl::Complete);
        assert_eq!(complete.bytes, expected as u64);

        let closed = fixture_download(vec![2; 4096], expected, true, false).await;
        assert_eq!(closed.verdict, Dl::Failed);
        assert_eq!(closed.bytes, 4096);
    }

    #[tokio::test]
    async fn tcp_probe_counts_open_and_refused_attempts() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            for _ in 0..TCP_TRIES {
                let (stream, _) = listener.accept().await.unwrap();
                drop(stream);
            }
        });
        let open = tcp_check(&["127.0.0.1".parse().unwrap()], port).await;
        assert_eq!((open.state, open.ok, open.flaky), (Tcp::Open, TCP_TRIES, false));
        server.await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let refused = tcp_check(&["127.0.0.1".parse().unwrap()], port).await;
        assert_eq!((refused.state, refused.ok), (Tcp::Refused, 0));
    }

    #[test]
    fn dns_comparison_reports_same_different_bogus_and_nxdomain() {
        let public: IpAddr = "203.0.113.9".parse().unwrap();
        assert_eq!(compare_dns(Some(&[public]), Some(&[public])), Some("same"));
        assert_eq!(compare_dns(Some(&[public]), Some(&["203.0.113.10".parse().unwrap()])), Some("different"));
        assert_eq!(compare_dns(Some(&[]), Some(&[public])), Some("nxdomain"));
        assert_eq!(compare_dns(Some(&["192.168.1.1".parse().unwrap()]), Some(&[public])), Some("bogus"));
        assert_eq!(compare_dns(Some(&["::".parse().unwrap()]), Some(&[public])), Some("bogus"));
    }

    fn test_panel_report() -> PanelReport {
        PanelReport {
            v: 1,
            vpn: false,
            direct_checked: true,
            network: PanelNetwork {
                asn: Some(8359),
                operator: Some("MTS PJSC".into()),
                country: Some("RU".into()),
                region: Some("Moscow".into()),
                net_type: Some("mobile".into()),
            },
            servers: Vec::new(),
        }
    }

    #[test]
    fn panel_report_golden_json_contains_no_names_ips_or_hwid() {
        let address = ServerAddr { name: "личное имя сервера".into(), kind: "vless".into(), host: "node.example".into(), port: 443 };
        let server = ServerProbe {
            verdict: ProbeVerdict::Freeze,
            flaky: false,
            tcp_ok: Some(3),
            tcp_tries: Some(3),
            tcp_ms: Some(52),
            delay_ms: Some(140),
            dl_bytes: Some(10_985),
            dl_stalled: Some(true),
            dns: Some("same".into()),
            host: address.host.clone(),
            port: address.port,
        };
        let report = panel_report(&ProbeNetwork {
            vpn: false,
            direct_checked: true,
            asn: Some(8359),
            operator: Some("MTS PJSC".into()),
            country: Some("RU".into()),
            region: Some("Moscow".into()),
            net_type: Some("mobile".into()),
        }, &[address], &HashMap::from([("личное имя сервера".into(), server)]));
        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(json, r#"{"v":1,"vpn":false,"direct_checked":true,"network":{"asn":8359,"operator":"MTS PJSC","country":"RU","region":"Moscow","type":"mobile"},"servers":[{"type":"vless","host":"node.example","port":443,"verdict":"freeze","flaky":false,"tcp_ok":3,"tcp_tries":3,"tcp_ms":52,"delay_ms":140,"dl_bytes":10985,"dl_stalled":true,"dns":"same"}]}"#);
        for field in ["личное имя сервера", "\"name\"", "\"ip\"", "\"hwid\""] {
            assert!(!json.contains(field), "report leaked {field}: {json}");
        }
    }

    async fn capture_one_request(listener: TcpListener, target: String) -> String {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut part = [0_u8; 2048];
        let mut header_end = None;
        let mut content_length = 0;
        loop {
            let count = stream.read(&mut part).await.unwrap();
            assert!(count > 0);
            request.extend_from_slice(&part[..count]);
            if header_end.is_none() {
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(end + 4);
                    let headers = String::from_utf8_lossy(&request[..end]);
                    content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                }
            }
            if header_end.is_some_and(|end| request.len() >= end + content_length) {
                break;
            }
        }
        let body = b"{\"accepted\":1}";
        let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.write_all(body).await.unwrap();
        let request = String::from_utf8_lossy(&request).into_owned();
        if target.is_empty() { request } else { request.replace(&target, "<panel>") }
    }

    #[tokio::test]
    async fn report_uses_the_subscription_path_and_vpn_proxy() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(capture_one_request(listener, "".into()));
        let target = ShareTarget { url: format!("http://{address}/prefix/token/probe"), version: 1, vpn_port: None };
        assert_eq!(send_report(target, test_panel_report()).await, "sent");
        let request = server.await.unwrap();
        assert!(request.starts_with("POST /prefix/token/probe HTTP/1.1"), "{request}");
        assert!(request.to_ascii_lowercase().contains("user-agent: klick/"), "{request}");
        assert!(request.to_ascii_lowercase().contains("content-type: application/json"), "{request}");
        assert!(!request.to_ascii_lowercase().contains("x-hwid"), "{request}");

        let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_port = proxy.local_addr().unwrap().port();
        let panel_url = "http://panel.example/prefix/token/probe";
        let server = tokio::spawn(capture_one_request(proxy, panel_url.into()));
        let target = ShareTarget { url: panel_url.into(), version: 1, vpn_port: Some(proxy_port) };
        assert_eq!(send_report(target, test_panel_report()).await, "sent");
        let request = server.await.unwrap();
        assert!(request.starts_with("POST <panel> HTTP/1.1"), "{request}");
    }

    #[tokio::test]
    async fn report_does_not_follow_redirects_to_another_host() {
        let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let destination_port = destination.local_addr().unwrap().port();
        let destination_task = tokio::spawn(async move { destination.accept().await });
        let panel = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let panel_address = panel.local_addr().unwrap();
        let panel_task = tokio::spawn(async move {
            let (stream, _) = panel.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).await.unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            let mut stream = reader.into_inner();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{destination_port}/collect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let target = ShareTarget { url: format!("http://{panel_address}/probe"), version: 1, vpn_port: None };
        assert_eq!(send_report(target, test_panel_report()).await, "failed");
        panel_task.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(200), destination_task).await.is_err(), "the panel redirect target must not receive the report");
    }

    async fn run_connect_relay(mut client: TcpStream, freeze: bool) -> std::io::Result<()> {
        let mut reader = BufReader::new(client);
        let mut request = String::new();
        reader.read_line(&mut request).await?;
        if !request.starts_with("CONNECT ") {
            return Ok(());
        }
        let target = request.split_whitespace().nth(1).unwrap_or_default().to_string();
        loop {
            request.clear();
            if reader.read_line(&mut request).await? == 0 || request == "\r\n" {
                break;
            }
        }
        let Some((host, port)) = target.rsplit_once(':') else { return Ok(()) };
        let Ok(upstream) = TcpStream::connect((host.trim_matches(['[', ']']), port.parse::<u16>().unwrap_or(443))).await else {
            return Ok(())
        };
        client = reader.into_inner();
        client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;
        let (mut client_read, mut client_write) = client.into_split();
        let (mut upstream_read, mut upstream_write) = upstream.into_split();
        let upstream_to_client = async {
            if freeze {
                let mut limited = upstream_read.take(16 * 1024);
                tokio::io::copy(&mut limited, &mut client_write).await?;
                std::future::pending::<()>().await;
            } else {
                tokio::io::copy(&mut upstream_read, &mut client_write).await?;
            }
            Ok::<(), std::io::Error>(())
        };
        let client_to_upstream = tokio::io::copy(&mut client_read, &mut upstream_write);
        tokio::select! {
            result = upstream_to_client => result?,
            result = client_to_upstream => { result?; },
        }
        Ok(())
    }

    async fn spawn_test_origin() -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { break };
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stream);
                    let mut request_line = String::new();
                    let _ = reader.read_line(&mut request_line).await;
                    let mut request = String::new();
                    loop {
                        request.clear();
                        if reader.read_line(&mut request).await.unwrap_or(0) == 0 || request == "\r\n" {
                            break;
                        }
                    }
                    let mut stream = reader.into_inner();
                    if request_line.starts_with("GET /generate_204 ") {
                        let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n").await;
                    } else {
                        let body = vec![8; DOWNLOAD_BYTES as usize];
                        let _ = stream
                            .write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes())
                            .await;
                        let _ = stream.write_all(&body).await;
                    }
                });
            }
        });
        (port, task)
    }

    #[tokio::test]
    async fn connect_relay_stops_after_16_kib_and_keeps_the_tunnel_open() {
        let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_addr = origin.local_addr().unwrap();
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.unwrap();
            stream.write_all(&vec![5; 64 * 1024]).await.unwrap();
            tokio::time::sleep(Duration::from_secs(2)).await;
        });
        let relay = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let relay_addr = relay.local_addr().unwrap();
        let relay_task = tokio::spawn(async move {
            let (stream, _) = relay.accept().await.unwrap();
            let _ = run_connect_relay(stream, true).await;
        });
        let mut client = TcpStream::connect(relay_addr).await.unwrap();
        client
            .write_all(format!("CONNECT {origin_addr} HTTP/1.1\r\nHost: {origin_addr}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut reader = BufReader::new(client);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        assert!(line.starts_with("HTTP/1.1 200"), "{line}");
        loop {
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            if line == "\r\n" {
                break;
            }
        }
        let mut body = vec![0; 16 * 1024];
        reader.read_exact(&mut body).await.unwrap();
        assert!(body.iter().all(|byte| *byte == 5));
        let mut extra = [0_u8; 1];
        assert!(tokio::time::timeout(Duration::from_millis(150), reader.read(&mut extra)).await.is_err());
        drop(reader);
        relay_task.await.unwrap();
        origin_task.abort();
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("klick-probe-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn spawn_test_relay(freeze: bool) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { break };
                tokio::spawn(run_connect_relay(stream, freeze));
            }
        });
        (port, task)
    }

    #[tokio::test]
    async fn mihomo_detects_a_freezing_http_proxy_and_keeps_generate_204_working() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let exe = root.join("resources/core").join(if cfg!(windows) { "mihomo.exe" } else { "mihomo" });
        if !exe.is_file() {
            return;
        }
        let (freeze_port, freeze_task) = spawn_test_relay(true).await;
        let (ok_port, ok_task) = spawn_test_relay(false).await;
        let (origin_port, origin_task) = spawn_test_origin().await;
        let delay_url = format!("http://127.0.0.1:{origin_port}/generate_204");
        let download_url = format!("http://127.0.0.1:{origin_port}/__down");
        let temp = TestDir::new();
        let paths = Paths::new(temp.0.join("data"), root.join("resources"));
        paths.ensure_dirs().unwrap();
        let provider = serde_json::json!({"proxies":[
            {"name":"freeze","type":"http","server":"127.0.0.1","port":freeze_port},
            {"name":"ok","type":"http","server":"127.0.0.1","port":ok_port}
        ]});
        std::fs::write(paths.provider_file("test"), provider.to_string()).unwrap();
        let slot_ports = allocate_ports(2).await.unwrap();
        let controller_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let controller_port = controller_listener.local_addr().unwrap().port();
        drop(controller_listener);
        let controller = format!("tcp://127.0.0.1:{controller_port}");
        let layout = CoreLayout {
            os: Os::CURRENT,
            controller: controller.clone(),
            secret: random_secret(),
            mixed_port: 0,
            check_vpn_port: 0,
            check_direct_port: 0,
            tun_device: "klick".into(),
            log_level: "warning".into(),
            core_exe: exe.to_string_lossy().into_owned(),
        };
        let config_path = paths.core_home.join("probe.yaml");
        let mut config = compile::compile_probe(&layout, &Paths::provider_rel("test"), &slot_ports);
        let config = config.as_object_mut().unwrap();
        config.remove("external-controller-pipe");
        config.remove("external-controller-unix");
        config.insert("external-controller".into(), serde_json::json!(format!("127.0.0.1:{controller_port}")));
        storage::write_atomic(&config_path, &serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        let api = CoreApi::new(&layout.controller, &layout.secret);
        let mut command = tokio::process::Command::new(&exe);
        command
            .arg("-d")
            .arg(&paths.core_home)
            .arg("-f")
            .arg(&config_path)
            .current_dir(&paths.core_home)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        for var in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] {
            command.env_remove(var);
        }
        #[cfg(windows)]
        {
            command.creation_flags(0x0800_0000);
        }
        let mut core = command.spawn().unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if api.version().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        let servers = api.wait_provider(PROVIDER, 30, Duration::from_millis(100)).await.unwrap();
        assert_eq!(servers.len(), 2);
        assert!(api.select("klick-probe-0", "freeze").await.is_ok(), "probe slot must select the relay");
        assert_eq!(api.selected("klick-probe-0").await.unwrap().as_deref(), Some("freeze"));
        assert!(api.proxy_delay("klick-probe-0", &delay_url, DELAY_LIMIT_MS).await.unwrap().is_some(), "generate_204 through the freezing server must pass");
        let freeze = probe_server_at_urls(
            &api,
            0,
            slot_ports[0],
            "freeze",
            "http",
            Some(&ServerAddr { name: "freeze".into(), kind: "http".into(), host: "127.0.0.1".into(), port: freeze_port }),
            false,
            None,
            &delay_url,
            &download_url,
        )
        .await;
        assert_eq!(freeze.verdict, ProbeVerdict::Freeze, "{freeze:?}");
        assert!(freeze.delay_ms.is_some(), "generate_204 through the freezing server must pass");
        let ok = probe_server_at_urls(
            &api,
            1,
            slot_ports[1],
            "ok",
            "http",
            Some(&ServerAddr { name: "ok".into(), kind: "http".into(), host: "127.0.0.1".into(), port: ok_port }),
            false,
            None,
            &delay_url,
            &download_url,
        )
        .await;
        assert_eq!(ok.verdict, ProbeVerdict::Ok, "{ok:?}");
        assert_eq!(ok.dl_stalled, Some(false));
        core.kill().await.unwrap();
        let _ = core.wait().await;
        freeze_task.abort();
        ok_task.abort();
        origin_task.abort();
    }

    #[test]
    fn panel_report_without_measurements_omits_optional_fields() {
        let json = serde_json::to_value(test_panel_report()).unwrap();
        assert!(json["servers"].as_array().unwrap().is_empty());
        assert!(json["network"].get("ip").is_none());
        assert!(json["network"].get("city").is_none());
    }
}
