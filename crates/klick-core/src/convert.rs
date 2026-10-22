//! Чужие форматы → серверы mihomo. Ядро kl!ck — mihomo, его proxy-provider читает YAML/JSON со списком
//! `proxies` и списки ссылок. Остальное переводим здесь: конфиги sing-box и Xray/V2Ray (в том числе
//! массивом — так отдаёт «xray-json» Remnawave), WireGuard/AmneziaWG `.conf`, Shadowsocks SIP008 и
//! ключи Outline, а ещё ссылки Hysteria (`hysteria_link`): конвертер самого ядра портит их пароль.
//! Берём только серверы: маршрутизация, DNS и TUN у kl!ck свои.

use serde_json::{json, Map, Value};

/// Откуда серверы — для журнала и сообщений.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    SingBox,
    Xray,
    WireGuard,
    Shadowsocks,
}

/// Распознать формат. `None` — это не один из переводимых форматов.
pub fn detect(body: &str) -> Option<Format> {
    let t = body.trim_start_matches('\u{feff}').trim();
    if t.contains("[Interface]") && t.contains("[Peer]") {
        return Some(Format::WireGuard);
    }
    let v: Value = serde_json::from_str(t).ok()?;
    detect_json(&v)
}

fn detect_json(v: &Value) -> Option<Format> {
    match v {
        Value::Array(items) => items.iter().find_map(detect_json),
        Value::Object(o) => {
            let outs = o.get("outbounds").and_then(Value::as_array);
            if let Some(outs) = outs {
                if outs.iter().any(|x| x.get("protocol").is_some()) {
                    return Some(Format::Xray);
                }
                if outs.iter().any(|x| x.get("type").is_some()) {
                    return Some(Format::SingBox);
                }
            }
            if o.get("endpoints").and_then(Value::as_array).is_some_and(|e| e.iter().any(|x| x.get("type").and_then(Value::as_str) == Some("wireguard"))) {
                return Some(Format::SingBox);
            }
            if o.get("servers").and_then(Value::as_array).is_some_and(|s| s.iter().any(is_ss_server)) || is_ss_server(v) {
                return Some(Format::Shadowsocks);
            }
            None
        }
        _ => None,
    }
}

fn is_ss_server(v: &Value) -> bool {
    v.get("server").is_some() && v.get("server_port").is_some() && v.get("method").is_some() && v.get("password").is_some()
}

/// Серверы mihomo из тела в формате `format`. Пусто — серверов не нашлось.
pub fn proxies(body: &str, format: Format) -> Vec<Value> {
    let t = body.trim_start_matches('\u{feff}').trim();
    let mut out = match format {
        Format::WireGuard => wireguard_conf(t).into_iter().collect(),
        _ => {
            let Ok(v) = serde_json::from_str::<Value>(t) else { return Vec::new() };
            match format {
                Format::SingBox => each_config(&v).flat_map(singbox).collect(),
                Format::Xray => each_config(&v).flat_map(xray).collect(),
                Format::Shadowsocks => sip008(&v),
                Format::WireGuard => unreachable!(),
            }
        }
    };
    unique_names(&mut out);
    out
}

/// Текст для файла серверов ядра: `{"proxies": [...]}` (JSON читается как YAML).
pub fn provider_text(proxies: &[Value]) -> String {
    serde_json::to_string_pretty(&json!({ "proxies": proxies })).unwrap_or_default()
}

/// Конфиг целиком или массив конфигов (Remnawave «xray-json»).
fn each_config(v: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    match v {
        Value::Array(a) => Box::new(a.iter()),
        other => Box::new(std::iter::once(other)),
    }
}

/// mihomo требует уникальные имена: «DE», «DE 2», «DE 3».
fn unique_names(list: &mut [Value]) {
    let mut seen: Vec<String> = Vec::new();
    for p in list.iter_mut() {
        let base = p["name"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or("Сервер").trim().to_string();
        let mut name = base.clone();
        let mut n = 2;
        while seen.contains(&name) {
            name = format!("{base} {n}");
            n += 1;
        }
        seen.push(name.clone());
        p["name"] = json!(name);
    }
}

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|x| !x.is_empty())
}

fn port(v: &Value, key: &str) -> Option<u64> {
    match v.get(key)? {
        Value::Number(n) => n.as_u64(),
        Value::String(x) => x.trim().parse().ok(),
        _ => None,
    }
}

fn put(m: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value.filter(|v| !v.is_null()) {
        m.insert(key.into(), v);
    }
}

// ── sing-box ────────────────────────────────────────────────────────────

fn singbox(cfg: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for o in cfg.get("outbounds").and_then(Value::as_array).into_iter().flatten() {
        if let Some(p) = singbox_outbound(o) {
            out.push(p);
        }
    }
    // sing-box 1.11+: WireGuard — в `endpoints`.
    for e in cfg.get("endpoints").and_then(Value::as_array).into_iter().flatten() {
        if s(e, "type") == Some("wireguard") {
            if let Some(p) = singbox_wireguard(e) {
                out.push(p);
            }
        }
    }
    out
}

fn singbox_outbound(o: &Value) -> Option<Value> {
    let kind = s(o, "type")?;
    let name = s(o, "tag").unwrap_or(kind).to_string();
    if kind == "wireguard" {
        return singbox_wireguard(o);
    }
    let server = s(o, "server")?;
    // У Hysteria порт можно не задавать вовсе: хватает `server_ports` (прыжки по портам).
    let ports = matches!(kind, "hysteria" | "hysteria2").then(|| singbox_ports(o)).flatten();
    let port_ = port(o, "server_port").or_else(|| ports.as_ref().map(|(first, _)| u64::from(*first)))?;
    let hop = ports.and_then(|(_, hop)| hop);
    let mut m = Map::new();
    m.insert("name".into(), json!(name));
    m.insert("server".into(), json!(server));
    m.insert("port".into(), json!(port_));
    match kind {
        "vless" => {
            m.insert("type".into(), json!("vless"));
            m.insert("uuid".into(), json!(s(o, "uuid")?));
            put(&mut m, "flow", s(o, "flow").map(|f| json!(f)));
            put(&mut m, "packet-encoding", s(o, "packet_encoding").map(|f| json!(f)));
            m.insert("udp".into(), json!(true));
        }
        "vmess" => {
            m.insert("type".into(), json!("vmess"));
            m.insert("uuid".into(), json!(s(o, "uuid")?));
            m.insert("alterId".into(), json!(port(o, "alter_id").unwrap_or(0)));
            m.insert("cipher".into(), json!(s(o, "security").unwrap_or("auto")));
            m.insert("udp".into(), json!(true));
        }
        "trojan" => {
            m.insert("type".into(), json!("trojan"));
            m.insert("password".into(), json!(s(o, "password")?));
            m.insert("udp".into(), json!(true));
        }
        "shadowsocks" => {
            m.insert("type".into(), json!("ss"));
            m.insert("cipher".into(), json!(s(o, "method")?));
            m.insert("password".into(), json!(s(o, "password")?));
            m.insert("udp".into(), json!(true));
            ss_plugin(&mut m, s(o, "plugin"), s(o, "plugin_opts"));
        }
        "hysteria2" => {
            m.insert("type".into(), json!("hysteria2"));
            m.insert("password".into(), json!(s(o, "password")?));
            put(&mut m, "up", sb_speed(o, "up_mbps", "up"));
            put(&mut m, "down", sb_speed(o, "down_mbps", "down"));
            if let Some(obfs) = o.get("obfs") {
                put(&mut m, "obfs", s(obfs, "type").map(|t| json!(t)));
                put(&mut m, "obfs-password", s(obfs, "password").map(|t| json!(t)));
            }
            hop_options(&mut m, o, hop);
        }
        "hysteria" => {
            m.insert("type".into(), json!("hysteria"));
            // Hysteria 1: скорость обязательна — ядро без неё отвергает весь файл серверов.
            m.insert("up".into(), sb_speed(o, "up_mbps", "up")?);
            m.insert("down".into(), sb_speed(o, "down_mbps", "down")?);
            // `auth` — base64 (так его понимает и mihomo), `auth_str` — обычная строка.
            put(&mut m, "auth", s(o, "auth").map(|v| json!(v)));
            put(&mut m, "auth-str", s(o, "auth_str").map(|v| json!(v)));
            // В Hysteria 1 `obfs` — сам пароль обфускации (xplus).
            put(&mut m, "obfs", s(o, "obfs").map(|v| json!(v)));
            put(&mut m, "recv-window-conn", o.get("recv_window_conn").filter(|v| v.is_u64()).cloned());
            put(&mut m, "recv-window", o.get("recv_window").filter(|v| v.is_u64()).cloned());
            if o.get("disable_mtu_discovery").and_then(Value::as_bool) == Some(true) {
                m.insert("disable-mtu-discovery".into(), json!(true));
            }
            hop_options(&mut m, o, hop);
        }
        "tuic" => {
            m.insert("type".into(), json!("tuic"));
            m.insert("uuid".into(), json!(s(o, "uuid")?));
            m.insert("password".into(), json!(s(o, "password")?));
            put(&mut m, "congestion-controller", s(o, "congestion_control").map(|v| json!(v)));
            put(&mut m, "udp-relay-mode", s(o, "udp_relay_mode").map(|v| json!(v)));
            put(&mut m, "reduce-rtt", o.get("zero_rtt_handshake").cloned());
        }
        "anytls" => {
            m.insert("type".into(), json!("anytls"));
            m.insert("password".into(), json!(s(o, "password")?));
            m.insert("udp".into(), json!(true));
        }
        "socks" => {
            m.insert("type".into(), json!("socks5"));
            put(&mut m, "username", s(o, "username").map(|v| json!(v)));
            put(&mut m, "password", s(o, "password").map(|v| json!(v)));
            m.insert("udp".into(), json!(true));
        }
        "http" => {
            m.insert("type".into(), json!("http"));
            put(&mut m, "username", s(o, "username").map(|v| json!(v)));
            put(&mut m, "password", s(o, "password").map(|v| json!(v)));
        }
        // direct, block, dns, selector, urltest и прочее — не серверы.
        _ => return None,
    }
    if let Some(tls) = o.get("tls").filter(|t| t.get("enabled").and_then(Value::as_bool).unwrap_or(false)) {
        singbox_tls(&mut m, kind, tls);
    }
    if let Some(tr) = o.get("transport") {
        singbox_transport(&mut m, tr);
    }
    Some(Value::Object(m))
}

/// Скорость sing-box → строка mihomo: `up_mbps: 100` или `up: "100 Mbps"`.
fn sb_speed(o: &Value, mbps_key: &str, text_key: &str) -> Option<Value> {
    if let Some(v) = port(o, mbps_key).filter(|v| *v > 0) {
        return Some(json!(format!("{v} Mbps")));
    }
    s(o, text_key).and_then(bandwidth).map(Value::from)
}

/// `server_ports` sing-box (`"20000:30000"`, `"443"`): первый порт и список ядра `443,20000-30000`.
/// Если он задан, `server_port` sing-box игнорирует.
fn singbox_ports(o: &Value) -> Option<(u16, Option<String>)> {
    let raw: Vec<&str> = match o.get("server_ports")? {
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        Value::String(x) => vec![x.as_str()],
        _ => return None,
    };
    parse_ports(&raw.join(","))
}

/// Прыжки по портам: `ports` и `hop-interval` (`hop_interval: "30s"`).
fn hop_options(m: &mut Map<String, Value>, o: &Value, hop: Option<String>) {
    let Some(hop) = hop else { return };
    m.insert("ports".into(), json!(hop));
    let secs = |key: &str| s(o, key).and_then(duration_secs).or_else(|| port(o, key));
    if let Some(min) = secs("hop_interval") {
        let max = secs("hop_interval_max").filter(|m| *m > min);
        let v = match (m.get("type").and_then(Value::as_str), max) {
            // Hysteria 2 в mihomo принимает диапазон «5-10»; у Hysteria 1 интервал — одно число.
            (Some("hysteria2"), Some(max)) => json!(format!("{min}-{max}")),
            (Some("hysteria2"), None) => json!(min.to_string()),
            _ => json!(min),
        };
        m.insert("hop-interval".into(), v);
    }
}

/// «30s», «1m», «30» → секунды.
fn duration_secs(text: &str) -> Option<u64> {
    let t = text.trim();
    let (num, mul) = match t.chars().last()? {
        's' => (&t[..t.len() - 1], 1),
        'm' => (&t[..t.len() - 1], 60),
        'h' => (&t[..t.len() - 1], 3600),
        _ => (t, 1),
    };
    num.trim().parse::<u64>().ok().map(|n| n * mul).filter(|n| *n > 0)
}

fn singbox_tls(m: &mut Map<String, Value>, kind: &str, tls: &Value) {
    if matches!(kind, "vless" | "vmess" | "http" | "socks") {
        m.insert("tls".into(), json!(true));
    }
    // vless и vmess называют SNI `servername`, остальные — `sni`.
    let sni_key = if matches!(kind, "vless" | "vmess") { "servername" } else { "sni" };
    put(m, sni_key, s(tls, "server_name").map(|v| json!(v)));
    if tls.get("insecure").and_then(Value::as_bool) == Some(true) {
        m.insert("skip-cert-verify".into(), json!(true));
    }
    put(m, "alpn", tls.get("alpn").filter(|a| a.as_array().is_some_and(|x| !x.is_empty())).cloned());
    if let Some(fp) = tls.get("utls").filter(|u| u.get("enabled").and_then(Value::as_bool).unwrap_or(false)).and_then(|u| s(u, "fingerprint")) {
        m.insert("client-fingerprint".into(), json!(fp));
    }
    if let Some(r) = tls.get("reality").filter(|r| r.get("enabled").and_then(Value::as_bool).unwrap_or(false)) {
        let mut ro = Map::new();
        put(&mut ro, "public-key", s(r, "public_key").map(|v| json!(v)));
        put(&mut ro, "short-id", s(r, "short_id").map(|v| json!(v)));
        m.insert("reality-opts".into(), Value::Object(ro));
        // Reality без отпечатка mihomo не принимает: chrome — как у sing-box по умолчанию.
        m.entry("client-fingerprint").or_insert(json!("chrome"));
    }
}

fn singbox_transport(m: &mut Map<String, Value>, tr: &Value) {
    match s(tr, "type") {
        Some("ws") => {
            m.insert("network".into(), json!("ws"));
            let mut ws = Map::new();
            put(&mut ws, "path", s(tr, "path").map(|v| json!(v)));
            if let Some(h) = tr.get("headers").and_then(|h| h.get("Host").or_else(|| h.get("host"))) {
                let host = h.as_str().map(str::to_string).or_else(|| h.as_array().and_then(|a| a.first()).and_then(Value::as_str).map(str::to_string));
                put(&mut ws, "headers", host.map(|h| json!({ "Host": h })));
            }
            put(&mut ws, "max-early-data", tr.get("max_early_data").cloned());
            put(&mut ws, "early-data-header-name", s(tr, "early_data_header_name").map(|v| json!(v)));
            m.insert("ws-opts".into(), Value::Object(ws));
        }
        Some("httpupgrade") => {
            m.insert("network".into(), json!("ws"));
            let mut ws = Map::new();
            put(&mut ws, "path", s(tr, "path").map(|v| json!(v)));
            put(&mut ws, "headers", s(tr, "host").map(|h| json!({ "Host": h })));
            ws.insert("v2ray-http-upgrade".into(), json!(true));
            m.insert("ws-opts".into(), Value::Object(ws));
        }
        Some("grpc") => {
            m.insert("network".into(), json!("grpc"));
            m.insert("grpc-opts".into(), json!({ "grpc-service-name": s(tr, "service_name").unwrap_or("") }));
        }
        Some("http") => {
            m.insert("network".into(), json!("h2"));
            let mut h2 = Map::new();
            put(&mut h2, "host", tr.get("host").cloned().map(|h| if h.is_string() { json!([h]) } else { h }));
            put(&mut h2, "path", s(tr, "path").map(|v| json!(v)));
            m.insert("h2-opts".into(), Value::Object(h2));
        }
        _ => {}
    }
}

fn singbox_wireguard(o: &Value) -> Option<Value> {
    let name = s(o, "tag").unwrap_or("WireGuard").to_string();
    // Старый формат: server/server_port/peer_public_key; новый (endpoints): peers[].
    let peer = o.get("peers").and_then(Value::as_array).and_then(|p| p.first());
    let server = peer.and_then(|p| s(p, "address").or_else(|| s(p, "server"))).or_else(|| s(o, "server"))?;
    let port_ = peer.and_then(|p| port(p, "port").or_else(|| port(p, "server_port"))).or_else(|| port(o, "server_port"))?;
    let public_key = peer.and_then(|p| s(p, "public_key")).or_else(|| s(o, "peer_public_key"))?;
    let addresses: Vec<String> = o.get("address").or_else(|| o.get("local_address")).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    let mut m = Map::new();
    m.insert("name".into(), json!(name));
    m.insert("type".into(), json!("wireguard"));
    m.insert("server".into(), json!(server));
    m.insert("port".into(), json!(port_));
    m.insert("private-key".into(), json!(s(o, "private_key")?));
    m.insert("public-key".into(), json!(public_key));
    put(&mut m, "pre-shared-key", peer.and_then(|p| s(p, "pre_shared_key")).or_else(|| s(o, "pre_shared_key")).map(|v| json!(v)));
    wg_addresses(&mut m, &addresses);
    put(&mut m, "mtu", o.get("mtu").cloned());
    put(&mut m, "reserved", peer.and_then(|p| p.get("reserved")).or_else(|| o.get("reserved")).cloned());
    put(&mut m, "allowed-ips", peer.and_then(|p| p.get("allowed_ips")).cloned());
    put(&mut m, "persistent-keepalive", peer.and_then(|p| port(p, "persistent_keepalive_interval")).filter(|n| *n > 0).map(|n| json!(n)));
    // Форки sing-box с AmneziaWG (NekoBox, Throne, Lantern) держат параметры прямо в конечной точке.
    let mut awg = Map::new();
    for (key, awg_key) in [
        ("junk_packet_count", "jc"),
        ("junk_packet_min_size", "jmin"),
        ("junk_packet_max_size", "jmax"),
        ("init_packet_junk_size", "s1"),
        ("response_packet_junk_size", "s2"),
        ("init_packet_magic_header", "h1"),
        ("response_packet_magic_header", "h2"),
        ("underload_packet_magic_header", "h3"),
        ("transport_packet_magic_header", "h4"),
    ] {
        let raw = match o.get(key) {
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::String(x)) => x.clone(),
            _ => continue,
        };
        awg_param(&mut awg, awg_key, &raw);
    }
    if !awg.is_empty() {
        m.insert("amnezia-wg-option".into(), Value::Object(awg));
    }
    m.insert("udp".into(), json!(true));
    Some(Value::Object(m))
}

/// Адреса интерфейса WireGuard: первый IPv4 — `ip`, первый IPv6 — `ipv6`, без маски.
fn wg_addresses(m: &mut Map<String, Value>, addresses: &[String]) {
    for a in addresses {
        let ip = a.split('/').next().unwrap_or(a).trim();
        if ip.contains(':') {
            m.entry("ipv6").or_insert(json!(ip));
        } else if !ip.is_empty() {
            m.entry("ip").or_insert(json!(ip));
        }
    }
}

/// Плагин Shadowsocks: obfs-local (`obfs=http;obfs-host=…`) и v2ray-plugin.
fn ss_plugin(m: &mut Map<String, Value>, plugin: Option<&str>, opts: Option<&str>) {
    let Some(plugin) = plugin else { return };
    let kv: Vec<(String, String)> = opts.unwrap_or("").split(';').filter_map(|p| p.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
    let get = |k: &str| kv.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
    if plugin.contains("obfs") {
        m.insert("plugin".into(), json!("obfs"));
        let mut po = Map::new();
        put(&mut po, "mode", get("obfs").map(|v| json!(v)));
        put(&mut po, "host", get("obfs-host").map(|v| json!(v)));
        m.insert("plugin-opts".into(), Value::Object(po));
    } else if plugin.contains("v2ray") {
        m.insert("plugin".into(), json!("v2ray-plugin"));
        let mut po = Map::new();
        po.insert("mode".into(), json!("websocket"));
        put(&mut po, "host", get("host").map(|v| json!(v)));
        put(&mut po, "path", get("path").map(|v| json!(v)));
        if kv.iter().any(|(k, _)| k == "tls") {
            po.insert("tls".into(), json!(true));
        }
        m.insert("plugin-opts".into(), Value::Object(po));
    }
}

// ── Xray / V2Ray ────────────────────────────────────────────────────────

fn xray(cfg: &Value) -> Vec<Value> {
    let outs: Vec<&Value> = cfg.get("outbounds").and_then(Value::as_array).into_iter().flatten().collect();
    let servers: Vec<&Value> = outs.iter().copied().filter(|o| !matches!(s(o, "protocol"), Some("freedom" | "blackhole" | "dns" | "loopback") | None)).collect();
    // Один сервер в конфиге — имя из `remarks` (так подписывает v2rayN и Remnawave).
    let remarks = s(cfg, "remarks");
    servers.iter().filter_map(|o| xray_outbound(o, if servers.len() == 1 { remarks } else { None })).collect()
}

fn xray_outbound(o: &Value, remarks: Option<&str>) -> Option<Value> {
    let protocol = s(o, "protocol")?;
    let settings = o.get("settings")?;
    let name = remarks.or_else(|| s(o, "tag").filter(|t| *t != "proxy")).unwrap_or(protocol).to_string();
    let mut m = Map::new();
    m.insert("name".into(), json!(name));
    match protocol {
        "vless" | "vmess" => {
            let vnext = settings.get("vnext").and_then(Value::as_array).and_then(|a| a.first())?;
            let user = vnext.get("users").and_then(Value::as_array).and_then(|a| a.first())?;
            m.insert("type".into(), json!(protocol));
            m.insert("server".into(), json!(s(vnext, "address")?));
            m.insert("port".into(), json!(port(vnext, "port")?));
            m.insert("uuid".into(), json!(s(user, "id")?));
            if protocol == "vless" {
                put(&mut m, "flow", s(user, "flow").map(|v| json!(v)));
            } else {
                m.insert("alterId".into(), json!(port(user, "alterId").unwrap_or(0)));
                m.insert("cipher".into(), json!(s(user, "security").unwrap_or("auto")));
            }
            m.insert("udp".into(), json!(true));
        }
        "trojan" | "shadowsocks" | "socks" | "http" => {
            let srv = settings.get("servers").and_then(Value::as_array).and_then(|a| a.first())?;
            m.insert("server".into(), json!(s(srv, "address")?));
            m.insert("port".into(), json!(port(srv, "port")?));
            match protocol {
                "trojan" => {
                    m.insert("type".into(), json!("trojan"));
                    m.insert("password".into(), json!(s(srv, "password")?));
                    m.insert("udp".into(), json!(true));
                }
                "shadowsocks" => {
                    m.insert("type".into(), json!("ss"));
                    m.insert("cipher".into(), json!(s(srv, "method")?));
                    m.insert("password".into(), json!(s(srv, "password")?));
                    m.insert("udp".into(), json!(true));
                }
                _ => {
                    m.insert("type".into(), json!(if protocol == "socks" { "socks5" } else { "http" }));
                    if let Some(u) = srv.get("users").and_then(Value::as_array).and_then(|a| a.first()) {
                        put(&mut m, "username", s(u, "user").map(|v| json!(v)));
                        put(&mut m, "password", s(u, "pass").map(|v| json!(v)));
                    }
                }
            }
        }
        "hysteria" => {
            // Hysteria 2 в Xray: адрес и порт — в settings, пароль — в hysteriaSettings,
            // обфускация, скорость и прыжки по портам — в finalmask.
            let ss = o.get("streamSettings")?;
            m.insert("type".into(), json!("hysteria2"));
            m.insert("server".into(), json!(s(settings, "address")?));
            m.insert("port".into(), json!(port(settings, "port")?));
            m.insert("password".into(), json!(s(ss.get("hysteriaSettings")?, "auth")?));
            let fm = ss.get("finalmask");
            for mask in fm.and_then(|f| f.get("udp")).and_then(Value::as_array).into_iter().flatten() {
                let Some(st) = mask.get("settings") else { continue };
                if s(mask, "type") != Some("salamander") {
                    continue;
                }
                put(&mut m, "obfs-password", s(st, "password").map(|v| json!(v)));
                // С packetSize («512-1200») Xray включает Gecko, а не Salamander.
                let size = s(st, "packetSize").and_then(|r| r.split_once('-')).and_then(|(a, b)| Some((a.trim().parse::<u64>().ok()?, b.trim().parse::<u64>().ok()?)));
                match size {
                    Some((min, max)) => {
                        m.insert("obfs".into(), json!("gecko"));
                        m.insert("obfs-min-packet-size".into(), json!(min));
                        m.insert("obfs-max-packet-size".into(), json!(max));
                    }
                    None => {
                        m.insert("obfs".into(), json!("salamander"));
                    }
                }
            }
            if let Some(q) = fm.and_then(|f| f.get("quicParams")) {
                put(&mut m, "up", xray_speed(q.get("brutalUp")));
                put(&mut m, "down", xray_speed(q.get("brutalDown")));
                if let Some(h) = q.get("udpHop") {
                    if let Some((_, Some(ports))) = s(h, "ports").and_then(parse_ports) {
                        m.insert("ports".into(), json!(ports));
                        let interval = h.get("interval").and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())));
                        put(&mut m, "hop-interval", interval.map(|v| json!(v)));
                    }
                }
            }
            m.insert("udp".into(), json!(true));
            xray_stream(&mut m, protocol, ss);
            // Отпечаток браузера Hysteria не нужен; закрепление сертификата — `fingerprint` ядра.
            m.remove("client-fingerprint");
            let pin = ss.get("tlsSettings").and_then(|t| s(t, "pinnedPeerCertSha256")).and_then(|p| p.split(',').next());
            put(&mut m, "fingerprint", pin.map(|p| json!(p.trim())));
            return Some(Value::Object(m));
        }
        "wireguard" => {
            let peer = settings.get("peers").and_then(Value::as_array).and_then(|a| a.first())?;
            let (host, p) = split_endpoint(s(peer, "endpoint")?)?;
            m.insert("type".into(), json!("wireguard"));
            m.insert("server".into(), json!(host));
            m.insert("port".into(), json!(p));
            m.insert("private-key".into(), json!(s(settings, "secretKey")?));
            m.insert("public-key".into(), json!(s(peer, "publicKey")?));
            put(&mut m, "pre-shared-key", s(peer, "preSharedKey").map(|v| json!(v)));
            put(&mut m, "persistent-keepalive", port(peer, "keepAlive").filter(|n| *n > 0).map(|n| json!(n)));
            let addresses: Vec<String> = settings.get("address").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
            wg_addresses(&mut m, &addresses);
            put(&mut m, "mtu", settings.get("mtu").cloned());
            put(&mut m, "reserved", settings.get("reserved").cloned());
            m.insert("udp".into(), json!(true));
            return Some(Value::Object(m));
        }
        _ => return None,
    }
    if let Some(ss) = o.get("streamSettings") {
        xray_stream(&mut m, protocol, ss);
    }
    Some(Value::Object(m))
}

fn xray_stream(m: &mut Map<String, Value>, protocol: &str, ss: &Value) {
    let sni_key = if matches!(protocol, "vless" | "vmess") { "servername" } else { "sni" };
    match s(ss, "security") {
        Some("tls") => {
            if matches!(protocol, "vless" | "vmess" | "socks" | "http") {
                m.insert("tls".into(), json!(true));
            }
            if let Some(t) = ss.get("tlsSettings") {
                put(m, sni_key, s(t, "serverName").map(|v| json!(v)));
                put(m, "alpn", t.get("alpn").filter(|a| a.as_array().is_some_and(|x| !x.is_empty())).cloned());
                put(m, "client-fingerprint", s(t, "fingerprint").map(|v| json!(v)));
                if t.get("allowInsecure").and_then(Value::as_bool) == Some(true) {
                    m.insert("skip-cert-verify".into(), json!(true));
                }
            }
        }
        Some("reality") => {
            m.insert("tls".into(), json!(true));
            if let Some(r) = ss.get("realitySettings") {
                put(m, sni_key, s(r, "serverName").map(|v| json!(v)));
                m.insert("client-fingerprint".into(), json!(s(r, "fingerprint").unwrap_or("chrome")));
                let mut ro = Map::new();
                put(&mut ro, "public-key", s(r, "publicKey").or_else(|| s(r, "password")).map(|v| json!(v)));
                put(&mut ro, "short-id", s(r, "shortId").map(|v| json!(v)));
                m.insert("reality-opts".into(), Value::Object(ro));
            }
        }
        _ => {}
    }
    match s(ss, "network").unwrap_or("tcp") {
        "ws" => {
            m.insert("network".into(), json!("ws"));
            let w = ss.get("wsSettings").cloned().unwrap_or(json!({}));
            let mut ws = Map::new();
            put(&mut ws, "path", s(&w, "path").map(|v| json!(v)));
            let host = s(&w, "host").or_else(|| w.get("headers").and_then(|h| s(h, "Host")));
            put(&mut ws, "headers", host.map(|h| json!({ "Host": h })));
            m.insert("ws-opts".into(), Value::Object(ws));
        }
        "httpupgrade" => {
            m.insert("network".into(), json!("ws"));
            let w = ss.get("httpupgradeSettings").cloned().unwrap_or(json!({}));
            let mut ws = Map::new();
            put(&mut ws, "path", s(&w, "path").map(|v| json!(v)));
            put(&mut ws, "headers", s(&w, "host").map(|h| json!({ "Host": h })));
            ws.insert("v2ray-http-upgrade".into(), json!(true));
            m.insert("ws-opts".into(), Value::Object(ws));
        }
        "grpc" => {
            m.insert("network".into(), json!("grpc"));
            let g = ss.get("grpcSettings").cloned().unwrap_or(json!({}));
            m.insert("grpc-opts".into(), json!({ "grpc-service-name": s(&g, "serviceName").unwrap_or("") }));
        }
        "h2" | "http" => {
            m.insert("network".into(), json!("h2"));
            let h = ss.get("httpSettings").cloned().unwrap_or(json!({}));
            let mut h2 = Map::new();
            put(&mut h2, "host", h.get("host").cloned());
            put(&mut h2, "path", s(&h, "path").map(|v| json!(v)));
            m.insert("h2-opts".into(), Value::Object(h2));
        }
        "xhttp" | "splithttp" => {
            m.insert("network".into(), json!("xhttp"));
            let x = ss.get("xhttpSettings").or_else(|| ss.get("splithttpSettings")).cloned().unwrap_or(json!({}));
            let mut xo = Map::new();
            put(&mut xo, "path", s(&x, "path").map(|v| json!(v)));
            put(&mut xo, "host", s(&x, "host").map(|v| json!(v)));
            put(&mut xo, "mode", s(&x, "mode").map(|v| json!(v)));
            m.insert("xhttp-opts".into(), Value::Object(xo));
        }
        _ => {}
    }
}

/// Скорость Xray: число или «60» без единицы — биты в секунду, «60 mbps» — как записано.
fn xray_speed(v: Option<&Value>) -> Option<Value> {
    match v? {
        Value::Number(n) => n.as_u64().filter(|n| *n > 0).map(|n| json!(format!("{n} bps"))),
        Value::String(x) if !x.trim().is_empty() && x.trim().chars().all(|c| c.is_ascii_digit()) => {
            x.trim().parse::<u64>().ok().filter(|n| *n > 0).map(|n| json!(format!("{n} bps")))
        }
        Value::String(x) => bandwidth(x).map(Value::from),
        _ => None,
    }
}

fn split_endpoint(ep: &str) -> Option<(String, u64)> {
    let (host, p) = ep.trim().rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Some((host.to_string(), p.parse().ok()?))
}

// ── WireGuard / AmneziaWG .conf ─────────────────────────────────────────

/// Параметры AmneziaWG в `.conf` (ключи в нижнем регистре).
const AWG_KEYS: [&str; 20] = [
    "jc", "jmin", "jmax", "s1", "s2", "s3", "s4", "h1", "h2", "h3", "h4", "i1", "i2", "i3", "i4", "i5", "j1", "j2", "j3", "itime",
];

/// Один параметр AmneziaWG → `amnezia-wg-option`. Числа — Jc, Jmin, Jmax, S1–S4, Itime; H1–H4 — число
/// или диапазон `100-200` (AWG 2.0), I1–I5 и J1–J3 — строки как есть. `0` — «не задано»: так пишет
/// `awg showconf`; ядро нулевое число и так пропускает, а нулевой H стал бы заголовком рукопожатия.
fn awg_param(awg: &mut Map<String, Value>, key: &str, raw: &str) {
    let raw = raw.trim();
    match key {
        "jc" | "jmin" | "jmax" | "s1" | "s2" | "s3" | "s4" | "itime" => {
            if let Some(n) = raw.parse::<u64>().ok().filter(|n| *n > 0) {
                awg.insert(key.into(), json!(n));
            }
        }
        "h1" | "h2" | "h3" | "h4" => {
            if !raw.is_empty() && raw.parse::<u64>() != Ok(0) {
                awg.insert(key.into(), json!(raw));
            }
        }
        "i1" | "i2" | "i3" | "i4" | "i5" | "j1" | "j2" | "j3" => {
            if !raw.is_empty() {
                awg.insert(key.into(), json!(raw));
            }
        }
        _ => {}
    }
}

/// `PersistentKeepalive`: секунды; диапазон AWG 3 (`22-30`) — нижняя граница; `0` и `off` — не писать.
fn keepalive(raw: &str) -> Option<u64> {
    raw.trim().split('-').next()?.trim().parse::<u64>().ok().filter(|n| *n > 0)
}

fn wireguard_conf(text: &str) -> Option<Value> {
    let mut section = String::new();
    let mut iface: Vec<(String, String)> = Vec::new();
    let mut peer: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.split(['#', ';']).next().unwrap_or("").trim();
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']']).to_ascii_lowercase();
            // Берём первый [Peer]: у клиентского конфига он один.
            if section == "peer" && !peer.is_empty() {
                section = "skip".into();
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let kv = (k.trim().to_ascii_lowercase(), v.trim().to_string());
        match section.as_str() {
            "interface" => iface.push(kv),
            "peer" => peer.push(kv),
            _ => {}
        }
    }
    let get = |list: &[(String, String)], k: &str| list.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
    let (host, p) = split_endpoint(&get(&peer, "endpoint")?)?;
    let mut m = Map::new();
    m.insert("name".into(), json!(host.clone()));
    m.insert("type".into(), json!("wireguard"));
    m.insert("server".into(), json!(host));
    m.insert("port".into(), json!(p));
    m.insert("private-key".into(), json!(get(&iface, "privatekey")?));
    m.insert("public-key".into(), json!(get(&peer, "publickey")?));
    put(&mut m, "pre-shared-key", get(&peer, "presharedkey").map(|v| json!(v)));
    let addresses: Vec<String> = get(&iface, "address").unwrap_or_default().split(',').map(|a| a.trim().to_string()).collect();
    wg_addresses(&mut m, &addresses);
    put(&mut m, "mtu", get(&iface, "mtu").and_then(|v| v.parse::<u64>().ok()).map(|v| json!(v)));
    let allowed: Vec<String> = get(&peer, "allowedips").unwrap_or_default().split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    if !allowed.is_empty() {
        m.insert("allowed-ips".into(), json!(allowed));
    }
    put(&mut m, "persistent-keepalive", get(&peer, "persistentkeepalive").and_then(|v| keepalive(&v)).map(|v| json!(v)));
    // AmneziaWG: Jc, Jmin, Jmax, S1–S4, H1–H4, I1–I5, J1–J3, Itime — параметры маскировки.
    let mut awg = Map::new();
    for k in AWG_KEYS {
        if let Some(v) = get(&iface, k) {
            awg_param(&mut awg, k, &v);
        }
    }
    if !awg.is_empty() {
        m.insert("amnezia-wg-option".into(), Value::Object(awg));
    }
    m.insert("udp".into(), json!(true));
    Some(Value::Object(m))
}

// ── Shadowsocks SIP008 и ключи Outline ──────────────────────────────────

fn sip008(v: &Value) -> Vec<Value> {
    let list: Vec<&Value> = match v.get("servers").and_then(Value::as_array) {
        Some(a) => a.iter().collect(),
        None => vec![v],
    };
    list.into_iter()
        .filter(|x| is_ss_server(x))
        .filter_map(|x| {
            let mut m = Map::new();
            let server = s(x, "server")?;
            m.insert("name".into(), json!(s(x, "remarks").or_else(|| s(x, "name")).unwrap_or(server)));
            m.insert("type".into(), json!("ss"));
            m.insert("server".into(), json!(server));
            m.insert("port".into(), json!(port(x, "server_port")?));
            m.insert("cipher".into(), json!(s(x, "method")?));
            m.insert("password".into(), json!(s(x, "password")?));
            m.insert("udp".into(), json!(true));
            ss_plugin(&mut m, s(x, "plugin"), s(x, "plugin_opts"));
            Some(Value::Object(m))
        })
        .collect()
}

// ── Ссылки Hysteria ─────────────────────────────────────────────────────

/// Серверы mihomo из ссылок Hysteria; негодные ссылки пропускаем, одинаковые имена нумеруем.
pub fn hysteria_proxies(links: &[&str]) -> Vec<Value> {
    let mut out: Vec<Value> = links.iter().filter_map(|l| hysteria_link(l)).collect();
    unique_names(&mut out);
    out
}

/// Ссылка `hysteria2://`, `hy2://` или `hysteria://` → сервер mihomo. `None` — ссылка негодная.
/// Разбираем сами, а не отдаём ядру: его конвертер оставляет в пароле `%XX` и `user%3Apass`, теряет
/// IPv6 с прыжками по портам, `mport` и `ech`, не знает `obfsParam` Hysteria 1, оставляет безымянному
/// серверу пустое имя, а `obfs` без пароля или Hysteria 1 без скорости роняют весь файл серверов.
pub fn hysteria_link(line: &str) -> Option<Value> {
    use crate::sub::percent_decode;
    let (scheme, rest) = line.trim().split_once("://")?;
    let v2 = match scheme.to_ascii_lowercase().as_str() {
        "hysteria2" | "hy2" => true,
        "hysteria" => false,
        _ => return None,
    };
    let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
    let authority = rest.split('/').next().unwrap_or("");
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(percent_decode(u)).filter(|u| !u.is_empty()), h),
        None => (None, authority),
    };
    let (host, ports) = match hostport.strip_prefix('[') {
        Some(v6) => {
            let (host, tail) = v6.split_once(']')?;
            (host, tail.strip_prefix(':'))
        }
        None => match hostport.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (hostport, None),
        },
    };
    if host.is_empty() {
        return None;
    }
    let q: Vec<(String, String)> = query
        .split('&')
        .filter_map(|kv| kv.split_once('=').or(Some((kv, ""))))
        .filter(|(k, _)| !k.is_empty())
        .map(|(k, v)| (k.to_ascii_lowercase(), percent_decode(v)))
        .collect();
    let get = |k: &str| q.iter().find(|(x, v)| x == k && !v.trim().is_empty()).map(|(_, v)| v.trim().to_string());
    // Адрес: `host`, `host:443`, `host:443,20000-30000`; `mport` — те же прыжки отдельным параметром.
    let (port_, hop) = match ports.filter(|p| !p.is_empty()) {
        Some(p) => parse_ports(p)?,
        None => (443, None),
    };
    let hop = match get("mport") {
        Some(mport) => parse_ports(&mport)?.1.or(hop),
        None => hop,
    };
    let truthy = |k: &str| get(k).is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
    let name = percent_decode(fragment).trim().to_string();
    let mut m = Map::new();
    m.insert("name".into(), json!(if name.is_empty() { host } else { name.as_str() }));
    m.insert("type".into(), json!(if v2 { "hysteria2" } else { "hysteria" }));
    m.insert("server".into(), json!(host));
    m.insert("port".into(), json!(port_));
    put(&mut m, "ports", hop.map(|h| json!(h)));
    if truthy("insecure") {
        m.insert("skip-cert-verify".into(), json!(true));
    }
    let alpn: Vec<String> = get("alpn").map(|a| a.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    if !alpn.is_empty() {
        m.insert("alpn".into(), json!(alpn));
    }
    if v2 {
        put(&mut m, "sni", get("sni").map(|v| json!(v)));
        // Весь userinfo — пароль: сервер с userpass ждёт `user:pass` целиком.
        put(&mut m, "password", userinfo.map(|u| json!(u)));
        if let Some(obfs) = get("obfs") {
            let obfs = obfs.to_ascii_lowercase();
            if !matches!(obfs.as_str(), "salamander" | "gecko") {
                return None;
            }
            m.insert("obfs".into(), json!(obfs));
            m.insert("obfs-password".into(), json!(get("obfs-password")?));
        }
        put(&mut m, "up", get("up").and_then(|v| bandwidth(&v)).map(Value::from));
        put(&mut m, "down", get("down").and_then(|v| bandwidth(&v)).map(Value::from));
        if let Some(pin) = get("pinsha256") {
            // Ядро принимает только 64 hex-символа (двоеточия оно убирает само) и иначе роняет весь файл.
            let pin: String = pin.chars().filter(|c| *c != ':' && !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
            if pin.len() != 64 || !pin.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            m.insert("fingerprint".into(), json!(pin));
        }
        put(&mut m, "ech-opts", get("ech").map(|c| json!({ "enable": true, "config": c })));
    } else {
        put(&mut m, "sni", get("peer").or_else(|| get("sni")).map(|v| json!(v)));
        put(&mut m, "auth-str", get("auth").or(userinfo).map(|v| json!(v)));
        // Скорость у Hysteria 1 обязательна.
        m.insert("up".into(), json!(bandwidth(&get("upmbps").or_else(|| get("up"))?)?));
        m.insert("down".into(), json!(bandwidth(&get("downmbps").or_else(|| get("down"))?)?));
        // `obfs=xplus&obfsParam=пароль`; у части программ пароль лежит прямо в `obfs`.
        put(&mut m, "obfs", get("obfsparam").or_else(|| get("obfs").filter(|o| !o.eq_ignore_ascii_case("xplus"))).map(|v| json!(v)));
        if let Some(proto) = get("protocol") {
            if !matches!(proto.as_str(), "udp" | "wechat-video" | "faketcp") {
                return None;
            }
            m.insert("protocol".into(), json!(proto));
        }
    }
    Some(Value::Object(m))
}

/// Порты ссылки и конфигов: `443`, `20000-30000`, `443,20000-30000` (`:` вместо `-` — как в sing-box).
/// Возвращает первый порт и — если портов больше одного — список в виде, который читает ядро.
fn parse_ports(spec: &str) -> Option<(u16, Option<String>)> {
    let mut first = None;
    let mut parts = Vec::new();
    for part in spec.split(',') {
        let part = part.trim().replace(':', "-");
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim().parse::<u16>().ok()?, b.trim().parse::<u16>().ok()?),
            None => {
                let p = part.parse::<u16>().ok()?;
                (p, p)
            }
        };
        if a == 0 || a > b {
            return None;
        }
        first.get_or_insert(a);
        parts.push(if a == b { a.to_string() } else { format!("{a}-{b}") });
    }
    let multi = parts.len() > 1 || parts.first().is_some_and(|p| p.contains('-'));
    Some((first?, multi.then(|| parts.join(","))))
}

/// Скорость как её читает ядро: «100» → «100 Mbps», «100 mbps», «1gbps», «10 MBps»; непонятное — `None`.
/// Латинская `b` — биты, `B` — байты (как в Hysteria); без единицы — мегабиты.
fn bandwidth(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let digits = raw.chars().take_while(char::is_ascii_digit).count();
    let n: u64 = raw[..digits].parse().ok().filter(|n| *n > 0)?;
    let unit = raw[digits..].trim();
    if unit.is_empty() {
        return Some(format!("{n} Mbps"));
    }
    let (prefix, tail) = match unit.chars().next()? {
        c @ ('k' | 'K' | 'm' | 'M' | 'g' | 'G' | 't' | 'T') => (c.to_ascii_uppercase().to_string(), &unit[1..]),
        _ => (String::new(), unit),
    };
    // «b» или «B», затем необязательное «ps» (в любом регистре) или «/s».
    let (b, ps) = tail.split_at(if tail.starts_with(['b', 'B']) { 1 } else { 0 });
    if b.is_empty() && !prefix.is_empty() && ps.is_empty() {
        // «1g» — как у Xray, биты.
        return Some(format!("{n} {prefix}bps"));
    }
    (!b.is_empty() && matches!(ps.to_ascii_lowercase().as_str(), "" | "ps" | "/s")).then(|| format!("{n} {prefix}{b}ps"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGBOX: &str = r#"{
      "dns": { "servers": [{ "tag": "d", "type": "local" }] },
      "inbounds": [{ "type": "tun", "tag": "tun-in" }],
      "outbounds": [
        { "type": "vless", "tag": "vless-out", "server": "203.0.113.5", "server_port": 443, "uuid": "11111111-2222-4333-8444-555555555555",
          "flow": "xtls-rprx-vision",
          "tls": { "enabled": true, "server_name": "example.com", "utls": { "enabled": true, "fingerprint": "chrome" },
                   "reality": { "enabled": true, "public_key": "Q2sDgDCjyNy_9Ydp1s7yI3i7WEKtMP3vOIZ2Ahj1OxM", "short_id": "ab12" } } },
        { "type": "hysteria2", "tag": "hy", "server": "hy.example.com", "server_port": 8443, "password": "p",
          "obfs": { "type": "salamander", "password": "o" }, "tls": { "enabled": true, "server_name": "hy.example.com" } },
        { "type": "vmess", "tag": "vm", "server": "vm.example.com", "server_port": 443, "uuid": "11111111-2222-4333-8444-555555555555", "transport": { "type": "ws", "path": "/w", "headers": { "Host": "cdn.example.com" } }, "tls": { "enabled": true } },
        { "type": "direct", "tag": "direct-out" },
        { "type": "block", "tag": "reject" }
      ],
      "endpoints": [{ "type": "wireguard", "tag": "wg", "address": ["10.0.0.2/32", "fd00::2/128"], "private_key": "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=",
                      "peers": [{ "address": "wg.example.com", "port": 51820, "public_key": "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=", "allowed_ips": ["0.0.0.0/0"] }] }]
    }"#;

    #[test]
    fn singbox_servers_only() {
        assert_eq!(detect(SINGBOX), Some(Format::SingBox));
        let p = proxies(SINGBOX, Format::SingBox);
        assert_eq!(p.len(), 4, "{p:#?}");
        assert_eq!(p[0]["type"], "vless");
        assert_eq!(p[0]["servername"], "example.com");
        assert_eq!(p[0]["flow"], "xtls-rprx-vision");
        assert_eq!(p[0]["client-fingerprint"], "chrome");
        assert_eq!(p[0]["reality-opts"]["public-key"], "Q2sDgDCjyNy_9Ydp1s7yI3i7WEKtMP3vOIZ2Ahj1OxM");
        assert_eq!(p[0]["tls"], true);
        assert_eq!(p[1]["type"], "hysteria2");
        assert_eq!(p[1]["obfs"], "salamander");
        assert_eq!(p[1]["sni"], "hy.example.com");
        assert_eq!(p[2]["network"], "ws");
        assert_eq!(p[2]["ws-opts"]["headers"]["Host"], "cdn.example.com");
        assert_eq!(p[3]["type"], "wireguard");
        assert_eq!(p[3]["ip"], "10.0.0.2");
        assert_eq!(p[3]["ipv6"], "fd00::2");
    }

    const XRAY: &str = r#"[
      { "remarks": "🇩🇪 Германия", "outbounds": [
        { "tag": "proxy", "protocol": "vless",
          "settings": { "vnext": [{ "address": "de.example.com", "port": 443, "users": [{ "id": "11111111-2222-4333-8444-555555555555", "flow": "xtls-rprx-vision", "encryption": "none" }] }] },
          "streamSettings": { "network": "tcp", "security": "reality",
            "realitySettings": { "serverName": "www.example.com", "publicKey": "Q2sDgDCjyNy_9Ydp1s7yI3i7WEKtMP3vOIZ2Ahj1OxM", "shortId": "01", "fingerprint": "firefox" } } },
        { "tag": "direct", "protocol": "freedom" }, { "tag": "block", "protocol": "blackhole" } ] },
      { "remarks": "🇫🇮 Финляндия", "outbounds": [
        { "tag": "proxy", "protocol": "vless",
          "settings": { "vnext": [{ "address": "fi.example.com", "port": 443, "users": [{ "id": "11111111-2222-4333-8444-555555555555" }] }] },
          "streamSettings": { "network": "xhttp", "security": "tls", "tlsSettings": { "serverName": "fi.example.com" },
            "xhttpSettings": { "path": "/x", "mode": "auto" } } } ] }
    ]"#;

    #[test]
    fn xray_array_like_remnawave() {
        assert_eq!(detect(XRAY), Some(Format::Xray));
        let p = proxies(XRAY, Format::Xray);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0]["name"], "🇩🇪 Германия");
        assert_eq!(p[0]["reality-opts"]["short-id"], "01");
        assert_eq!(p[0]["client-fingerprint"], "firefox");
        assert_eq!(p[1]["network"], "xhttp");
        assert_eq!(p[1]["xhttp-opts"]["path"], "/x");
        assert_eq!(p[1]["tls"], true);
    }

    const AWG: &str = "[Interface]\nPrivateKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAddress = 10.8.0.2/32, fd00::2/128\nDNS = 1.1.1.1\nMTU = 1280\nJc = 4\nJmin = 40\nJmax = 70\nS1 = 0\nS2 = 0\nH1 = 1\nH2 = 2\nH3 = 3\nH4 = 4\n\n[Peer]\nPublicKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nPresharedKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = [2001:db8::1]:51820\n";

    #[test]
    fn amneziawg_conf() {
        assert_eq!(detect(AWG), Some(Format::WireGuard));
        let p = proxies(AWG, Format::WireGuard);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0]["server"], "2001:db8::1");
        assert_eq!(p[0]["port"], 51820);
        assert_eq!(p[0]["pre-shared-key"], "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=");
        assert_eq!(p[0]["mtu"], 1280);
        assert_eq!(p[0]["amnezia-wg-option"]["jmax"], 70);
        assert_eq!(p[0]["allowed-ips"], json!(["0.0.0.0/0", "::/0"]));
    }

    #[test]
    fn shadowsocks_sip008_and_outline_key() {
        let sip = r#"{ "version": 1, "servers": [{ "server": "a.example.com", "server_port": 8388, "password": "p", "method": "chacha20-ietf-poly1305", "remarks": "A" },
                                              { "server": "b.example.com", "server_port": 8388, "password": "p", "method": "aes-256-gcm", "remarks": "A" }] }"#;
        assert_eq!(detect(sip), Some(Format::Shadowsocks));
        let p = proxies(sip, Format::Shadowsocks);
        assert_eq!(p.len(), 2);
        assert_eq!(p[1]["name"], "A 2", "одинаковые имена — с номером");
        let outline = r#"{ "server": "o.example.com", "server_port": 443, "password": "p", "method": "chacha20-ietf-poly1305" }"#;
        assert_eq!(detect(outline), Some(Format::Shadowsocks));
        assert_eq!(proxies(outline, Format::Shadowsocks)[0]["cipher"], "chacha20-ietf-poly1305");
    }

    #[test]
    fn not_ours() {
        assert_eq!(detect("proxies:\n  - name: a"), None);
        assert_eq!(detect(r#"{ "common": { "add": "Добавить" } }"#), None);
        assert_eq!(detect(r#"{ "proxies": [] }"#), None);
        assert_eq!(detect("vless://x@y:1#z"), None);
    }

    #[test]
    fn hysteria2_link_official_example() {
        let l = "hysteria2://letmein@example.com:123,5000-6000/?insecure=1&obfs=salamander&obfs-password=gawrgura&pinSHA256=DE:AD:BE:EF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB&sni=real.example.com&alpn=h3,h3-29#%F0%9F%87%A9%F0%9F%87%AA%20cool";
        let p = hysteria_link(l).unwrap();
        assert_eq!(p["name"], "🇩🇪 cool");
        assert_eq!(p["type"], "hysteria2");
        assert_eq!((p["server"].as_str(), p["port"].as_u64(), p["ports"].as_str()), (Some("example.com"), Some(123), Some("123,5000-6000")));
        assert_eq!(p["password"], "letmein");
        assert_eq!((p["obfs"].as_str(), p["obfs-password"].as_str()), (Some("salamander"), Some("gawrgura")));
        assert_eq!(p["sni"], "real.example.com");
        assert_eq!(p["skip-cert-verify"], true);
        assert_eq!(p["alpn"], json!(["h3", "h3-29"]));
        assert_eq!(p["fingerprint"], "deadbeef00112233445566778899aabbccddeeff00112233445566778899aabb");
    }

    /// Конвертер ядра оставляет в пароле `%40` и `%3A`: сервер получает не тот пароль.
    #[test]
    fn hysteria2_link_password_is_decoded() {
        let p = hysteria_link("hy2://user:p%40ss%3Aw0rd%2F%23@example.com:443#n").unwrap();
        assert_eq!(p["password"], "user:p@ss:w0rd/#", "userinfo целиком — это auth сервера");
        assert_eq!(hysteria_link("hy2://user:pa:ss@example.com#n").unwrap()["password"], "user:pa:ss");
        assert!(hysteria_link("hy2://example.com:443#n").unwrap().get("password").is_none());
    }

    #[test]
    fn hysteria2_link_addresses() {
        // IPv6 с прыжками по портам ядро молча выбрасывает.
        let p = hysteria_link("hysteria2://pw@[2001:db8::1]:443,20000-30000/?sni=a.b#v6").unwrap();
        assert_eq!((p["server"].as_str(), p["port"].as_u64(), p["ports"].as_str()), (Some("2001:db8::1"), Some(443), Some("443,20000-30000")));
        let p = hysteria_link("hysteria2://pw@example.com:20000-30000").unwrap();
        assert_eq!((p["port"].as_u64(), p["ports"].as_str()), (Some(20000), Some("20000-30000")));
        // Без порта — 443, без имени — адрес сервера (у ядра имя пустое, и сервер не выбрать).
        let p = hysteria_link("HY2://pw@example.com/?sni=a.b").unwrap();
        assert_eq!((p["port"].as_u64(), p["name"].as_str(), p["ports"].as_str()), (Some(443), Some("example.com"), None));
        assert_eq!(hysteria_link("hysteria2://pw@[::1]").unwrap()["name"], "::1");
        // `mport` — прыжки отдельным параметром.
        let p = hysteria_link("hysteria2://pw@example.com:443/?mport=20000-30000#m").unwrap();
        assert_eq!((p["port"].as_u64(), p["ports"].as_str()), (Some(443), Some("20000-30000")));
        assert!(hysteria_link("hysteria2://pw@example.com:70000").is_none());
        assert!(hysteria_link("hysteria2://pw@example.com:30000-20000").is_none());
        assert!(hysteria_link("hysteria2://pw@:443").is_none());
        assert!(hysteria_link("vless://a@b:1").is_none());
    }

    /// Ядро роняет весь файл серверов из-за одного такого сервера.
    #[test]
    fn hysteria2_link_rejects_what_would_break_the_core() {
        assert!(hysteria_link("hysteria2://pw@e.com:443/?obfs=salamander").is_none(), "obfs без пароля");
        assert!(hysteria_link("hysteria2://pw@e.com:443/?obfs=xplus&obfs-password=x").is_none(), "неизвестный obfs");
        assert!(hysteria_link("hysteria2://pw@e.com:443/?pinSHA256=abc").is_none(), "отпечаток не sha256");
        let p = hysteria_link("hysteria2://pw@e.com:443/?obfs=Gecko&obfs-password=x").unwrap();
        assert_eq!(p["obfs"], "gecko");
    }

    #[test]
    fn hysteria_speed_units() {
        assert_eq!(bandwidth("100").as_deref(), Some("100 Mbps"));
        assert_eq!(bandwidth("100 mbps").as_deref(), Some("100 Mbps"), "ядро строчное mbps не знает и считает скорость нулём");
        assert_eq!(bandwidth("1gbps").as_deref(), Some("1 Gbps"));
        assert_eq!(bandwidth("10 MBps").as_deref(), Some("10 MBps"));
        assert_eq!(bandwidth("500 kbps").as_deref(), Some("500 Kbps"));
        assert_eq!(bandwidth("1g").as_deref(), Some("1 Gbps"));
        assert_eq!(bandwidth("12.5 mbps"), None);
        assert_eq!(bandwidth("0"), None);
        assert_eq!(bandwidth("fast"), None);
        let p = hysteria_link("hy2://pw@e.com:443/?up=20&down=100%20mbps").unwrap();
        assert_eq!((p["up"].as_str(), p["down"].as_str()), (Some("20 Mbps"), Some("100 Mbps")));
    }

    #[test]
    fn hysteria2_link_ech() {
        let p = hysteria_link("hysteria2://pw@e.com:443/?ech=AEX%2BDQA%3D&sni=a.b").unwrap();
        assert_eq!(p["ech-opts"], json!({ "enable": true, "config": "AEX+DQA=" }));
    }

    #[test]
    fn hysteria1_link() {
        let l = "hysteria://example.com:443?protocol=wechat-video&auth=secret&peer=sni.example&insecure=1&upmbps=50&downmbps=200&alpn=hysteria&obfs=xplus&obfsParam=pw#v1";
        let p = hysteria_link(l).unwrap();
        assert_eq!(p["type"], "hysteria");
        assert_eq!((p["auth-str"].as_str(), p["sni"].as_str(), p["protocol"].as_str()), (Some("secret"), Some("sni.example"), Some("wechat-video")));
        assert_eq!((p["up"].as_str(), p["down"].as_str()), (Some("50 Mbps"), Some("200 Mbps")));
        // У Hysteria 1 пароль обфускации — obfsParam, а не слово xplus (конвертер ядра берёт xplus как пароль).
        assert_eq!(p["obfs"], "pw");
        assert_eq!(p["skip-cert-verify"], true);
        let hop = hysteria_link("hysteria://example.com:443?auth=a&upmbps=1&downmbps=1&mport=20000-30000").unwrap();
        assert_eq!(hop["ports"], "20000-30000");
        // Без скорости ядро отвергает весь файл.
        assert!(hysteria_link("hysteria://example.com:443?auth=a").is_none());
    }

    #[test]
    fn hysteria_proxies_get_unique_names() {
        let list = hysteria_proxies(&["hy2://a@h1.example.com:443#DE", "hy2://b@h2.example.com:443#DE", "hy2://obfs@h3.example.com/?obfs=salamander", "vless://x@y:1"]);
        assert_eq!(list.len(), 2);
        assert_eq!(list[1]["name"], "DE 2");
    }

    #[test]
    fn singbox_hysteria2_hop_and_speeds() {
        let cfg = r#"{"outbounds": [
          { "type": "hysteria2", "tag": "a", "server": "h.example.com", "server_ports": ["20000:30000", "443"], "hop_interval": "30s", "hop_interval_max": "1m",
            "up_mbps": 50, "down_mbps": 200, "password": "p", "obfs": { "type": "salamander", "password": "o" },
            "tls": { "enabled": true, "server_name": "h.example.com", "insecure": true, "alpn": ["h3"] } },
          { "type": "hysteria2", "tag": "b", "server": "h2.example.com", "server_port": 8443, "up": "100 Mbps", "password": "p", "tls": { "enabled": true } }
        ]}"#;
        let p = proxies(cfg, Format::SingBox);
        assert_eq!(p.len(), 2, "{p:#?}");
        // Порта нет — только прыжки: первый порт списка становится основным.
        assert_eq!((p[0]["port"].as_u64(), p[0]["ports"].as_str()), (Some(20000), Some("20000-30000,443")));
        assert_eq!((p[0]["hop-interval"].as_str(), p[0]["up"].as_str(), p[0]["down"].as_str()), (Some("30-60"), Some("50 Mbps"), Some("200 Mbps")));
        assert_eq!((p[0]["skip-cert-verify"].as_bool(), p[0]["alpn"].clone()), (Some(true), json!(["h3"])));
        assert_eq!((p[1]["port"].as_u64(), p[1]["up"].as_str()), (Some(8443), Some("100 Mbps")));
        assert!(p[1].get("ports").is_none() && p[1].get("hop-interval").is_none());
    }

    #[test]
    fn singbox_hysteria1() {
        let cfg = r#"{"outbounds": [
          { "type": "hysteria", "tag": "h1", "server": "h.example.com", "server_port": 443, "up_mbps": 20, "down": "100 Mbps", "auth_str": "s", "obfs": "xp",
            "recv_window_conn": 1048576, "disable_mtu_discovery": true, "tls": { "enabled": true, "server_name": "h.example.com", "alpn": ["hysteria"] } },
          { "type": "hysteria", "tag": "no-speed", "server": "h.example.com", "server_port": 443, "auth_str": "s", "tls": { "enabled": true } }
        ]}"#;
        let p = proxies(cfg, Format::SingBox);
        assert_eq!(p.len(), 1, "без скорости Hysteria 1 ядро не примет: {p:#?}");
        assert_eq!(p[0]["type"], "hysteria");
        assert_eq!((p[0]["up"].as_str(), p[0]["down"].as_str(), p[0]["auth-str"].as_str(), p[0]["obfs"].as_str()), (Some("20 Mbps"), Some("100 Mbps"), Some("s"), Some("xp")));
        assert_eq!((p[0]["recv-window-conn"].as_u64(), p[0]["disable-mtu-discovery"].as_bool(), p[0]["sni"].as_str()), (Some(1048576), Some(true), Some("h.example.com")));
    }

    #[test]
    fn xray_hysteria() {
        let cfg = r#"{ "remarks": "HY", "outbounds": [
          { "tag": "proxy", "protocol": "hysteria", "settings": { "version": 2, "address": "h.example.com", "port": 443 },
            "streamSettings": { "network": "hysteria", "security": "tls",
              "tlsSettings": { "serverName": "h.example.com", "alpn": ["h3"], "fingerprint": "chrome", "pinnedPeerCertSha256": "e8e2d387fdbffeb38e9c9065cf30a97ee23c0e3d32ee6f78ffae40966befccc9" },
              "hysteriaSettings": { "version": 2, "auth": "secret" },
              "finalmask": { "udp": [{ "type": "salamander", "settings": { "password": "obfs-pw" } }],
                "quicParams": { "brutalUp": "60 mbps", "brutalDown": 0, "udpHop": { "ports": "20000-50000", "interval": "5-10" } } } } },
          { "tag": "direct", "protocol": "freedom" } ] }"#;
        assert_eq!(detect(cfg), Some(Format::Xray));
        let p = proxies(cfg, Format::Xray);
        assert_eq!(p.len(), 1, "{p:#?}");
        let h = &p[0];
        assert_eq!((h["type"].as_str(), h["password"].as_str(), h["obfs"].as_str(), h["obfs-password"].as_str()), (Some("hysteria2"), Some("secret"), Some("salamander"), Some("obfs-pw")));
        assert_eq!((h["up"].as_str(), h["ports"].as_str(), h["hop-interval"].as_str(), h["sni"].as_str()), (Some("60 Mbps"), Some("20000-50000"), Some("5-10"), Some("h.example.com")));
        assert!(h.get("down").is_none() && h.get("client-fingerprint").is_none() && h.get("tls").is_none());
        assert_eq!(h["fingerprint"], "e8e2d387fdbffeb38e9c9065cf30a97ee23c0e3d32ee6f78ffae40966befccc9");
    }

    #[test]
    fn awg_two_point_zero_params() {
        // AWG 2.0: H1–H4 — диапазоны, плюс S3, S4 и I1–I5; AWG 1.5: J1–J3 и Itime.
        let conf = "[Interface]\nPrivateKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAddress = 10.8.0.2/32\nJc = 5\nJmin = 10\nJmax = 50\nS1 = 30\nS2 = 40\nS3 = 18\nS4 = 9\nH1 = 100000-200000\nH2 = 300000-400000\nH3 = 500000-600000\nH4 = 700000\nI1 = <b 0xf6ab3267fa><c><t><r 10>\nI2 = <r 20>\nJ1 = <b 0x00>\nItime = 7\n\n[Peer]\nPublicKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAllowedIPs = 0.0.0.0/0\nEndpoint = 203.0.113.5:51820\nPersistentKeepalive = 25\n";
        let p = &proxies(conf, Format::WireGuard)[0];
        let o = &p["amnezia-wg-option"];
        assert_eq!((o["jc"].as_u64(), o["s3"].as_u64(), o["s4"].as_u64(), o["itime"].as_u64()), (Some(5), Some(18), Some(9), Some(7)));
        assert_eq!((o["h1"].as_str(), o["h2"].as_str(), o["h3"].as_str(), o["h4"].as_str()), (Some("100000-200000"), Some("300000-400000"), Some("500000-600000"), Some("700000")));
        assert_eq!((o["i1"].as_str(), o["i2"].as_str(), o["j1"].as_str()), (Some("<b 0xf6ab3267fa><c><t><r 10>"), Some("<r 20>"), Some("<b 0x00>")));
        assert!(o.get("i3").is_none());
        assert_eq!(p["persistent-keepalive"], 25);
    }

    #[test]
    fn awg_zero_values_and_keepalive_forms() {
        // `awg showconf` пишет нули для незаданного: нулевые H не переносим, нулевые числа пропускает и ядро.
        let conf = |extra: &str| format!("[Interface]\nPrivateKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAddress = 10.8.0.2/32\nJc = 0\nJmin = 0\nJmax = 0\nS1 = 0\nS2 = 0\nH1 = 0\nH2 = 0\nH3 = 0\nH4 = 0\n\n[Peer]\nPublicKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAllowedIPs = 0.0.0.0/0\nEndpoint = 203.0.113.5:51820\n{extra}");
        let p = &proxies(&conf("PersistentKeepalive = 22-30\n"), Format::WireGuard)[0];
        assert!(p.get("amnezia-wg-option").is_none(), "одни нули — обычный WireGuard: {p}");
        assert_eq!(p["persistent-keepalive"], 22, "диапазон AWG 3 — нижняя граница");
        for off in ["PersistentKeepalive = 0\n", "PersistentKeepalive = off\n", ""] {
            assert!(proxies(&conf(off), Format::WireGuard)[0].get("persistent-keepalive").is_none(), "{off:?}");
        }
        // Часть параметров нулевая, часть нет: переносим только заданные.
        let part = conf("").replace("Jc = 0", "Jc = 4").replace("H1 = 0", "H1 = 123");
        let o = &proxies(&part, Format::WireGuard)[0]["amnezia-wg-option"];
        assert_eq!((o["jc"].as_u64(), o["h1"].as_str()), (Some(4), Some("123")));
        assert!(o.get("jmin").is_none() && o.get("h2").is_none());
    }

    #[test]
    fn wireguard_keepalive_and_awg_from_json_configs() {
        let sb = r#"{ "endpoints": [{ "type": "wireguard", "tag": "wg", "address": ["10.0.0.2/32"], "private_key": "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=",
            "junk_packet_count": 4, "junk_packet_min_size": 40, "junk_packet_max_size": 70, "init_packet_junk_size": 15, "response_packet_junk_size": 0,
            "init_packet_magic_header": 1234567, "response_packet_magic_header": 2345678,
            "peers": [{ "address": "wg.example.com", "port": 51820, "public_key": "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=", "allowed_ips": ["0.0.0.0/0"], "persistent_keepalive_interval": 25 }] }] }"#;
        let p = &proxies(sb, Format::SingBox)[0];
        assert_eq!(p["persistent-keepalive"], 25);
        let o = &p["amnezia-wg-option"];
        assert_eq!((o["jc"].as_u64(), o["jmax"].as_u64(), o["s1"].as_u64(), o["h1"].as_str()), (Some(4), Some(70), Some(15), Some("1234567")));
        assert!(o.get("s2").is_none());
        let xr = r#"{ "outbounds": [{ "protocol": "wireguard", "tag": "wg", "settings": { "secretKey": "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=", "address": ["10.0.0.2/32"],
            "peers": [{ "publicKey": "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=", "endpoint": "wg.example.com:51820", "keepAlive": 20 }] } }] }"#;
        assert_eq!(proxies(xr, Format::Xray)[0]["persistent-keepalive"], 20);
    }

    /// Свой файл: `KLICK_CONVERT_FILE=путь cargo test -p klick-core convert_file -- --ignored --nocapture`.
    /// Печатает только число и виды серверов — ключи из файла на экран не попадают.
    #[test]
    #[ignore]
    fn convert_file() {
        let path = std::env::var("KLICK_CONVERT_FILE").expect("KLICK_CONVERT_FILE");
        let body = std::fs::read_to_string(path).unwrap();
        let format = detect(&body).expect("формат не распознан");
        let list = proxies(&body, format);
        let kinds: Vec<String> = list.iter().map(|p| format!("{}{}", p["type"].as_str().unwrap_or("?"), p.get("network").and_then(Value::as_str).map(|n| format!("/{n}")).unwrap_or_default())).collect();
        println!("формат {format:?}, серверов {}: {}", list.len(), kinds.join(", "));
        check_with_mihomo(list);
    }

    fn check_with_mihomo(mut all: Vec<Value>) {
        let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("resources").join("core").join("mihomo.exe");
        unique_names(&mut all);
        let dir = std::env::temp_dir().join(format!("klick-convert-{}-{}", std::process::id(), all.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let names: Vec<Value> = all.iter().map(|p| p["name"].clone()).collect();
        let cfg = json!({ "mode": "rule", "proxies": all, "proxy-groups": [{ "name": "G", "type": "select", "proxies": names }], "rules": ["MATCH,G"] });
        let path = dir.join("config.yaml");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(core).args(["-t", "-d"]).arg(&dir).arg("-f").arg(&path).output().expect("mihomo.exe не запустился");
        let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success() && text.contains("test is successful"), "{text}");
        println!("mihomo -t: принято");
    }

    /// Настоящее ядро принимает то, что мы собрали: `cargo test -p klick-core mihomo_accepts -- --ignored`.
    #[test]
    #[ignore]
    fn mihomo_accepts_converted() {
        let mut all = proxies(SINGBOX, Format::SingBox);
        all.extend(proxies(XRAY, Format::Xray));
        all.extend(proxies(AWG, Format::WireGuard));
        check_with_mihomo(all);
    }

    /// Ядро принимает Hysteria и AmneziaWG 2.0 в нашем виде: `cargo test -p klick-core mihomo_accepts_hysteria -- --ignored`.
    #[test]
    #[ignore]
    fn mihomo_accepts_hysteria_and_awg() {
        let mut all = hysteria_proxies(&[
            "hysteria2://letmein@example.com:443,5000-6000/?insecure=1&obfs=salamander&obfs-password=gawrgura&pinSHA256=DE:AD:BE:EF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB&sni=real.example.com&up=100%20mbps&down=200&alpn=h3#a",
            "hy2://user:p%40ss%3Aw0rd@[2001:db8::1]:443,20000-30000/?ech=AEX%2BDQA%3D&obfs=gecko&obfs-password=g#b",
            "hysteria2://pw@example.org",
            "hysteria://example.net:443?protocol=udp&auth=s&peer=p.example&insecure=1&upmbps=10&downmbps=50&obfs=xplus&obfsParam=pw&mport=20000-30000#d",
        ]);
        assert_eq!(all.len(), 4);
        let sb = r#"{"outbounds": [
          { "type": "hysteria2", "tag": "sb-hy2", "server": "h.example.com", "server_ports": ["20000:30000", "443"], "hop_interval": "30s", "hop_interval_max": "1m",
            "up_mbps": 50, "down_mbps": 200, "password": "p", "obfs": { "type": "salamander", "password": "o" }, "tls": { "enabled": true, "server_name": "h.example.com", "insecure": true, "alpn": ["h3"] } },
          { "type": "hysteria", "tag": "sb-hy1", "server": "h.example.com", "server_port": 443, "up_mbps": 20, "down": "100 Mbps", "auth_str": "s", "obfs": "xp",
            "recv_window_conn": 1048576, "tls": { "enabled": true, "server_name": "h.example.com", "alpn": ["hysteria"] } }]}"#;
        all.extend(proxies(sb, Format::SingBox));
        let xr = r#"{ "remarks": "xray-hy", "outbounds": [
          { "tag": "proxy", "protocol": "hysteria", "settings": { "version": 2, "address": "h.example.com", "port": 443 },
            "streamSettings": { "network": "hysteria", "security": "tls",
              "tlsSettings": { "serverName": "h.example.com", "pinnedPeerCertSha256": "e8e2d387fdbffeb38e9c9065cf30a97ee23c0e3d32ee6f78ffae40966befccc9" },
              "hysteriaSettings": { "version": 2, "auth": "secret" },
              "finalmask": { "udp": [{ "type": "salamander", "settings": { "password": "obfs-pw", "packetSize": "512-1200" } }],
                "quicParams": { "brutalUp": "60 mbps", "udpHop": { "ports": "20000-50000", "interval": "5-10" } } } } } ] }"#;
        all.extend(proxies(xr, Format::Xray));
        let awg = "[Interface]\nPrivateKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAddress = 10.8.0.2/32\nJc = 5\nJmin = 10\nJmax = 50\nS1 = 30\nS2 = 40\nS3 = 18\nS4 = 9\nH1 = 100000-200000\nH2 = 300000-400000\nH3 = 500000-600000\nH4 = 700000\nI1 = <b 0xf6ab3267fa><c><t><r 10>\n\n[Peer]\nPublicKey = YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXphYmNkZWY=\nAllowedIPs = 0.0.0.0/0\nEndpoint = 203.0.113.5:51820\nPersistentKeepalive = 25\n";
        all.extend(proxies(awg, Format::WireGuard));
        check_with_mihomo(all);
    }
}
