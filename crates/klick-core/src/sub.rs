//! Подписки: что вставил пользователь, что ответила панель, сколько там серверов.

use crate::model::SubInfo;
use base64::Engine as _;

/// Схемы одиночных ссылок, которые понимает ядро.
pub const LINK_SCHEMES: [&str; 14] = [
    "vless", "vmess", "trojan", "ss", "ssr", "socks", "socks5", "hysteria", "hysteria2", "hy2", "tuic", "wireguard", "wg", "anytls",
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
    if rest.is_empty() || s.contains(char::is_whitespace) {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    if scheme == "http" || scheme == "https" {
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

/// Что лежит в ответе панели.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// Конфиг mihomo/Clash со списком `proxies`.
    ClashYaml,
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
    let links = count_links(t);
    if links > 0 {
        return Content::Links(links);
    }
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(compact.trim_end_matches('=').as_bytes()))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(compact.trim_end_matches('=').as_bytes()));
    if let Ok(bytes) = decoded {
        if let Ok(text) = String::from_utf8(bytes) {
            let n = count_links(&text);
            if n > 0 {
                return Content::Base64Links(n);
            }
        }
    }
    Content::Unknown
}

fn count_links(text: &str) -> usize {
    text.lines().filter(|l| detect_source(l) == Some(SourceKind::Link)).count()
}

fn percent_decode(s: &str) -> String {
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
    }
}
