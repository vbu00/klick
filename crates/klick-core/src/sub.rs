//! Подписки: что вставил пользователь, что ответила панель, сколько там серверов.

use crate::convert;
use crate::model::SubInfo;
use base64::Engine as _;

/// Схемы одиночных ссылок, которые понимает ядро.
pub const LINK_SCHEMES: [&str; 16] = [
    "vless", "vmess", "trojan", "ss", "ssr", "socks", "socks5", "hysteria", "hysteria2", "hy2", "hysteria2+realm", "hy2+realm", "tuic",
    "wireguard", "wg", "anytls",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Subscription,
    Link,
}

/// Что вставили в поле «Добавить»: ссылку на подписку или одиночную конфигурацию.
pub fn detect_source(input: &str) -> Option<SourceKind> {
    let s = input.trim();
    let (scheme, rest) = s.split_once("://")?;
    // Пробел допустим только в имени после `#` (так подписывают ссылки некоторые программы); перенос строки — нет.
    let head = s.split_once('#').map_or(s, |(h, _)| h);
    if rest.is_empty() || s.contains(['\n', '\r']) || head.contains(char::is_whitespace) {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    // ssconf:// — динамический ключ Outline: по https лежит сервер Shadowsocks в JSON.
    if scheme == "http" || scheme == "https" || scheme == "ssconf" {
        Some(SourceKind::Subscription)
    } else if LINK_SCHEMES.contains(&scheme.as_str()) {
        Some(SourceKind::Link)
    } else {
        None
    }
}

/// Имя одиночной ссылки: то, что после `#`, иначе адрес сервера.
pub fn link_name(uri: &str) -> Option<String> {
    let uri = uri.trim();
    if let Some((_, fragment)) = uri.split_once('#') {
        let name = percent_decode(fragment);
        if !name.trim().is_empty() {
            return Some(name.trim().to_string());
        }
    }
    let rest = uri.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = match host_port.strip_prefix('[') {
        Some(v6) => v6.split(']').next()?,
        None => host_port.rsplit_once(':').map(|(h, _)| h).unwrap_or(host_port),
    };
    (!host.is_empty() && !host.contains('=')).then(|| host.to_string())
}

/// Срок позже 2090 года — это «без срока», а не «осталось 26 000 дней».
const NO_EXPIRY_AFTER: f64 = 3_786_912_000.0;

/// Заголовок `subscription-userinfo: upload=…; download=…; total=…; expire=…`.
pub fn parse_userinfo(header: &str) -> Option<SubInfo> {
    let mut info = SubInfo::default();
    let mut any = false;
    for part in header.split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        let v = v.trim();
        let num = v.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0);
        match (k.trim().to_ascii_lowercase().as_str(), num) {
            ("upload", Some(n)) => info.upload = n as u64,
            ("download", Some(n)) => info.download = n as u64,
            ("total", Some(n)) => info.total = n as u64,
            // Бессрочную подписку панели помечают по-разному: 0 или дата около 2100 года.
            ("expire", Some(n)) => info.expire = (n > 0.0 && n < NO_EXPIRY_AFTER).then_some(n as i64),
            _ => continue,
        }
        any = true;
    }
    any.then_some(info)
}

/// Заголовок `profile-update-interval` в часах.
pub fn parse_update_interval(header: &str) -> Option<u32> {
    header.trim().parse::<f64>().ok().filter(|h| *h >= 1.0 && *h <= 24.0 * 30.0).map(|h| h.round() as u32)
}

/// Заголовок `profile-title`: обычный текст или `base64:…`.
pub fn parse_title(header: &str) -> Option<String> {
    let h = header.trim();
    let title = match h.strip_prefix("base64:") {
        Some(b64) => String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()?).ok()?,
        None => h.to_string(),
    };
    let title = title.trim().to_string();
    (!title.is_empty()).then_some(title)
}

/// Объявление панели из заголовка `announce`: обычный текст или `base64:…`. Управляющие знаки
/// (кроме переноса строки) выбрасываем, длину режем: показываем это человеку как есть.
pub fn parse_announce(header: &str) -> Option<String> {
    let text: String = parse_title(header)?.chars().filter(|c| *c == '\n' || !c.is_control()).take(MAX_ANNOUNCE).collect();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

const MAX_ANNOUNCE: usize = 500;

/// Ссылка из заголовков `support-url` и `profile-web-page-url`: только `https://` с именем сайта и без пароля в адресе.
pub fn parse_https_link(header: &str) -> Option<String> {
    let h = header.trim();
    if h.len() > 2048 {
        return None;
    }
    let u = url::Url::parse(h).ok()?;
    (u.scheme() == "https" && u.host_str().is_some() && u.username().is_empty() && u.password().is_none()).then(|| u.to_string())
}

/// Имя файла из `content-disposition`, без расширения.
pub fn filename_from_disposition(header: &str) -> Option<String> {
    let mut plain = None;
    for part in header.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("filename*=") {
            let v = v.trim_matches('"');
            let v = v.split_once("''").map(|(_, rest)| rest).unwrap_or(v);
            return Some(strip_ext(&percent_decode(v)));
        }
        if let Some(v) = part.strip_prefix("filename=") {
            plain = Some(strip_ext(v.trim_matches('"')));
        }
    }
    plain.filter(|s| !s.is_empty())
}

fn strip_ext(name: &str) -> String {
    let name = name.trim();
    match name.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() && ext.len() <= 5 => base.to_string(),
        _ => name.to_string(),
    }
}

/// Адрес, по которому качать подписку: ключ Outline `ssconf://` лежит по `https://`.
pub fn fetch_url(source: &str) -> String {
    match source.trim().split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("ssconf") => format!("https://{rest}"),
        _ => source.trim().to_string(),
    }
}

/// Что лежит в ответе панели.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// Конфиг mihomo/Clash со списком `proxies` — YAML или JSON.
    ClashYaml,
    /// Чужой формат (sing-box, Xray, WireGuard, Shadowsocks): переводим в серверы mihomo; число — сколько.
    Converted(convert::Format, usize),
    /// Ссылки построчно; число — сколько их.
    Links(usize),
    /// Ссылки в base64.
    Base64Links(usize),
    /// Страница сайта вместо подписки: обычно неверная ссылка.
    Html,
    Unknown,
}

pub fn inspect_content(body: &str) -> Content {
    let t = body.trim_start_matches('\u{feff}').trim();
    let head: String = t.chars().take(200).collect::<String>().to_ascii_lowercase();
    if head.starts_with("<!doctype") || head.starts_with("<html") {
        return Content::Html;
    }
    if t.lines().any(|l| l.trim_start().starts_with("proxies:")) {
        return Content::ClashYaml;
    }
    // JSON-конфиг mihomo/Clash: ядро читает его как YAML (JSON — подмножество YAML).
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
        if v.get("proxies").and_then(|p| p.as_array()).is_some_and(|a| !a.is_empty()) {
            return Content::ClashYaml;
        }
    }
    if let Some(format) = convert::detect(t) {
        let n = convert::proxies(t, format).len();
        return if n > 0 { Content::Converted(format, n) } else { Content::Unknown };
    }
    let links = count_links(t);
    if links > 0 {
        return Content::Links(links);
    }
    match decode_links(t) {
        Some(text) => Content::Base64Links(count_links(&text)),
        None => Content::Unknown,
    }
}

/// Список ссылок, спрятанный в base64: расшифрованный текст, если в нём есть хоть одна ссылка.
fn decode_links(body: &str) -> Option<String> {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let text = base64_text(&compact)?;
    (count_links(&text) > 0).then_some(text)
}

/// Текст из base64 любого вида: с добивкой и без, обычный алфавит и URL-безопасный.
fn base64_text(s: &str) -> Option<String> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(s.trim_end_matches('=').as_bytes()))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s.trim_end_matches('=').as_bytes()));
    String::from_utf8(decoded.ok()?).ok()
}

/// Что положить в файл серверов ядра: чужой формат — переведённым, остальное — как есть.
/// Ссылки Hysteria в списках правим: конвертер ядра портит пароль со спецсимволами, теряет IPv6 с
/// прыжками по портам, `mport` и `ech`, оставляет серверу пустое имя, а один неверный сервер
/// (obfs без пароля, Hysteria 1 без скорости) роняет весь файл.
pub fn provider_body(body: &str, content: Content) -> String {
    match content {
        Content::Converted(format, _) => convert::provider_text(&convert::proxies(body, format)),
        Content::Links(_) => fix_hysteria(body).unwrap_or_else(|| body.to_string()),
        Content::Base64Links(_) => decode_links(body).and_then(|text| fix_hysteria(&text)).unwrap_or_else(|| body.to_string()),
        _ => body.to_string(),
    }
}

/// Файл серверов для одиночной ссылки: то, что служба кладёт в файл провайдера.
pub fn link_file(link: &str) -> String {
    let link = link.trim();
    fix_hysteria(link).unwrap_or_else(|| format!("{link}\n"))
}

/// Схема ссылки Hysteria, которую разбираем сами (`+realm` оставляем ядру).
fn is_hysteria(line: &str) -> bool {
    line.split_once("://").is_some_and(|(s, _)| ["hysteria", "hysteria2", "hy2"].iter().any(|h| s.eq_ignore_ascii_case(h)))
}

/// Новый текст файла, если в списке ссылок есть Hysteria и её надо поправить; иначе `None`.
/// Одни Hysteria — переводим в серверы mihomo сами. Вперемешку с другими схемами файл должен
/// остаться списком ссылок: там убираем негодные ссылки Hysteria и даём безымянным имя по адресу.
fn fix_hysteria(text: &str) -> Option<String> {
    let links: Vec<&str> = text.lines().map(str::trim).filter(|l| detect_source(l) == Some(SourceKind::Link)).collect();
    if !links.iter().any(|l| is_hysteria(l)) {
        return None;
    }
    if links.iter().all(|l| is_hysteria(l)) {
        let proxies = convert::hysteria_proxies(&links);
        return (!proxies.is_empty()).then(|| convert::provider_text(&proxies));
    }
    let mut changed = false;
    let mut out = String::new();
    for line in text.lines() {
        let l = line.trim();
        if is_hysteria(l) && detect_source(l) == Some(SourceKind::Link) {
            let Some(p) = convert::hysteria_link(l) else {
                changed = true;
                continue;
            };
            if l.split_once('#').is_none_or(|(_, name)| name.trim().is_empty()) {
                out.push_str(l.split_once('#').map_or(l, |(head, _)| head));
                out.push('#');
                out.push_str(p["server"].as_str().unwrap_or("Сервер"));
                out.push('\n');
                changed = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    changed.then_some(out)
}

// ── Ответ, в котором нет настоящих серверов ─────────────────────────────────

/// Что не так с ответом панели, хотя формат понятен. Такой ответ нельзя класть в файл провайдера:
/// он сотрёт рабочий список серверов.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unusable {
    /// Серверов нет совсем (`proxies: []`): у человека нет рабочей точки входа.
    NoServers,
    /// Панель отказала (лимит устройств, срок вышел, трафик кончился…) и вместо серверов отдала заглушку:
    /// адрес 0.0.0.0, 127.0.0.1 или `::`, порт 0 или 1. Причина — имя заглушки, если оно есть.
    Placeholder(Option<String>),
}

/// Пригоден ли ответ как список серверов. Не знаем формата или не смогли разобрать — `None`, то есть «пригоден».
/// Заглушкой считается ответ, где заглушкой является каждый сервер (так делают Mistgate, Remnawave, Marzban).
pub fn unusable(body: &str, content: Content) -> Option<Unusable> {
    let endpoints: Vec<Endpoint> = match content {
        Content::ClashYaml => {
            let t = body.trim_start_matches('\u{feff}').trim_start();
            if t.starts_with('{') {
                json_endpoints(&serde_json::from_str::<serde_json::Value>(t).ok()?)?
            } else {
                yaml_endpoints(t)?
            }
        }
        Content::Converted(format, _) => convert::proxies(body, format).iter().map(value_endpoint).collect(),
        Content::Links(_) => link_endpoints(body),
        Content::Base64Links(_) => link_endpoints(&decode_links(body)?),
        Content::Html | Content::Unknown => return None,
    };
    if endpoints.is_empty() {
        return Some(Unusable::NoServers);
    }
    endpoints.iter().all(Endpoint::is_placeholder).then(|| {
        Unusable::Placeholder(endpoints.iter().filter_map(|e| e.name.as_deref()).map(sanitize_reason).find(|r| !r.is_empty()))
    })
}

/// Причина отказа в одну строку: без управляющих знаков и не длиннее 200 знаков.
pub fn sanitize_reason(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(200).collect::<String>().trim().to_string()
}

/// Куда ведёт сервер подписки: всё, что нужно, чтобы узнать заглушку.
#[derive(Default)]
struct Endpoint {
    name: Option<String>,
    host: Option<String>,
    port: Option<u32>,
}

impl Endpoint {
    fn set(&mut self, key: &str, value: String) {
        match key {
            "name" => self.name = Some(value),
            "server" => self.host = Some(value),
            "port" => self.port = value.trim().parse().ok(),
            _ => {}
        }
    }

    fn is_placeholder(&self) -> bool {
        let host = self.host.as_deref().map(|h| h.trim().trim_matches(['[', ']']).to_ascii_lowercase());
        host.is_some_and(|h| matches!(h.as_str(), "0.0.0.0" | "127.0.0.1" | "::" | "::1" | "localhost")) || self.port.is_some_and(|p| p <= 1)
    }
}

fn value_endpoint(p: &serde_json::Value) -> Endpoint {
    let text = |key: &str| p.get(key).and_then(|v| v.as_str()).map(str::to_string);
    Endpoint { name: text("name"), host: text("server"), port: port_of(p.get("port")) }
}

fn port_of(v: Option<&serde_json::Value>) -> Option<u32> {
    let v = v?;
    v.as_u64().and_then(|n| u32::try_from(n).ok()).or_else(|| v.as_str()?.trim().parse().ok())
}

fn json_endpoints(v: &serde_json::Value) -> Option<Vec<Endpoint>> {
    Some(v.get("proxies")?.as_array()?.iter().map(value_endpoint).collect())
}

/// Секция `proxies:` блочного YAML (и элементы в фигурных скобках `- {name: a, server: b}`): по одной записи на сервер.
/// Читаем только ключи самого элемента (на его отступе), вложенные `reality-opts:` и прочее не смотрим.
/// `None` — секции нет или она записана одной строкой `proxies: [ … ]`: тогда не знаем, не трогаем.
fn yaml_endpoints(body: &str) -> Option<Vec<Endpoint>> {
    let mut lines = body.lines();
    let head = lines.by_ref().find(|l| l.starts_with("proxies:"))?;
    let rest = head["proxies:".len()..].trim();
    if rest == "[]" {
        return Some(Vec::new());
    }
    if !rest.is_empty() && !rest.starts_with('#') {
        return None;
    }
    let mut out: Vec<Endpoint> = Vec::new();
    let (mut dash, mut keys) = (None::<usize>, 0usize);
    for l in lines {
        let t = l.trim_start();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let indent = l.len() - t.len();
        let is_item = t.starts_with("- ");
        if indent == 0 && !is_item && t != "-" {
            break; // следующая секция файла
        }
        if is_item && dash.map_or(true, |d| indent <= d) {
            dash = Some(indent);
            let item = t[2..].trim_start();
            keys = indent + (t.len() - item.len());
            let mut e = Endpoint::default();
            if item.starts_with('{') {
                for (k, v) in flow_pairs(item) {
                    e.set(&k, v);
                }
            } else if let Some((k, v)) = item.split_once(':') {
                e.set(k.trim(), unquote(v));
            }
            out.push(e);
        } else if !is_item && indent == keys {
            if let (Some(e), Some((k, v))) = (out.last_mut(), t.split_once(':')) {
                e.set(k.trim(), unquote(v));
            }
        }
    }
    Some(out)
}

fn flow_pairs(item: &str) -> Vec<(String, String)> {
    let inner = item.trim().trim_start_matches('{').trim_end().trim_end_matches('}');
    split_top(inner).into_iter().filter_map(|part| part.split_once(':').map(|(k, v)| (unquote(k), unquote(v)))).collect()
}

/// Делит по запятым вне кавычек и вложенных скобок.
fn split_top(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut quote, mut escaped, mut start) = (Vec::new(), 0i32, None::<char>, false, 0usize);
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' && q == '"' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Значение YAML: в кавычках (`"…"` с `\"`, `'…'`) или простое, без хвостового комментария.
fn unquote(s: &str) -> String {
    let s = s.trim();
    match s.chars().next() {
        Some(q @ ('"' | '\'')) => {
            let mut out = String::new();
            let mut chars = s[1..].chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    // Экранирование YAML в двойных кавычках: Mistgate пишет флаги как \U0001F1E9\U0001F1EA.
                    '\\' if q == '"' => match chars.next() {
                        Some(k @ ('x' | 'u' | 'U')) => {
                            let len = match k { 'x' => 2, 'u' => 4, _ => 8 };
                            let hex: String = (0..len).filter_map(|_| chars.next()).collect();
                            out.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
                        }
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some('r') => out.push('\r'),
                        Some('0') => out.push('\0'),
                        Some(other) => out.push(other),
                        None => {}
                    },
                    // '' внутри одинарных кавычек — сама кавычка.
                    '\'' if q == '\'' && chars.peek() == Some(&'\'') => {
                        chars.next();
                        out.push('\'');
                    }
                    c if c == q => break,
                    c => out.push(c),
                }
            }
            out
        }
        _ => s.split(" #").next().unwrap_or("").trim().to_string(),
    }
}

fn link_endpoints(text: &str) -> Vec<Endpoint> {
    text.lines().map(str::trim).filter(|l| detect_source(l) == Some(SourceKind::Link)).map(link_endpoint).collect()
}

/// Имя, адрес и порт ссылки. Не вышло разобрать — пустая запись («не заглушка»).
fn link_endpoint(line: &str) -> Endpoint {
    let mut e = Endpoint::default();
    let Some((scheme, rest)) = line.split_once("://") else { return e };
    let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    e.name = Some(percent_decode(fragment)).filter(|n| !n.trim().is_empty());
    let scheme = scheme.to_ascii_lowercase();
    if scheme == "vmess" {
        // vmess://base64(JSON): add, port, ps.
        let Some(v) = base64_text(rest.trim()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else { return e };
        let text = |key: &str| v.get(key).and_then(|x| x.as_str()).map(str::to_string);
        e.name = e.name.or_else(|| text("ps"));
        e.host = text("add");
        e.port = port_of(v.get("port"));
        return e;
    }
    let mut authority = rest.split(['/', '?']).next().unwrap_or("").to_string();
    // Старый вид Shadowsocks: ss://base64(метод:пароль@адрес:порт).
    if scheme == "ss" && !authority.contains('@') {
        if let Some(plain) = base64_text(&authority) {
            authority = plain;
        }
    }
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let (host, port) = match host_port.strip_prefix('[') {
        Some(v6) => {
            let (h, tail) = v6.split_once(']').unwrap_or((v6, ""));
            (h, tail.strip_prefix(':'))
        }
        None => host_port.rsplit_once(':').map_or((host_port, None), |(h, p)| (h, Some(p))),
    };
    e.host = Some(host.to_string());
    // Порт Hysteria бывает со списком `443,20000-30000`: берём первое число.
    e.port = port.and_then(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok());
    e
}

fn count_links(text: &str) -> usize {
    text.lines().filter(|l| detect_source(l) == Some(SourceKind::Link)).count()
}

pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()))
            .flatten();
        match hex {
            Some(b) => {
                out.push(b);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_detected() {
        assert_eq!(detect_source("https://panel.example.com/sub/abc"), Some(SourceKind::Subscription));
        assert_eq!(detect_source("vless://uuid@host:443?security=reality#NL"), Some(SourceKind::Link));
        assert_eq!(detect_source("HYSTERIA2://pass@host:443"), Some(SourceKind::Link));
        assert_eq!(detect_source("ftp://example.com"), None);
        assert_eq!(detect_source("просто текст"), None);
    }

    #[test]
    fn link_names() {
        assert_eq!(link_name("vless://id@nl.example.com:443?x=1#%F0%9F%87%B3%F0%9F%87%B1%20Amsterdam").unwrap(), "🇳🇱 Amsterdam");
        assert_eq!(link_name("trojan://pass@de.example.com:443").unwrap(), "de.example.com");
        assert_eq!(link_name("hysteria2://pass@[2001:db8::1]:443").unwrap(), "2001:db8::1");
    }

    #[test]
    fn hysteria_links_are_detected_and_named() {
        for s in [
            "hy2://pw@host:443,20000-30000/?sni=a.b#Hop",
            "Hy2://user:pa:ss@host:20000-30000",
            "hysteria2://pw@[2001:db8::1]:443,20000-30000/?obfs=salamander&obfs-password=x#%F0%9F%87%A9%F0%9F%87%AA",
            "hysteria://host:443?auth=a&upmbps=1&downmbps=1&mport=1000-2000",
            "hysteria2+realm://token@rendezvous.example.com/room?auth=pw",
            "hy2://pw@host:443#🇩🇪 Германия, Франкфурт",
        ] {
            assert_eq!(detect_source(s), Some(SourceKind::Link), "{s}");
        }
        // Пробел до `#` и перенос строки — по-прежнему не одна ссылка.
        assert_eq!(detect_source("hy2://pw@host:443 #x"), None);
        assert_eq!(detect_source("hy2://pw@host:443#a\nhy2://pw@host2:443#b"), None);
        assert_eq!(link_name("hy2://pw@host:443,20000-30000/?sni=a.b").unwrap(), "host");
        assert_eq!(link_name("hy2://user:pa:ss@host:20000-30000").unwrap(), "host");
        assert_eq!(link_name("hysteria2://pw@[2001:db8::1]:443,20000-30000#").unwrap(), "2001:db8::1");
        assert_eq!(link_name("hy2://pw@host:443#%F0%9F%87%A9%F0%9F%87%AA%20DE").unwrap(), "🇩🇪 DE");
        assert_eq!(link_name("hy2://pw@host:443#🇩🇪 Германия, Франкфурт").unwrap(), "🇩🇪 Германия, Франкфурт");
        assert_eq!(count_links("hy2://pw@a:443,2000-3000#x\nhysteria2://pw@[::1]:443,2000-3000#y y\n# комментарий\nvless://u@b:1#z"), 3);
    }

    #[test]
    fn hysteria_only_list_is_translated() {
        let links = "hy2://user:p%40ss@h1.example.com:443,20000-30000/?sni=a.b#DE\n\
                     hysteria2://pw@[2001:db8::1]:443,20000-30000#v6\n\
                     hysteria2://pw@h3.example.com/?obfs=salamander#broken\n\
                     hysteria://h4.example.com:443?auth=x&upmbps=10&downmbps=50#v1\n";
        let kind = inspect_content(links);
        assert_eq!(kind, Content::Links(4));
        let body = provider_body(links, kind);
        let v: serde_json::Value = serde_json::from_str(&body).expect("файл серверов — JSON");
        let list = v["proxies"].as_array().unwrap();
        assert_eq!(list.len(), 3, "негодную ссылку выбросили, остальные живы: {body}");
        assert_eq!(list[0]["password"], "user:p@ss");
        assert_eq!(list[0]["ports"], "443,20000-30000");
        assert_eq!(list[1]["server"], "2001:db8::1");
        assert_eq!(list[2]["type"], "hysteria");
        // В base64 то же самое.
        let b64 = base64::engine::general_purpose::STANDARD.encode(links);
        let kind = inspect_content(&b64);
        assert_eq!(kind, Content::Base64Links(4));
        assert_eq!(provider_body(&b64, kind), body);
    }

    #[test]
    fn hysteria_in_mixed_list_stays_a_list() {
        let links = "vless://u@v.example.com:443?security=tls#VL\nhy2://pw@h.example.com:443\nhy2://pw@h2.example.com:443/?obfs=salamander#broken\nhy2://pw@h3.example.com:443#ok\nhysteria2+realm://t@r.example.com/room?auth=x\n";
        let body = provider_body(links, inspect_content(links));
        assert_eq!(
            body,
            "vless://u@v.example.com:443?security=tls#VL\nhy2://pw@h.example.com:443#h.example.com\nhy2://pw@h3.example.com:443#ok\nhysteria2+realm://t@r.example.com/room?auth=x\n"
        );
        // Без Hysteria и без нужды что-либо менять список не трогаем.
        let plain = "vless://a@b:1#x\ntrojan://c@d:2#y\n";
        assert_eq!(provider_body(plain, inspect_content(plain)), plain);
        let fine = "vless://a@b:1#x\nhy2://pw@h:443#ok\n";
        assert_eq!(provider_body(fine, inspect_content(fine)), fine);
    }

    #[test]
    fn single_link_file() {
        assert_eq!(link_file(" vless://a@b:1#x "), "vless://a@b:1#x\n");
        let body = link_file("hy2://pw@host:443,20000-30000/?sni=a.b");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!((v["proxies"][0]["name"].as_str(), v["proxies"][0]["ports"].as_str()), (Some("host"), Some("443,20000-30000")));
        // Негодную ссылку отдаём как есть: ядро сообщит об ошибке само.
        assert_eq!(link_file("hy2://pw@host:443/?obfs=salamander"), "hy2://pw@host:443/?obfs=salamander\n");
    }

    #[test]
    fn userinfo_is_parsed() {
        let info = parse_userinfo("upload=1024; download=2048; total=214748364800; expire=1795000000").unwrap();
        assert_eq!(info.upload, 1024);
        assert_eq!(info.download, 2048);
        assert_eq!(info.total, 214748364800);
        assert_eq!(info.expire, Some(1795000000));
        let unlimited = parse_userinfo("upload=0;download=10.5;total=0;expire=0").unwrap();
        assert_eq!(unlimited.expire, None);
        // Remnawave и другие панели: бессрочная подписка — дата около 2100 года.
        assert_eq!(parse_userinfo("upload=0; download=0; total=0; expire=4110000000").unwrap().expire, None);
        assert_eq!(unlimited.download, 10);
        assert_eq!(parse_userinfo("garbage"), None);
    }

    #[test]
    fn titles_and_filenames() {
        assert_eq!(parse_title("base64:UmVtbmF3YXZlIMK3IEFsZXg=").unwrap(), "Remnawave · Alex");
        assert_eq!(parse_title("My VPN").unwrap(), "My VPN");
        assert_eq!(filename_from_disposition("attachment; filename=\"alex.yaml\"").unwrap(), "alex");
        assert_eq!(filename_from_disposition("attachment; filename*=UTF-8''%D0%9C%D0%BE%D0%B9.yaml").unwrap(), "Мой");
        assert_eq!(parse_update_interval("12"), Some(12));
        assert_eq!(parse_update_interval("0"), None);
    }

    #[test]
    fn content_kinds() {
        assert_eq!(inspect_content("proxies:\n  - name: a\n"), Content::ClashYaml);
        assert_eq!(inspect_content("vless://a@b:1#x\ntrojan://c@d:2#y\n"), Content::Links(2));
        let b64 = base64::engine::general_purpose::STANDARD.encode("vless://a@b:1#x\nss://Y2hhY2hhMjA6cGFzcw@e:3#z");
        assert_eq!(inspect_content(&b64), Content::Base64Links(2));
        assert_eq!(inspect_content("<!DOCTYPE html><html>"), Content::Html);
        assert_eq!(inspect_content("hello"), Content::Unknown);
        let json = r#"{"proxies": [{"name": "a", "type": "ss", "server": "e", "port": 3, "cipher": "aes-128-gcm", "password": "p"}]}"#;
        assert_eq!(inspect_content(json), Content::ClashYaml);
        assert_eq!(provider_body(json, Content::ClashYaml), json);
        let sb = r#"{"outbounds": [{"type": "trojan", "tag": "t", "server": "e", "server_port": 443, "password": "p"}, {"type": "direct", "tag": "d"}]}"#;
        let kind = inspect_content(sb);
        assert_eq!(kind, Content::Converted(convert::Format::SingBox, 1));
        assert!(provider_body(sb, kind).contains("\"proxies\""));
        assert_eq!(inspect_content(r#"{"outbounds": [{"type": "direct"}]}"#), Content::Unknown);
    }

    #[test]
    fn outline_key() {
        assert_eq!(detect_source("ssconf://example.com/key.json"), Some(SourceKind::Subscription));
        assert_eq!(fetch_url("ssconf://example.com/key.json"), "https://example.com/key.json");
        assert_eq!(fetch_url("https://example.com/sub"), "https://example.com/sub");
    }
}

/// Совместимость с панелью Mistgate и общее распознавание «ответа без серверов».
/// Ответы сняты настоящим обработчиком подписки Mistgate по HTTP (`User-Agent: mihomo/1.19.31`, заголовки X-HWID… как у kl!ck);
/// токены, ключи и адреса в них тестовые (`example.com`, `203.0.113.x`).
#[cfg(test)]
mod mistgate_compat {
    use super::*;
    use crate::convert::Format;

    #[test]
    fn yaml_escapes_in_quoted_names() {
        // Так Mistgate пишет имена серверов с флагами.
        assert_eq!(unquote(r#""\U0001F1E9\U0001F1EA DE · VLESS · TCP""#), "🇩🇪 DE · VLESS · TCP");
        assert_eq!(unquote(r#""A\x42 \"q\" a\\b""#), "AB \"q\" a\\b");
        assert_eq!(unquote("'it''s'"), "it's");
        assert_eq!(unquote("plain # comment"), "plain");
    }

    macro_rules! fx {
        ($f:expr) => {
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mistgate/", $f))
        };
    }

    /// Значение заголовка из снятого файла `*.headers.txt` («Имя: значение»).
    fn header<'a>(saved: &'a str, name: &str) -> Option<&'a str> {
        saved.lines().find_map(|l| {
            let (k, v) = l.split_once(": ")?;
            k.eq_ignore_ascii_case(name).then_some(v)
        })
    }

    fn verdict(body: &str) -> Option<Unusable> {
        unusable(body, inspect_content(body))
    }

    /// (имя, ответ, заголовки, причина отказа)
    const REFUSALS: [(&str, &str, &str, &str); 7] = [
        ("limit_second_device_klick", fx!("limit_second_device_klick.body"), fx!("limit_second_device_klick.headers.txt"), "Device limit: 1 of 1. Remove one you no longer use on the subscription page"),
        ("limitq_second_device_klick", fx!("limitq_second_device_klick.body"), fx!("limitq_second_device_klick.headers.txt"), "Device limit: 1 of 1. Remove one you no longer use on the subscription page"),
        ("expired_klick", fx!("expired_klick.body"), fx!("expired_klick.headers.txt"), "Subscription ended on 20 October. Message us to renew"),
        ("quota_exhausted_klick", fx!("quota_exhausted_klick.body"), fx!("quota_exhausted_klick.headers.txt"), "Traffic used up, resets on 1 November"),
        ("disabled_klick", fx!("disabled_klick.body"), fx!("disabled_klick.headers.txt"), "Access paused"),
        ("blocked_after_klick", fx!("blocked_after_klick.body"), fx!("blocked_after_klick.headers.txt"), "This device was turned off by the owner"),
        ("strict_nohwid_klick", fx!("strict_nohwid_klick.body"), fx!("strict_nohwid_klick.headers.txt"), "This app does not report a device ID. Turn on HWID in the settings of Happ, or use kl!ck"),
    ];

    #[test]
    fn a_normal_answer_has_seven_servers_and_is_accepted() {
        let body = fx!("active_klick.body");
        assert_eq!(inspect_content(body), Content::ClashYaml);
        // Три протокола Mistgate: hysteria2 (salamander, gecko, hop), vless (reality tcp/xhttp), wireguard (AmneziaWG 2.0 и 3.1).
        for key in ["type: hysteria2", "type: vless", "type: wireguard", "amnezia-wg-option:", "reality-opts:", "obfs: gecko", "hop-interval:", "network: xhttp"] {
            assert!(body.contains(key), "в ответе нет {key}");
        }
        let servers = yaml_endpoints(body).unwrap();
        let names: Vec<_> = servers.iter().map(|e| e.name.as_deref().unwrap()).collect();
        assert_eq!(names, ["hy2-gecko", "hy2-salamander-hop", "p", "vless-tcp-vision", "vless-xhttp", "awg20", "awg31"]);
        // Вложенные списки и ключи (dns, allowed-ips, reality-opts) за сервер не считаются.
        assert_eq!((servers[6].host.as_deref(), servers[6].port), (Some("de1.example.com"), Some(13526)));
        assert_eq!(servers[1].port, Some(8444), "`ports:` не путается с `port:`");
        assert!(servers.iter().all(|e| !e.is_placeholder()));
        for answer in [body, fx!("unlimited_klick.body"), fx!("unicode_title_klick.body"), fx!("golden_all_variants.body")] {
            assert_eq!(inspect_content(answer), Content::ClashYaml);
            assert_eq!(verdict(answer), None);
            assert_eq!(provider_body(answer, Content::ClashYaml), answer, "kl!ck кладёт ответ в файл провайдера как есть");
        }
        // «Все варианты»: 16 серверов, элементы списка на уровне ключа `proxies:`, у одного в названии кавычка с переводом строки.
        assert_eq!(yaml_endpoints(fx!("golden_all_variants.body")).unwrap().len(), 16);
    }

    #[test]
    fn an_empty_proxy_list_is_no_servers() {
        let body = fx!("no_servers_klick.body");
        assert_eq!(inspect_content(body), Content::ClashYaml);
        assert_eq!(verdict(body), Some(Unusable::NoServers));
        assert_eq!(verdict("proxies: []\n"), Some(Unusable::NoServers));
        assert_eq!(verdict("proxies:\nproxy-groups:\n  - name: x\n"), Some(Unusable::NoServers));
        assert_eq!(verdict("proxies:\r\n  # пока пусто\r\nrules: []\r\n"), Some(Unusable::NoServers));
    }

    #[test]
    fn placeholders_of_the_panel_are_refusals_with_their_reason() {
        for (name, body, _, reason) in REFUSALS {
            assert_eq!(inspect_content(body), Content::ClashYaml, "{name}");
            assert_eq!(verdict(body), Some(Unusable::Placeholder(Some(reason.to_string()))), "{name}");
        }
    }

    #[test]
    fn placeholder_detection_is_not_tied_to_one_panel() {
        let yaml = |servers: &str| format!("proxies:\n{servers}proxy-groups:\n  - name: g\n    type: select\n");
        let reason = |v: Option<Unusable>| match v {
            Some(Unusable::Placeholder(r)) => r,
            other => panic!("ждали заглушку, а {other:?}"),
        };
        // Адрес 0.0.0.0, 127.0.0.1, `::` и порт 0 или 1 с настоящим адресом — всё заглушки.
        for (host, port) in [("0.0.0.0", 443), ("127.0.0.1", 8080), ("::", 443), ("::1", 443), ("localhost", 443), ("nl.example.com", 1), ("nl.example.com", 0)] {
            let body = yaml(&format!("  - name: \"Срок вышел\"\n    type: vless\n    server: \"{host}\"\n    port: {port}\n"));
            assert_eq!(reason(verdict(&body)).as_deref(), Some("Срок вышел"), "{host}:{port}");
        }
        // Remnawave и Marzban с несколькими заглушками подряд; поток в одну строку; кавычки; запятая в названии.
        let body = yaml("  - {name: \"⚠️ Лимит, 3 из 3\", type: vless, server: 0.0.0.0, port: 1}\n  - {name: 'второй', type: trojan, server: '127.0.0.1', port: '0'}\n");
        assert_eq!(reason(verdict(&body)).as_deref(), Some("⚠️ Лимит, 3 из 3"));
        // Поток вложенных ключей `- {name: a, opts: {host: b, port: 2}, server: …}` не путает порты.
        let body = yaml("  - {name: a, ws-opts: {headers: {Host: x}, port: 1}, server: nl.example.com, port: 443}\n");
        assert_eq!(verdict(&body), None);
        // Настоящий сервер рядом с заглушкой-«счётчиком дней» (так делают некоторые панели) — подписка рабочая.
        let body = yaml("  - name: \"⏳ 20 дней\"\n    server: 0.0.0.0\n    port: 1\n  - name: NL\n    server: nl.example.com\n    port: 443\n");
        assert_eq!(verdict(&body), None);
        // Запись без адреса — не знаем, значит не заглушка.
        assert_eq!(verdict(&yaml("  - name: a\n    type: ss\n")), None);
        // JSON-вид и формат sing-box.
        assert_eq!(
            reason(verdict(r#"{"proxies": [{"name": "Срок вышел", "type": "ss", "server": "0.0.0.0", "port": 1, "cipher": "aes-128-gcm", "password": "p"}]}"#)).as_deref(),
            Some("Срок вышел")
        );
        let sb = r#"{"outbounds": [{"type": "trojan", "tag": "Лимит", "server": "127.0.0.1", "server_port": 1, "password": "p"}]}"#;
        assert_eq!(inspect_content(sb), Content::Converted(Format::SingBox, 1));
        assert_eq!(reason(verdict(sb)).as_deref(), Some("Лимит"));
        assert_eq!(verdict(fx!("active_singbox.body")), None);
        assert_eq!(verdict(fx!("active_klick_xrayjson.body")), None);
    }

    #[test]
    fn placeholder_links_in_lists_and_base64() {
        let vmess = base64::engine::general_purpose::STANDARD.encode(r#"{"v":"2","ps":"vmess-отказ","add":"0.0.0.0","port":"1","id":"u"}"#);
        let ss_old = base64::engine::general_purpose::STANDARD.encode("aes-128-gcm:pw@127.0.0.1:1");
        let list = format!(
            "vless://00000000-0000-0000-0000-000000000000@0.0.0.0:1?security=none&type=tcp#%E2%9A%A0%EF%B8%8F%20Device%20limit\n\
             trojan://pw@[::]:1#second\n\
             hy2://pw@127.0.0.1:1,20000-30000/?sni=a.b\n\
             vmess://{vmess}\n\
             ss://{ss_old}#old\n"
        );
        assert_eq!(inspect_content(&list), Content::Links(5));
        assert_eq!(verdict(&list), Some(Unusable::Placeholder(Some("⚠️ Device limit".into()))));
        let b64 = base64::engine::general_purpose::STANDARD.encode(&list);
        assert_eq!(inspect_content(&b64), Content::Base64Links(5));
        assert_eq!(verdict(&b64), Some(Unusable::Placeholder(Some("⚠️ Device limit".into()))));
        // Без имени причины нет, но заглушка всё равно заглушка.
        assert_eq!(verdict("vless://u@0.0.0.0:1?type=tcp\n"), Some(Unusable::Placeholder(None)));
        // Настоящий сервер в списке — всё хорошо.
        let mixed = format!("{list}vless://u@nl.example.com:443?security=tls#NL\n");
        assert_eq!(verdict(&mixed), None);
        let real = format!("hy2://pw@nl.example.com:443,20000-30000/?sni=a.b\nvmess://{}\n", base64::engine::general_purpose::STANDARD.encode(r#"{"ps":"x","add":"de.example.com","port":443}"#));
        assert_eq!(verdict(&real), None);
        // Настоящие ответы Mistgate в виде списка ссылок.
        let curl = fx!("active_curl.body");
        assert_eq!(inspect_content(curl), Content::Base64Links(4));
        assert_eq!(verdict(curl), None);
    }

    #[test]
    fn headers_of_a_normal_answer() {
        let h = fx!("active_klick.headers.txt");
        let info = parse_userinfo(header(h, "subscription-userinfo").unwrap()).unwrap();
        assert_eq!((info.upload, info.download, info.total), (0, 0, 50 << 30));
        assert!(info.expire.is_some_and(|e| e > 1_700_000_000));
        assert_eq!(parse_update_interval(header(h, "profile-update-interval").unwrap()), Some(12));
        assert_eq!(parse_title(header(h, "profile-title").unwrap()).as_deref(), Some("Mistgate"));
        assert_eq!(filename_from_disposition(header(h, "content-disposition").unwrap()).as_deref(), Some("Mistgate"));
        // Страница подписки и отсутствие объявления в обычном ответе.
        assert_eq!(parse_https_link(header(h, "profile-web-page-url").unwrap()).unwrap().split('/').nth(2), Some("sub.example.com"));
        assert_eq!(header(h, "announce"), None);
        assert_eq!(header(h, "support-url"), None);
    }

    #[test]
    fn unlimited_user_has_no_expiry_and_no_quota() {
        let info = parse_userinfo(header(fx!("unlimited_klick.headers.txt"), "subscription-userinfo").unwrap()).unwrap();
        assert_eq!((info.total, info.expire), (0, None));
    }

    #[test]
    fn unicode_title_comes_through_base64() {
        let h = fx!("unicode_title_klick.headers.txt");
        assert_eq!(parse_title(header(h, "profile-title").unwrap()).as_deref(), Some("Мой VPN 50% (тест) 🐈.pro"));
        assert_eq!(parse_update_interval(header(h, "profile-update-interval").unwrap()), Some(6));
        // Имя файла из Content-Disposition теряет «.pro» как расширение — запасной путь, до него доходит редко.
        assert_eq!(filename_from_disposition(header(h, "content-disposition").unwrap()).as_deref(), Some("Мой VPN 50% (тест) 🐈"));
    }

    #[test]
    fn refusal_headers_are_read() {
        for (name, _, h, reason) in REFUSALS {
            // Панель просит заходить раз в час; причина в `announce` совпадает с именем заглушки.
            assert_eq!(parse_update_interval(header(h, "profile-update-interval").unwrap()), Some(1), "{name}");
            assert_eq!(parse_announce(header(h, "announce").unwrap()).as_deref(), Some(reason), "{name}");
            assert!(parse_https_link(header(h, "profile-web-page-url").unwrap()).is_some(), "{name}");
        }
        // Лимит устройств не стирает у человека срок и квоту.
        let info = parse_userinfo(header(fx!("limitq_second_device_klick.headers.txt"), "subscription-userinfo").unwrap()).unwrap();
        assert_eq!(info.total, 10 << 30);
        assert!(info.expire.is_some());
        // Квота исчерпана: upload+download >= total.
        let info = parse_userinfo(header(fx!("quota_exhausted_klick.headers.txt"), "subscription-userinfo").unwrap()).unwrap();
        assert!(info.total > 0 && info.upload + info.download >= info.total);
    }

    #[test]
    fn announce_and_links_are_cleaned() {
        assert_eq!(parse_announce("Продлите до 1 ноября").as_deref(), Some("Продлите до 1 ноября"));
        assert_eq!(parse_announce("base64:0J/RgNC+0LTQu9C40YLQtQ==").as_deref(), Some("Продлите"));
        assert_eq!(parse_announce("  ").as_deref(), None);
        assert_eq!(parse_announce("base64:***"), None);
        assert_eq!(parse_announce("a\u{7}b\nc\u{1b}[31m").as_deref(), Some("ab\nc[31m"));
        assert_eq!(parse_announce(&"я".repeat(900)).unwrap().chars().count(), 500);
        assert_eq!(parse_https_link(" https://t.me/support ").as_deref(), Some("https://t.me/support"));
        for bad in ["http://example.com/help", "javascript:alert(1)", "tg://resolve?domain=x", "t.me/support", "https://user:pw@example.com/", "https://", "file:///etc/passwd", ""] {
            assert_eq!(parse_https_link(bad), None, "{bad}");
        }
        assert_eq!(parse_https_link(&format!("https://example.com/{}", "a".repeat(3000))), None);
    }

    #[test]
    fn other_clients_of_the_same_link_still_open() {
        // Если владелец сменит правило формата, kl!ck всё равно поймёт ответ: список ссылок и JSON sing-box / Xray.
        assert_eq!(inspect_content(fx!("active_curl.body")), Content::Base64Links(4));
        assert_eq!(inspect_content(fx!("active_singbox.body")), Content::Converted(Format::SingBox, 3));
        assert_eq!(inspect_content(fx!("active_klick_xrayjson.body")), Content::Converted(Format::Xray, 4));
    }

    #[test]
    fn add_to_klick_links_of_the_page_are_accepted() {
        let want = ["Mistgate", "Мой VPN 50% (тест) 🐈.pro", "C++ & Co = test", &"Я".repeat(64), "a#b?c/d"];
        let links: Vec<&str> = fx!("deeplinks.txt").lines().collect();
        assert_eq!(links.len(), 5);
        assert_eq!(links.len(), want.len());
        for (link, name) in links.iter().zip(want) {
            let a = crate::deeplink::parse(link, false).unwrap().expect("это klick://add");
            assert!(a.url.starts_with("https://sub.example.com/k3xq8/") && a.url.len() == "https://sub.example.com/k3xq8/".len() + 43);
            assert_eq!(a.name.as_deref(), Some(name), "{link}");
        }
        // Название длиннее 64 знаков (панель режет до 100) kl!ck обрезает само.
        assert!(links[3].contains(&"%D0%AF".repeat(100)));
    }
}
