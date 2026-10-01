//! Чужие форматы → серверы mihomo. Ядро kl!ck — mihomo, его proxy-provider читает YAML/JSON со списком
//! `proxies` и списки ссылок. Остальное переводим здесь: конфиги sing-box и Xray/V2Ray (в том числе
//! массивом — так отдаёт «xray-json» Remnawave), WireGuard/AmneziaWG `.conf`, Shadowsocks SIP008 и
//! ключи Outline. Берём только серверы: маршрутизация, DNS и TUN у kl!ck свои.

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
    let port_ = port(o, "server_port")?;
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
            put(&mut m, "up", port(o, "up_mbps").map(|v| json!(format!("{v} Mbps"))));
            put(&mut m, "down", port(o, "down_mbps").map(|v| json!(format!("{v} Mbps"))));
            if let Some(obfs) = o.get("obfs") {
                put(&mut m, "obfs", s(obfs, "type").map(|t| json!(t)));
                put(&mut m, "obfs-password", s(obfs, "password").map(|t| json!(t)));
            }
            // Прыжки по портам: "20000:30000" → "20000-30000".
            if let Some(ports) = o.get("server_ports").and_then(Value::as_array) {
                let list: Vec<String> = ports.iter().filter_map(Value::as_str).map(|p| p.replace(':', "-")).collect();
                if !list.is_empty() {
                    m.insert("ports".into(), json!(list.join(",")));
                }
            }
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
        "wireguard" => {
            let peer = settings.get("peers").and_then(Value::as_array).and_then(|a| a.first())?;
            let (host, p) = split_endpoint(s(peer, "endpoint")?)?;
            m.insert("type".into(), json!("wireguard"));
            m.insert("server".into(), json!(host));
            m.insert("port".into(), json!(p));
            m.insert("private-key".into(), json!(s(settings, "secretKey")?));
            m.insert("public-key".into(), json!(s(peer, "publicKey")?));
            put(&mut m, "pre-shared-key", s(peer, "preSharedKey").map(|v| json!(v)));
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

fn split_endpoint(ep: &str) -> Option<(String, u64)> {
    let (host, p) = ep.trim().rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Some((host.to_string(), p.parse().ok()?))
}

// ── WireGuard / AmneziaWG .conf ─────────────────────────────────────────

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
    // AmneziaWG: Jc, Jmin, Jmax, S1, S2, H1–H4 — параметры маскировки.
    let mut awg = Map::new();
    for k in ["jc", "jmin", "jmax", "s1", "s2", "h1", "h2", "h3", "h4"] {
        if let Some(v) = get(&iface, k).and_then(|v| v.parse::<u64>().ok()) {
            awg.insert(k.into(), json!(v));
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
}
