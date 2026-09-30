//! Ссылка `klick://add?url=…&name=…` со страницы подписки: kl!ck открывается на «Добавить» с уже
//! вставленной подпиской. Здесь только разбор и проверки — добавляет человек сам, кнопкой.
//!
//! Формы:
//! - `klick://add?url=<https-ссылка, percent-encoded>&name=<название>` — основная, её строит панель;
//! - `klick://add/<https-ссылка>` — как у Happ (`happ://add/…`), ссылка может быть percent-encoded.

use crate::sub::percent_decode;

/// Длиннее не бывает у настоящих подписок; длиннее — почти наверняка мусор или атака.
pub const MAX_URL: usize = 2048;
pub const MAX_NAME: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeepLink {
    /// Открыть «Добавить» с этой подпиской.
    Add { url: String, name: Option<String> },
    /// Другой хост (`klick://something`) — молча ничего не делать: так новые ссылки не ломают старые версии.
    Ignore,
}

/// Почему ссылка не подходит. Сама ссылка в ошибку не попадает: токен в ней — секрет.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    NotKlick,
    NoUrl,
    NotHttps,
    TooLong,
    BadChars,
    NoHost,
}

/// `allow_local_http` — только в отладочной сборке: `http://127.0.0.1` и `http://localhost`.
pub fn parse(raw: &str, allow_local_http: bool) -> Result<DeepLink, LinkError> {
    let raw = raw.trim().trim_matches('"');
    let rest = strip_prefix_ci(raw, "klick://").ok_or(LinkError::NotKlick)?;
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if !rest[..host_end].eq_ignore_ascii_case("add") {
        return Ok(DeepLink::Ignore);
    }
    let tail = &rest[host_end..];
    // Форма Happ: всё после «add/» — ссылка целиком, вместе с её собственным «?».
    let path = tail.strip_prefix('/').unwrap_or("");
    let (url, name) = if !path.is_empty() && !path.starts_with('?') {
        let url = if strip_prefix_ci(path, "https%3a").is_some() || strip_prefix_ci(path, "http%3a").is_some() { percent_decode(path) } else { path.to_string() };
        (url, None)
    } else {
        let query = tail.split_once('?').map(|(_, q)| q).unwrap_or("");
        let query = query.split('#').next().unwrap_or("");
        let mut url = None;
        let mut name = None;
        for pair in query.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            match k {
                "url" if url.is_none() => url = Some(percent_decode(v)),
                "name" if name.is_none() => name = Some(percent_decode(&v.replace('+', " "))),
                _ => {}
            }
        }
        (url.ok_or(LinkError::NoUrl)?, name)
    };
    check_url(&url, allow_local_http)?;
    Ok(DeepLink::Add { url, name: name.and_then(clean_name) })
}

fn check_url(url: &str, allow_local_http: bool) -> Result<(), LinkError> {
    if url.is_empty() {
        return Err(LinkError::NoUrl);
    }
    if url.chars().count() > MAX_URL {
        return Err(LinkError::TooLong);
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(LinkError::BadChars);
    }
    let after = match strip_prefix_ci(url, "https://") {
        Some(a) => a,
        None => {
            let local = strip_prefix_ci(url, "http://").filter(|a| {
                ["127.0.0.1", "localhost"].iter().any(|h| strip_prefix_ci(a, h).is_some_and(|r| r.is_empty() || r.starts_with([':', '/', '?'])))
            });
            match local {
                Some(a) if allow_local_http => a,
                _ => return Err(LinkError::NotHttps),
            }
        }
    };
    let authority = &after[..after.find(['/', '?', '#']).unwrap_or(after.len())];
    // Логин и пароль в адресе (`https://user:pass@host`) в подписках не бывает — это обман.
    let host = authority.rsplit_once(':').map(|(h, p)| if p.chars().all(|c| c.is_ascii_digit()) { h } else { authority }).unwrap_or(authority);
    if host.is_empty() || authority.contains('@') {
        return Err(LinkError::NoHost);
    }
    Ok(())
}

fn clean_name(n: String) -> Option<String> {
    let n: String = n.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(MAX_NAME).collect();
    let n = n.trim().to_string();
    (!n.is_empty()).then_some(n)
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len() && s.is_char_boundary(prefix.len()) && s[..prefix.len()].eq_ignore_ascii_case(prefix)).then(|| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(url: &str, name: Option<&str>) -> Result<DeepLink, LinkError> {
        Ok(DeepLink::Add { url: url.into(), name: name.map(Into::into) })
    }

    #[test]
    fn main_form_with_encoded_url_and_name() {
        let r = parse("klick://add?url=https%3A%2F%2Fsub.example.com%2Fs%2Fa1b2%3Fx%3D1&name=%D0%9C%D0%BE%D0%B9%20VPN", false);
        assert_eq!(r, add("https://sub.example.com/s/a1b2?x=1", Some("Мой VPN")));
    }

    #[test]
    fn happ_form_plain_and_encoded() {
        assert_eq!(parse("klick://add/https://sub.example.com/s/a1b2?token=x&y=2", false), add("https://sub.example.com/s/a1b2?token=x&y=2", None));
        assert_eq!(parse("klick://add/https%3A%2F%2Fsub.example.com%2Fs%2Fa1b2", false), add("https://sub.example.com/s/a1b2", None));
    }

    #[test]
    fn tolerant_to_case_quotes_trailing_slash_and_unknown_params() {
        assert_eq!(parse("\"KLICK://ADD/?url=https%3A%2F%2Fsub.example.com&utm=1&name=A+B\"", false), add("https://sub.example.com", Some("A B")));
    }

    #[test]
    fn other_hosts_are_ignored_silently() {
        assert_eq!(parse("klick://something?url=https://x.y", false), Ok(DeepLink::Ignore));
        assert_eq!(parse("klick://", false), Ok(DeepLink::Ignore));
    }

    #[test]
    fn rejected_links() {
        assert_eq!(parse("happ://add/https://sub.example.com", false), Err(LinkError::NotKlick));
        assert_eq!(parse("klick://add", false), Err(LinkError::NoUrl));
        assert_eq!(parse("klick://add?name=x", false), Err(LinkError::NoUrl));
        assert_eq!(parse("klick://add?url=http%3A%2F%2Fsub.example.com", false), Err(LinkError::NotHttps));
        assert_eq!(parse("klick://add?url=javascript%3Aalert(1)", false), Err(LinkError::NotHttps));
        assert_eq!(parse("klick://add?url=file%3A%2F%2F%2FC%3A%2Fx", false), Err(LinkError::NotHttps));
        assert_eq!(parse("klick://add?url=https%3A%2F%2Fa.b%2Fx%20y", false), Err(LinkError::BadChars));
        assert_eq!(parse("klick://add?url=https%3A%2F%2Fa.b%2F%0Ax", false), Err(LinkError::BadChars));
        assert_eq!(parse("klick://add?url=https%3A%2F%2F%2Fpath", false), Err(LinkError::NoHost));
        assert_eq!(parse("klick://add?url=https%3A%2F%2Fuser%3Apass%40evil.com%2Fs", false), Err(LinkError::NoHost));
        let long = format!("klick://add?url=https%3A%2F%2Fa.b%2F{}", "x".repeat(MAX_URL));
        assert_eq!(parse(&long, false), Err(LinkError::TooLong));
    }

    #[test]
    fn local_http_only_in_debug_builds() {
        assert_eq!(parse("klick://add?url=http%3A%2F%2F127.0.0.1%3A8080%2Fsub", false), Err(LinkError::NotHttps));
        assert_eq!(parse("klick://add?url=http%3A%2F%2F127.0.0.1%3A8080%2Fsub", true), add("http://127.0.0.1:8080/sub", None));
        assert_eq!(parse("klick://add?url=http%3A%2F%2Flocalhost%2Fsub", true), add("http://localhost/sub", None));
        assert_eq!(parse("klick://add?url=http%3A%2F%2Flocalhost.evil.com%2Fsub", true), Err(LinkError::NotHttps));
    }

    #[test]
    fn name_is_cleaned_and_cut() {
        let long = "я".repeat(100);
        let DeepLink::Add { name, .. } = parse(&format!("klick://add?url=https%3A%2F%2Fa.b&name={long}"), false).unwrap() else { panic!() };
        assert_eq!(name.unwrap().chars().count(), MAX_NAME);
        let DeepLink::Add { name, .. } = parse("klick://add?url=https%3A%2F%2Fa.b&name=%20%0A%20", false).unwrap() else { panic!() };
        assert_eq!(name, None);
    }
}
