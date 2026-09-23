//! Ссылки vless/vmess/trojan/ss/hysteria2/tuic/anytls → прокси mihomo (те же
//! поля, что в секции `proxies:` YAML-конфига), и разбор YAML/JSON-конфигов
//! mihomo/Clash. Разбор URL — из KlutzBOX, где он покрыт тестами.

use serde_json::{json, Map, Value};

use crate::sys::base64_decode;

const SCHEMES: [&str; 8] = ["vless://", "vmess://", "trojan://", "ss://", "hysteria2://", "hy2://", "tuic://", "anytls://"];

pub fn is_link(line: &str) -> bool {
    let l = line.trim().to_ascii_lowercase();
    SCHEMES.iter().any(|s| l.starts_with(s))
}

/// Список ссылок, base64-список или YAML/JSON с `proxies:`. Битые строки
/// пропускаются; если не разобралось ничего — ошибка с причиной первой.
pub fn parse_any(input: &str) -> Result<Vec<Value>, String> {
    let text = input.trim().trim_start_matches('\u{feff}');
    if text.is_empty() {
        return Err("Пусто.".into());
    }
    if is_link(text) && !text.contains('\n') {
        return parse_link(text).map(|p| vec![p]);
    }
    if !text.lines().any(is_link) {
        // YAML/JSON-конфиг mihomo/Clash.
        if let Some(list) = yaml_proxies(text)? {
            return Ok(list);
        }
        // base64-список ссылок (3x-ui, Marzban с обычным клиентом).
        if let Some(decoded) = base64_decode(text).and_then(|b| String::from_utf8(b).ok()) {
            if decoded.lines().any(is_link) {
                return parse_lines(&decoded);
            }
        }
        return Err("Не похоже ни на ссылку, ни на подписку, ни на конфиг mihomo.".into());
    }
    parse_lines(text)
}

fn parse_lines(text: &str) -> Result<Vec<Value>, String> {
    let mut out = vec![];
    let mut first_err = None;
    for line in text.lines().map(str::trim).filter(|l| is_link(l)) {
        match parse_link(line) {
            Ok(p) => out.push(p),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    if out.is_empty() {
        return Err(first_err.unwrap_or_else(|| "Нет ни одного сервера.".into()));
    }
    Ok(dedupe_names(out))
}

/// `proxies:` из YAML или JSON. None — это не конфиг вовсе.
pub fn yaml_proxies(text: &str) -> Result<Option<Vec<Value>>, String> {
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(text) else { return Ok(None) };
    let Ok(doc) = serde_json::to_value(doc) else { return Ok(None) };
    let list = match &doc {
        Value::Object(o) if o.contains_key("proxies") => o["proxies"].as_array().cloned().unwrap_or_default(),
        _ => return Ok(None),
    };
    let proxies: Vec<Value> = list
        .into_iter()
        .filter(|p| p.get("type").and_then(Value::as_str).is_some() && p.get("server").is_some())
        .map(|mut p| {
            if p.get("name").and_then(Value::as_str).map(str::is_empty).unwrap_or(true) {
                let n = format!("{}:{}", p["server"].as_str().unwrap_or("?"), p.get("port").map(|v| v.to_string()).unwrap_or_default());
                p["name"] = n.into();
            }
            p
        })
        .collect();
    if proxies.is_empty() {
        return Err("В конфиге нет ни одного сервера.".into());
    }
    Ok(Some(dedupe_names(proxies)))
}

/// Имена прокси должны быть уникальны: по ним их выбирают в группе.
pub fn dedupe_names(list: Vec<Value>) -> Vec<Value> {
    let mut used: Vec<String> = vec![];
    list.into_iter()
        .map(|mut p| {
            let base = p.get("name").and_then(Value::as_str).unwrap_or("сервер").trim().to_string();
            let base = if base.is_empty() { "сервер".to_string() } else { base };
            let mut name = base.clone();
            let mut n = 2;
            while used.contains(&name) || name == "PROXY" || name == "DIRECT" || name == "REJECT" {
                name = format!("{base} ({n})");
                n += 1;
            }
            used.push(name.clone());
            p["name"] = name.into();
            p
        })
        .collect()
}

pub fn parse_link(link: &str) -> Result<Value, String> {
    let link = link.trim();
    let lower = link.to_ascii_lowercase();
    if lower.starts_with("vmess://") {
        return parse_vmess(link);
    }
    if lower.starts_with("ss://") {
        return parse_ss(link);
    }
    let u = Url::parse(link).ok_or("Ссылка не разобралась: нет адреса сервера.")?;
    match u.scheme.as_str() {
        "vless" => parse_vless(&u),
        "trojan" => parse_trojan(&u),
        "hysteria2" | "hy2" => parse_hy2(&u),
        "tuic" => parse_tuic(&u),
        "anytls" => parse_anytls(&u),
        other => Err(format!("Протокол {other}:// не поддерживается.")),
    }
}

fn base(kind: &str, u: &Url) -> Map<String, Value> {
    let mut m = Map::new();
    let name = if u.fragment.trim().is_empty() { format!("{}:{}", u.host, u.port) } else { u.fragment.trim().to_string() };
    m.insert("name".into(), name.into());
    m.insert("type".into(), kind.into());
    m.insert("server".into(), u.host.clone().into());
    m.insert("port".into(), u.port.into());
    m.insert("udp".into(), true.into());
    m
}

fn insecure(u: &Url) -> bool {
    matches!(u.q("allowInsecure").or_else(|| u.q("insecure")), Some("1") | Some("true"))
}

fn alpn(u: &Url) -> Option<Value> {
    let a: Vec<Value> = u.q("alpn")?.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.into()).collect();
    (!a.is_empty()).then(|| a.into())
}

/// TLS/Reality-поля vless и trojan. `sni_key` — у vless это servername, у trojan sni.
fn tls_fields(o: &mut Map<String, Value>, u: &Url, sni_key: &str) {
    if let Some(sni) = u.q("sni").or_else(|| u.q("peer")) {
        o.insert(sni_key.into(), sni.into());
    }
    o.insert("client-fingerprint".into(), u.q("fp").unwrap_or("chrome").into());
    if insecure(u) {
        o.insert("skip-cert-verify".into(), true.into());
    }
    if let Some(a) = alpn(u) {
        o.insert("alpn".into(), a);
    }
    if u.q("security") == Some("reality") {
        let mut r = Map::new();
        if let Some(pbk) = u.q("pbk") {
            r.insert("public-key".into(), pbk.into());
        }
        if let Some(sid) = u.q("sid") {
            r.insert("short-id".into(), sid.into());
        }
        o.insert("reality-opts".into(), Value::Object(r));
    }
}

fn transport(o: &mut Map<String, Value>, kind: Option<&str>, path: Option<&str>, host: Option<&str>, service: Option<&str>) -> Result<(), String> {
    let kind = kind.unwrap_or("tcp").to_ascii_lowercase();
    match kind.as_str() {
        "" | "tcp" | "raw" | "none" => {}
        "ws" => {
            o.insert("network".into(), "ws".into());
            let mut w = Map::new();
            if let Some(p) = path {
                let (p, ed) = split_early_data(p);
                w.insert("path".into(), p.into());
                if let Some(ed) = ed {
                    w.insert("max-early-data".into(), ed.into());
                    w.insert("early-data-header-name".into(), "Sec-WebSocket-Protocol".into());
                }
            }
            if let Some(h) = host {
                w.insert("headers".into(), json!({ "Host": h }));
            }
            o.insert("ws-opts".into(), Value::Object(w));
        }
        "grpc" => {
            o.insert("network".into(), "grpc".into());
            o.insert("grpc-opts".into(), json!({ "grpc-service-name": service.or(path).unwrap_or("") }));
        }
        "http" | "h2" => {
            o.insert("network".into(), "h2".into());
            let mut h = Map::new();
            if let Some(p) = path {
                h.insert("path".into(), p.into());
            }
            if let Some(hh) = host {
                h.insert("host".into(), json!([hh]));
            }
            o.insert("h2-opts".into(), Value::Object(h));
        }
        "httpupgrade" => {
            // В mihomo это ws с флагом v2ray-http-upgrade.
            o.insert("network".into(), "ws".into());
            let mut w = Map::new();
            w.insert("v2ray-http-upgrade".into(), true.into());
            if let Some(p) = path {
                w.insert("path".into(), p.into());
            }
            if let Some(h) = host {
                w.insert("headers".into(), json!({ "Host": h }));
            }
            o.insert("ws-opts".into(), Value::Object(w));
        }
        "xhttp" | "splithttp" => {
            o.insert("network".into(), "xhttp".into());
            let mut x = Map::new();
            if let Some(p) = path {
                x.insert("path".into(), p.into());
            }
            if let Some(h) = host {
                x.insert("host".into(), h.into());
            }
            o.insert("xhttp-opts".into(), Value::Object(x));
        }
        other => return Err(format!("Транспорт «{other}» не поддерживается.")),
    }
    Ok(())
}

fn parse_vless(u: &Url) -> Result<Value, String> {
    if u.user.is_empty() {
        return Err("В ссылке vless нет UUID.".into());
    }
    let mut o = base("vless", u);
    o.insert("uuid".into(), u.user.clone().into());
    if let Some(flow) = u.q("flow") {
        o.insert("flow".into(), flow.into());
    }
    if let Some(enc) = u.q("encryption").filter(|e| *e != "none") {
        o.insert("encryption".into(), enc.into());
    }
    if matches!(u.q("security"), Some("tls") | Some("reality")) {
        o.insert("tls".into(), true.into());
        tls_fields(&mut o, u, "servername");
    }
    transport(&mut o, u.q("type"), u.q("path"), u.q("host"), u.q("serviceName"))?;
    Ok(Value::Object(o))
}

fn parse_trojan(u: &Url) -> Result<Value, String> {
    if u.user.is_empty() {
        return Err("В ссылке trojan нет пароля.".into());
    }
    let mut o = base("trojan", u);
    o.insert("password".into(), u.user.clone().into());
    tls_fields(&mut o, u, "sni");
    transport(&mut o, u.q("type"), u.q("path"), u.q("host"), u.q("serviceName"))?;
    Ok(Value::Object(o))
}

fn parse_hy2(u: &Url) -> Result<Value, String> {
    let mut o = base("hysteria2", u);
    let pass = if u.pass.is_empty() { u.user.clone() } else { format!("{}:{}", u.user, u.pass) };
    o.insert("password".into(), pass.into());
    if let Some(sni) = u.q("sni") {
        o.insert("sni".into(), sni.into());
    }
    if insecure(u) {
        o.insert("skip-cert-verify".into(), true.into());
    }
    if let (Some(kind), Some(pw)) = (u.q("obfs"), u.q("obfs-password")) {
        o.insert("obfs".into(), kind.into());
        o.insert("obfs-password".into(), pw.into());
    }
    if let Some(ports) = u.q("mport") {
        o.insert("ports".into(), ports.into());
    }
    if let Some(a) = alpn(u) {
        o.insert("alpn".into(), a);
    }
    Ok(Value::Object(o))
}

fn parse_tuic(u: &Url) -> Result<Value, String> {
    let mut o = base("tuic", u);
    o.insert("uuid".into(), u.user.clone().into());
    o.insert("password".into(), u.pass.clone().into());
    if let Some(sni) = u.q("sni") {
        o.insert("sni".into(), sni.into());
    }
    if insecure(u) {
        o.insert("skip-cert-verify".into(), true.into());
    }
    o.insert("alpn".into(), alpn(u).unwrap_or_else(|| json!(["h3"])));
    if let Some(cc) = u.q("congestion_control") {
        o.insert("congestion-controller".into(), cc.into());
    }
    if let Some(m) = u.q("udp_relay_mode") {
        o.insert("udp-relay-mode".into(), m.into());
    }
    Ok(Value::Object(o))
}

fn parse_anytls(u: &Url) -> Result<Value, String> {
    let mut o = base("anytls", u);
    o.insert("password".into(), u.user.clone().into());
    tls_fields(&mut o, u, "sni");
    Ok(Value::Object(o))
}

/// vmess:// — base64 от JSON в формате v2rayN.
fn parse_vmess(link: &str) -> Result<Value, String> {
    let bytes = base64_decode(link["vmess://".len()..].trim()).ok_or("vmess: содержимое не в base64.")?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| "vmess: внутри не JSON.")?;
    let get = |k: &str| -> Option<String> {
        match v.get(k)? {
            Value::String(s) if !s.is_empty() => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    };
    let host = get("add").ok_or("vmess: нет адреса сервера.")?;
    let port: u16 = get("port").and_then(|p| p.parse().ok()).ok_or("vmess: неверный порт.")?;
    let mut o = Map::new();
    o.insert("name".into(), get("ps").unwrap_or_else(|| format!("{host}:{port}")).into());
    o.insert("type".into(), "vmess".into());
    o.insert("server".into(), host.into());
    o.insert("port".into(), port.into());
    o.insert("uuid".into(), get("id").ok_or("vmess: нет UUID.")?.into());
    o.insert("alterId".into(), get("aid").and_then(|a| a.parse::<u32>().ok()).unwrap_or(0).into());
    o.insert("cipher".into(), get("scy").unwrap_or_else(|| "auto".into()).into());
    o.insert("udp".into(), true.into());
    if get("tls").as_deref() == Some("tls") {
        o.insert("tls".into(), true.into());
        if let Some(sni) = get("sni").or_else(|| get("host")) {
            o.insert("servername".into(), sni.into());
        }
        if let Some(fp) = get("fp") {
            o.insert("client-fingerprint".into(), fp.into());
        }
    }
    let path = get("path");
    transport(&mut o, get("net").as_deref(), path.as_deref(), get("host").as_deref(), path.as_deref())?;
    Ok(Value::Object(o))
}

/// ss:// — SIP002 (base64 или url-encoded userinfo) и старый вид.
fn parse_ss(link: &str) -> Result<Value, String> {
    let mut rest = &link["ss://".len()..];
    let mut name = String::new();
    if let Some(i) = rest.find('#') {
        name = pct_decode(&rest[i + 1..]);
        rest = &rest[..i];
    }
    let mut plugin = None;
    if let Some(i) = rest.find('?') {
        plugin = query_pairs(&rest[i + 1..]).into_iter().find(|(k, _)| k == "plugin").map(|(_, v)| v);
        rest = &rest[..i];
    }
    let rest = rest.trim_end_matches('/');
    let (userinfo, hostport) = match rest.rfind('@') {
        Some(at) => {
            let ui = &rest[..at];
            let decoded = base64_decode(ui).and_then(|b| String::from_utf8(b).ok()).filter(|s| s.contains(':')).unwrap_or_else(|| pct_decode(ui));
            (decoded, rest[at + 1..].to_string())
        }
        None => {
            let all = base64_decode(rest).and_then(|b| String::from_utf8(b).ok()).ok_or("ss: ссылка не разобралась.")?;
            let at = all.rfind('@').ok_or("ss: нет адреса сервера.")?;
            (all[..at].to_string(), all[at + 1..].to_string())
        }
    };
    let (method, password) = userinfo.split_once(':').ok_or("ss: нет метода шифрования или пароля.")?;
    let (host, port) = split_host_port(&hostport).ok_or("ss: нет порта сервера.")?;
    if name.is_empty() {
        name = format!("{host}:{port}");
    }
    let mut o = json!({ "name": name, "type": "ss", "server": host, "port": port, "cipher": method, "password": password, "udp": true });
    if let Some(p) = plugin {
        // plugin=obfs-local;obfs=http;obfs-host=example.com
        let mut parts = p.split(';');
        let kind = parts.next().unwrap_or("");
        let opts: Vec<(&str, &str)> = parts.filter_map(|kv| kv.split_once('=')).collect();
        let get = |k: &str| opts.iter().find(|(a, _)| *a == k).map(|(_, b)| *b);
        match kind {
            "obfs-local" | "simple-obfs" => {
                o["plugin"] = "obfs".into();
                o["plugin-opts"] = json!({ "mode": get("obfs").unwrap_or("http"), "host": get("obfs-host").unwrap_or("") });
            }
            "v2ray-plugin" => {
                o["plugin"] = "v2ray-plugin".into();
                o["plugin-opts"] = json!({ "mode": "websocket", "host": get("host").unwrap_or(""), "path": get("path").unwrap_or("/"), "tls": opts.iter().any(|(k, _)| *k == "tls") });
            }
            other => return Err(format!("Плагин Shadowsocks «{other}» не поддерживается.")),
        }
    }
    Ok(o)
}

fn split_early_data(path: &str) -> (String, Option<u32>) {
    if let Some((p, q)) = path.split_once('?') {
        if let Some((_, v)) = query_pairs(q).into_iter().find(|(k, _)| k == "ed") {
            if let Ok(n) = v.parse() {
                return (p.to_string(), Some(n));
            }
        }
    }
    (path.to_string(), None)
}

/// Подпись протокола для списка серверов: «VLESS · Reality», «Trojan · gRPC».
pub fn proto_label(p: &Value) -> String {
    let kind = p.get("type").and_then(Value::as_str).unwrap_or("?");
    let name = match kind {
        "vless" => "VLESS",
        "vmess" => "VMess",
        "trojan" => "Trojan",
        "ss" => "Shadowsocks",
        "ssr" => "SSR",
        "hysteria2" => "Hysteria2",
        "hysteria" => "Hysteria",
        "tuic" => "TUIC",
        "anytls" => "AnyTLS",
        "wireguard" => "WireGuard",
        "socks5" => "SOCKS5",
        "http" => "HTTP",
        other => other,
    };
    let detail = if p.get("reality-opts").is_some() {
        Some("Reality")
    } else {
        match p.get("network").and_then(Value::as_str) {
            Some("ws") => Some("WS"),
            Some("grpc") => Some("gRPC"),
            Some("h2") | Some("http") => Some("H2"),
            Some("xhttp") => Some("XHTTP"),
            _ if p.get("tls").and_then(Value::as_bool) == Some(true) && kind == "vless" => Some("TLS"),
            _ => None,
        }
    };
    match detail {
        Some(d) => format!("{name} · {d}"),
        None => name.to_string(),
    }
}

/// Имя сайта из вставленного: «https://www.YouTube.com/watch?v=…» →
/// «www.youtube.com», «.ru» остаётся «.ru» (правило на всю зону).
pub fn normalize_site(input: &str) -> String {
    let mut s = input.trim();
    if let Some(i) = s.find("://") {
        s = &s[i + 3..];
    }
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    let s = s.rsplit('@').next().unwrap_or(s);
    let s = if s.matches(':').count() == 1 { s.split(':').next().unwrap_or("") } else { s };
    let zone = s.starts_with('.');
    let core = s.trim_start_matches(['*', '.']).trim_end_matches('.').to_lowercase();
    if core.is_empty() {
        String::new()
    } else if zone {
        format!(".{core}")
    } else {
        core
    }
}

// ─────────── Мини-разбор URL ───────────

#[derive(Default, Debug)]
struct Url {
    scheme: String,
    user: String,
    pass: String,
    host: String,
    port: u16,
    query: Vec<(String, String)>,
    fragment: String,
}

impl Url {
    fn parse(s: &str) -> Option<Url> {
        let (scheme, rest) = s.split_once("://")?;
        let (rest, fragment) = match rest.split_once('#') {
            Some((a, b)) => (a, pct_decode(b)),
            None => (rest, String::new()),
        };
        let (rest, query) = match rest.split_once('?') {
            Some((a, b)) => (a, query_pairs(b)),
            None => (rest, vec![]),
        };
        let rest = rest.split('/').next().unwrap_or("");
        let (userinfo, hostport) = match rest.rfind('@') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => ("", rest),
        };
        let (user, pass) = match userinfo.split_once(':') {
            Some((u, p)) => (pct_decode(u), pct_decode(p)),
            None => (pct_decode(userinfo), String::new()),
        };
        let (host, port) = split_host_port(hostport)?;
        Some(Url { scheme: scheme.to_ascii_lowercase(), user, pass, host, port, query, fragment })
    }

    fn q(&self, key: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
    }
}

fn split_host_port(s: &str) -> Option<(String, u16)> {
    let s = s.trim().trim_end_matches('/');
    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        let (h, p) = rest.split_once(']')?;
        (h.to_string(), p.strip_prefix(':')?)
    } else {
        let (h, p) = s.rsplit_once(':')?;
        (h.to_string(), p)
    };
    let port = port.parse().ok().filter(|p| *p > 0)?;
    (!host.is_empty()).then_some((host, port))
}

fn query_pairs(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (pct_decode(k), pct_decode(v)),
            None => (pct_decode(p), String::new()),
        })
        .collect()
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VLESS: &str = "vless://1b2c3d4e-0000-4000-8000-000000000001@203.0.113.5:443?type=tcp&security=reality&pbk=PUBKEY&fp=chrome&sni=www.vk.ru&sid=ab12&flow=xtls-rprx-vision#%D0%93%D0%B5%D1%80%D0%BC%D0%B0%D0%BD%D0%B8%D1%8F";

    #[test]
    fn vless_reality() {
        let p = parse_link(VLESS).unwrap();
        assert_eq!(p["name"], "Германия");
        assert_eq!(p["type"], "vless");
        assert_eq!(p["port"], 443);
        assert_eq!(p["tls"], true);
        assert_eq!(p["servername"], "www.vk.ru");
        assert_eq!(p["flow"], "xtls-rprx-vision");
        assert_eq!(p["reality-opts"]["public-key"], "PUBKEY");
        assert_eq!(p["reality-opts"]["short-id"], "ab12");
        assert_eq!(proto_label(&p), "VLESS · Reality");
    }

    #[test]
    fn vless_ws_и_grpc() {
        let p = parse_link("vless://id@h.example:8443?type=ws&path=%2Fws%3Fed%3D2048&host=cdn.example&security=tls").unwrap();
        assert_eq!(p["network"], "ws");
        assert_eq!(p["ws-opts"]["path"], "/ws");
        assert_eq!(p["ws-opts"]["max-early-data"], 2048);
        assert_eq!(p["ws-opts"]["headers"]["Host"], "cdn.example");
        let g = parse_link("trojan://pw@t.example:443?type=grpc&serviceName=svc&sni=t.example#T").unwrap();
        assert_eq!(g["grpc-opts"]["grpc-service-name"], "svc");
        assert_eq!(g["sni"], "t.example");
        assert_eq!(proto_label(&g), "Trojan · gRPC");
    }

    #[test]
    fn vmess_ss_hy2_tuic() {
        let js = r#"{"v":"2","ps":"NL","add":"nl.example","port":"443","id":"u","aid":"0","net":"ws","host":"nl.example","path":"/v","tls":"tls"}"#;
        let v = parse_link(&format!("vmess://{}", crate::sys::base64(js.as_bytes()))).unwrap();
        assert_eq!(v["type"], "vmess");
        assert_eq!(v["cipher"], "auto");
        assert_eq!(v["servername"], "nl.example");

        let ss = parse_link(&format!("ss://{}@1.2.3.4:8388#SS", crate::sys::base64(b"aes-256-gcm:pw"))).unwrap();
        assert_eq!(ss["cipher"], "aes-256-gcm");
        let ss2 = parse_link("ss://2022-blake3-aes-128-gcm:KEY%3D%3D@5.6.7.8:443#N").unwrap();
        assert_eq!(ss2["password"], "KEY==");
        let obfs = parse_link(&format!("ss://{}@1.2.3.4:8388?plugin=obfs-local%3Bobfs%3Dhttp%3Bobfs-host%3Dbing.com#O", crate::sys::base64(b"aes-128-gcm:p"))).unwrap();
        assert_eq!(obfs["plugin"], "obfs");
        assert_eq!(obfs["plugin-opts"]["host"], "bing.com");

        let hy = parse_link("hy2://secret@hy.example:8443?sni=hy.example&obfs=salamander&obfs-password=x&insecure=1#HY").unwrap();
        assert_eq!(hy["type"], "hysteria2");
        assert_eq!(hy["obfs"], "salamander");
        assert_eq!(hy["skip-cert-verify"], true);

        let t = parse_link("tuic://uuid:pass@tu.example:443?congestion_control=bbr&sni=tu.example#TU").unwrap();
        assert_eq!(t["password"], "pass");
        assert_eq!(t["congestion-controller"], "bbr");
    }

    #[test]
    fn yaml_подписка_mihomo() {
        let y = "proxies:\n  - {name: A, type: vless, server: 1.1.1.1, port: 443, uuid: u}\n  - name: A\n    type: trojan\n    server: 2.2.2.2\n    port: 443\n    password: p\nproxy-groups: []\n";
        let list = parse_any(y).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1]["name"], "A (2)");
    }

    #[test]
    fn base64_список_и_битые_строки() {
        let list = format!("{VLESS}\nvless://без-адреса\ntrojan://pw@t:443#T\n");
        assert_eq!(parse_any(&crate::sys::base64(list.as_bytes())).unwrap().len(), 2);
        assert!(parse_any("привет").is_err());
    }

    #[test]
    fn сайты() {
        assert_eq!(normalize_site("https://www.YouTube.com/watch?v=1"), "www.youtube.com");
        assert_eq!(normalize_site(".ru"), ".ru");
        assert_eq!(normalize_site("*.example.org:8080/x"), "example.org");
        assert_eq!(normalize_site("  "), "");
    }
}
