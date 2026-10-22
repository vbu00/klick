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
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(compact.trim_end_matches('=').as_bytes()))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(compact.trim_end_matches('=').as_bytes()));
    let text = String::from_utf8(decoded.ok()?).ok()?;
    (count_links(&text) > 0).then_some(text)
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
