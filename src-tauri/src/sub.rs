//! Подписки: скачать, разобрать заголовки (трафик, срок, название,
//! интервал обновления) и тело (YAML mihomo или список ссылок).
//!
//! Панели выбирают формат по User-Agent. «clash-verge» узнают и Remnawave,
//! и Marzban — отдают готовый YAML mihomo со всеми тонкостями транспорта;
//! 3x-ui отдаёт base64-список ссылок при любом UA, его тоже понимаем.

use serde_json::Value;

use crate::state::SubInfo;

const UA: &str = "clash-verge/v2.4.2";

pub struct Fetched {
    pub proxies: Vec<Value>,
    pub info: Option<SubInfo>,
    pub title: Option<String>,
    pub update_hours: Option<u32>,
}

pub fn fetch(url: &str) -> Result<Fetched, String> {
    let dir = std::env::temp_dir();
    let tag = crate::sys::random_hex(6);
    let head = dir.join(format!("klick-sub-{tag}.head"));
    let body = dir.join(format!("klick-sub-{tag}.body"));
    let out = crate::sys::command("curl.exe")
        .args(["-sS", "-L", "--fail", "--max-time", "30", "--compressed", "-A", UA, "-D"])
        .arg(&head)
        .arg("-o")
        .arg(&body)
        .arg(url)
        .output()
        .map_err(|e| format!("не удалось запустить curl: {e}"));
    let headers = std::fs::read(&head).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let text = std::fs::read(&body).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let _ = std::fs::remove_file(&head);
    let _ = std::fs::remove_file(&body);
    let out = out?;
    if !out.status.success() {
        let err = crate::sys::decode_console(&out.stderr);
        let err = err.trim().trim_start_matches("curl: ").trim();
        return Err(match status_code(&headers) {
            Some(code) => format!("Сервер подписки ответил {code}. Проверьте ссылку или повторите позже."),
            None if err.is_empty() => "Подписка не скачалась.".into(),
            None => format!("Подписка не скачалась: {err}"),
        });
    }
    let proxies = crate::links::parse_any(&text).map_err(|e| format!("Подписка скачалась, но серверов в ней нет: {e}"))?;
    let h = last_block(&headers);
    Ok(Fetched {
        proxies,
        info: header(&h, "subscription-userinfo").and_then(|v| parse_userinfo(&v)),
        title: header(&h, "profile-title").map(|t| decode_title(&t)).filter(|t| !t.is_empty()),
        update_hours: header(&h, "profile-update-interval").and_then(|v| v.trim().parse().ok()),
    })
}

/// После редиректов в файле несколько блоков заголовков — нужен последний.
fn last_block(raw: &str) -> Vec<(String, String)> {
    let blocks: Vec<&str> = raw.split("\r\n\r\n").filter(|b| b.trim_start().starts_with("HTTP/")).collect();
    let last = blocks.last().copied().unwrap_or(raw);
    last.lines()
        .skip(1)
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect()
}

fn status_code(raw: &str) -> Option<u16> {
    raw.lines().filter(|l| l.starts_with("HTTP/")).last()?.split_whitespace().nth(1)?.parse().ok()
}

fn header(h: &[(String, String)], name: &str) -> Option<String> {
    h.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

/// `upload=1; download=2; total=3; expire=4` — порядок и пробелы любые.
pub fn parse_userinfo(v: &str) -> Option<SubInfo> {
    let mut info = SubInfo::default();
    let mut any = false;
    for part in v.split(';') {
        let Some((k, val)) = part.split_once('=') else { continue };
        let n: u64 = val.trim().parse::<f64>().map(|f| f.max(0.0) as u64).unwrap_or(0);
        match k.trim().to_ascii_lowercase().as_str() {
            "upload" => info.upload = n,
            "download" => info.download = n,
            "total" => info.total = n,
            "expire" => info.expire = n,
            _ => continue,
        }
        any = true;
    }
    any.then_some(info)
}

/// profile-title бывает простой строкой или «base64:…».
pub fn decode_title(v: &str) -> String {
    let v = v.trim();
    if let Some(b) = v.strip_prefix("base64:") {
        if let Some(s) = crate::sys::base64_decode(b).and_then(|b| String::from_utf8(b).ok()) {
            return s.trim().to_string();
        }
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn userinfo() {
        let i = parse_userinfo("upload=455727941; download=6174315083; total=214748364800; expire=1795132800").unwrap();
        assert_eq!(i.total, 214748364800);
        assert_eq!(i.expire, 1795132800);
        assert_eq!(parse_userinfo("upload=0;download=1.5e9;total=0").unwrap().download, 1_500_000_000);
        assert!(parse_userinfo("мусор").is_none());
    }

    #[test]
    fn заголовки_после_редиректа() {
        let raw = "HTTP/1.1 302 Found\r\nLocation: /x\r\n\r\nHTTP/2 200\r\nprofile-title: base64:0JrQu9GO0LrQstCw\r\nSubscription-Userinfo: upload=1; download=2; total=3; expire=0\r\n\r\n";
        let h = last_block(raw);
        assert_eq!(decode_title(&header(&h, "profile-title").unwrap()), "Клюква");
        assert_eq!(parse_userinfo(&header(&h, "subscription-userinfo").unwrap()).unwrap().total, 3);
        assert_eq!(status_code(raw), Some(200));
    }
}
