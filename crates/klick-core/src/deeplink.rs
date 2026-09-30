//! Ссылка `klick://add`: страница подписки открывает kl!ck на экране «Добавить» с уже вставленной
//! подпиской, человек нажимает одну кнопку. Разбор и проверки — здесь, без Tauri и без сети.
//!
//! Формы:
//! * `klick://add?url=<https-ссылка, percent-encoded>&name=<название, необязательно>` — основная,
//!   её строит панель;
//! * `klick://add/<https-ссылка>` — как у Happ (`happ://add/…`), ссылка тоже может быть percent-encoded.
//!
//! Ссылку открыть может любой сайт, поэтому kl!ck по ней ничего не добавляет сам: только показывает
//! экран «Добавить». Сама ссылка подписки — секрет (токен в пути): её не пишут в журналы.

use percent_encoding::percent_decode_str;

pub const SCHEME: &str = "klick";
/// Ссылка подписки длиннее — отклоняется.
pub const MAX_URL: usize = 2048;
/// Название обрезается до стольких символов.
pub const MAX_NAME: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddLink {
    /// Ссылка подписки, `https://…`, как пришла (без нормализации: токен в ней должен остаться точным).
    pub url: String,
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("в ссылке нет url")]
    NoUrl,
    #[error("ссылка подписки не https")]
    NotHttps,
    #[error("ссылка подписки длиннее {MAX_URL} символов")]
    TooLong,
    #[error("в ссылке подписки пробелы или управляющие символы")]
    BadChars,
    #[error("ссылка подписки не разбирается")]
    Malformed,
}

/// Разобрать ссылку, с которой открыли kl!ck. `Ok(None)` — это не `klick://add` (другой хост или
/// вообще не `klick://`): такую ссылку молча пропускают. `allow_local_http` — только для отладочной
/// сборки: тогда подходят ещё `http://127.0.0.1` и `http://localhost`.
pub fn parse(raw: &str, allow_local_http: bool) -> Result<Option<AddLink>, LinkError> {
    let raw = raw.trim();
    let Some((scheme, rest)) = raw.split_once("://") else { return Ok(None) };
    if !scheme.eq_ignore_ascii_case(SCHEME) {
        return Ok(None);
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (host, tail) = rest.split_at(end);
    if !host.eq_ignore_ascii_case("add") {
        return Ok(None);
    }
    // Даже в закодированном виде ссылка не бывает втрое длиннее разрешённой: дальше не разбираем.
    if tail.len() > MAX_URL * 3 + 1024 {
        return Err(LinkError::TooLong);
    }

    let (url, name) = match tail.strip_prefix('/').filter(|p| !p.is_empty() && !p.starts_with('?')) {
        // klick://add/<ссылка>: всё после `add/` — ссылка, со своими параметрами и всем остальным.
        Some(path) => (if starts_with_http(path) { path.to_string() } else { decode(path)? }, None),
        // klick://add?… (или add/?…): параметры; неизвестные пропускаем — позже можно добавить новые.
        None => {
            let query = tail.trim_start_matches('/').strip_prefix('?').unwrap_or("");
            let query = query.split('#').next().unwrap_or("");
            let (mut url, mut name) = (None, None);
            for pair in query.split('&') {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                match k {
                    // В ссылке подписки `+` — это `+`, а в названии из формы — пробел.
                    "url" if url.is_none() => url = Some(decode(v)?),
                    "name" if name.is_none() => name = Some(decode(&v.replace('+', " "))?),
                    _ => {}
                }
            }
            (url.ok_or(LinkError::NoUrl)?, name)
        }
    };
    Ok(Some(AddLink { url: check_url(&url, allow_local_http)?, name: name.as_deref().map(clean_name).filter(|n| !n.is_empty()) }))
}

/// Домен ссылки подписки — его показывают крупно вместо всей ссылки: токен в пути — секрет.
pub fn host(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(str::to_ascii_lowercase)
}

fn starts_with_http(s: &str) -> bool {
    let lower = s.get(..8).unwrap_or(s).to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

fn decode(s: &str) -> Result<String, LinkError> {
    percent_decode_str(s).decode_utf8().map(|c| c.into_owned()).map_err(|_| LinkError::Malformed)
}

fn check_url(url: &str, allow_local_http: bool) -> Result<String, LinkError> {
    if url.is_empty() {
        return Err(LinkError::NoUrl);
    }
    if url.chars().count() > MAX_URL {
        return Err(LinkError::TooLong);
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(LinkError::BadChars);
    }
    let parsed = url::Url::parse(url).map_err(|_| LinkError::Malformed)?;
    match parsed.scheme() {
        "https" => {}
        "http" if allow_local_http && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost")) => {}
        _ => return Err(LinkError::NotHttps),
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err(LinkError::Malformed);
    }
    Ok(url.to_string())
}

/// Название — обычный текст: без управляющих символов, без лишних пробелов, не длиннее 64 символов.
fn clean_name(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s.chars().take(MAX_NAME).collect::<String>().trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(raw: &str) -> Result<Option<AddLink>, LinkError> {
        parse(raw, false)
    }

    fn ok(url: &str, name: Option<&str>) -> Result<Option<AddLink>, LinkError> {
        Ok(Some(AddLink { url: url.into(), name: name.map(Into::into) }))
    }

    #[test]
    fn main_form_with_name() {
        assert_eq!(
            add("klick://add?url=https%3A%2F%2Fsub.example.com%2Fs%2Fa1b2&name=%D0%9C%D0%BE%D0%B9%20VPN"),
            ok("https://sub.example.com/s/a1b2", Some("Мой VPN"))
        );
        assert_eq!(add("klick://add?url=https%3A%2F%2Fsub.example.com%2Fs"), ok("https://sub.example.com/s", None));
        // Название из HTML-формы: пробел как `+`. В самой ссылке `+` остаётся `+`.
        assert_eq!(add("klick://add?name=%D0%9C%D0%BE%D0%B9+VPN&url=https://sub.example.com/a+b"), ok("https://sub.example.com/a+b", Some("Мой VPN")));
        assert_eq!(add("KLICK://ADD/?url=https%3A%2F%2Fsub.example.com%2Fs"), ok("https://sub.example.com/s", None));
    }

    #[test]
    fn happ_style_form() {
        assert_eq!(add("klick://add/https://sub.example.com/s/a1b2?x=1&y=2"), ok("https://sub.example.com/s/a1b2?x=1&y=2", None));
        assert_eq!(add("klick://add/https%3A%2F%2Fsub.example.com%2Fs%2Fa1b2"), ok("https://sub.example.com/s/a1b2", None));
        // Своё percent-кодирование внутри незакодированной ссылки не трогаем.
        assert_eq!(add("klick://add/https://sub.example.com/s/%D1%82%D0%BE%D0%BA"), ok("https://sub.example.com/s/%D1%82%D0%BE%D0%BA", None));
    }

    #[test]
    fn other_links_are_ignored_silently() {
        assert_eq!(add("klick://something?url=https%3A%2F%2Fsub.example.com"), Ok(None));
        assert_eq!(add("klick://"), Ok(None));
        assert_eq!(add("https://sub.example.com/s"), Ok(None));
        assert_eq!(add("--hidden"), Ok(None));
        assert_eq!(add("happ://add/https://sub.example.com/s"), Ok(None));
    }

    #[test]
    fn unknown_parameters_are_ignored() {
        assert_eq!(add("klick://add?v=2&url=https%3A%2F%2Fsub.example.com%2Fs&color=red&name=VPN"), ok("https://sub.example.com/s", Some("VPN")));
        // Первое значение побеждает.
        assert_eq!(add("klick://add?url=https%3A%2F%2Fa.example.com&url=https%3A%2F%2Fb.example.com"), ok("https://a.example.com", None));
    }

    #[test]
    fn bad_links_are_rejected() {
        assert_eq!(add("klick://add"), Err(LinkError::NoUrl));
        assert_eq!(add("klick://add?name=VPN"), Err(LinkError::NoUrl));
        assert_eq!(add("klick://add?url="), Err(LinkError::NoUrl));
        assert_eq!(add("klick://add?url=http%3A%2F%2Fsub.example.com%2Fs"), Err(LinkError::NotHttps));
        assert_eq!(add("klick://add?url=javascript%3Aalert(1)"), Err(LinkError::NotHttps));
        assert_eq!(add("klick://add?url=file%3A%2F%2F%2Fetc%2Fpasswd"), Err(LinkError::NotHttps));
        assert_eq!(add("klick://add?url=vless%3A%2F%2Fid%40host%3A443"), Err(LinkError::NotHttps));
        assert_eq!(add("klick://add/javascript:alert(1)"), Err(LinkError::NotHttps));
        assert_eq!(add("klick://add?url=https%3A%2F%2Fsub.example.com%2Fa%20b"), Err(LinkError::BadChars));
        assert_eq!(add("klick://add?url=https%3A%2F%2Fsub.example.com%2Fa%0Ab"), Err(LinkError::BadChars));
        assert_eq!(add("klick://add?url=https%3A%2F%2F"), Err(LinkError::Malformed));
        assert_eq!(add("klick://add?url=%FF%FE"), Err(LinkError::Malformed));
        let long = format!("klick://add?url=https%3A%2F%2Fsub.example.com%2F{}", "a".repeat(MAX_URL));
        assert_eq!(add(&long), Err(LinkError::TooLong));
        let just_fits = format!("https://sub.example.com/{}", "a".repeat(MAX_URL - "https://sub.example.com/".len()));
        assert_eq!(add(&format!("klick://add/{just_fits}")), ok(&just_fits, None));
        assert_eq!(add(&format!("klick://add/{}", "a".repeat(MAX_URL * 4))), Err(LinkError::TooLong));
    }

    #[test]
    fn local_http_only_in_debug_builds() {
        let local = "klick://add?url=http%3A%2F%2F127.0.0.1%3A8080%2Fs";
        assert_eq!(parse(local, false), Err(LinkError::NotHttps));
        assert_eq!(parse(local, true), ok("http://127.0.0.1:8080/s", None));
        assert_eq!(parse("klick://add/http://localhost/s", true), ok("http://localhost/s", None));
        assert_eq!(parse("klick://add/http://example.com/s", true), Err(LinkError::NotHttps));
    }

    #[test]
    fn name_is_plain_text_up_to_64_chars() {
        let long = "Я".repeat(100);
        let r = add(&format!("klick://add?url=https%3A%2F%2Fsub.example.com&name={long}")).unwrap().unwrap();
        assert_eq!(r.name.unwrap().chars().count(), MAX_NAME);
        let r = add("klick://add?url=https%3A%2F%2Fsub.example.com&name=%20%20My%0A%09VPN%20%20").unwrap().unwrap();
        assert_eq!(r.name.as_deref(), Some("My VPN"));
        let r = add("klick://add?url=https%3A%2F%2Fsub.example.com&name=%20%0A").unwrap().unwrap();
        assert_eq!(r.name, None);
    }

    #[test]
    fn same_result_after_the_plugin_parsed_the_link() {
        // Плагин deep-link отдаёт ссылку как `url::Url`: его строка разбирается так же, как исходная.
        for raw in [
            "klick://add?url=https%3A%2F%2Fsub.example.com%2Fs%2Fa1b2&name=%D0%9C%D0%BE%D0%B9%20VPN",
            "klick://add?url=https%3A%2F%2Fsub.example.com%2Fs&name=Мой VPN",
            "klick://add/https://sub.example.com/s/a1b2?x=1&y=2",
            "klick://add/https%3A%2F%2Fsub.example.com%2Fs%2Fa1b2",
            "klick://add/?url=https%3A%2F%2Fsub.example.com%2Fs#frag",
            "klick://add?url=javascript%3Aalert(1)",
            "klick://add",
            "klick://other?url=https%3A%2F%2Fsub.example.com",
        ] {
            let plugin = url::Url::parse(raw).unwrap();
            assert_eq!(add(plugin.as_str()), add(raw), "{raw} → {plugin}");
        }
    }

    #[test]
    fn host_is_shown_instead_of_the_link() {
        assert_eq!(host("https://Sub.Example.com/s/secret-token"), Some("sub.example.com".into()));
        assert_eq!(host("https://sub.example.com@evil.example/s"), Some("evil.example".into()));
        assert_eq!(host("not a link"), None);
    }
}
