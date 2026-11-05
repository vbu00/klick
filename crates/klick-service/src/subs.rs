//! Скачивание подписок. Ядро само за ними не ходит: служба качает, проверяет и кладёт файл.

use klick_core::sub::{self, Content};
use klick_core::SubInfo;
use klick_proto::ErrorInfo;
use serde_json::json;
use std::time::Duration;

/// Так панели вроде Remnawave и Marzban отдают конфиг в формате mihomo.
pub const USER_AGENT: &str = "mihomo/1.19.31";

pub struct Fetched {
    pub body: String,
    pub info: Option<SubInfo>,
    pub title: Option<String>,
    pub interval_hours: Option<u32>,
    pub web_page: Option<String>,
    pub support_url: Option<String>,
    pub content: Content,
}

/// `via_port` — служебный вход ядра, если VPN включён; иначе напрямую, мимо любых прокси.
pub async fn fetch(url: &str, via_port: Option<u16>) -> Result<Fetched, ErrorInfo> {
    let mut builder = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10));
    builder = match via_port {
        Some(port) => builder.proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).map_err(|_| ErrorInfo::new("sub.fetch_failed"))?),
        None => builder.no_proxy(),
    };
    let client = builder.build().map_err(|_| ErrorInfo::new("sub.fetch_failed"))?;
    let res = client.get(url).send().await.map_err(|e| {
        tracing::warn!("подписка не скачалась: {}", without_url(&e));
        ErrorInfo::with("sub.fetch_failed", json!({ "reason": if e.is_timeout() { "timeout" } else if e.is_connect() { "connect" } else { "other" } }))
    })?;
    let status = res.status();
    if !status.is_success() {
        return Err(ErrorInfo::with("sub.http_status", json!({ "status": status.as_u16() })));
    }
    let header = |name: &str| res.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    let info = header("subscription-userinfo").and_then(|h| sub::parse_userinfo(&h));
    let interval_hours = header("profile-update-interval").and_then(|h| sub::parse_update_interval(&h));
    let title = header("profile-title")
        .and_then(|h| sub::parse_title(&h))
        .or_else(|| header("content-disposition").and_then(|h| sub::filename_from_disposition(&h)));
    let web_page = header("profile-web-page-url").and_then(|h| web_link(&h));
    let support_url = header("support-url").and_then(|h| web_link(&h));
    let body = res.text().await.map_err(|_| ErrorInfo::new("sub.fetch_failed"))?;
    let content = check(&body)?;
    Ok(Fetched { body, info, title, interval_hours, web_page, support_url, content })
}

/// Ссылка на страницу провайдера из заголовка: только http(s), без пробелов, не длиннее 2048 символов.
/// Окно откроет её в браузере, поэтому другие схемы не пропускаем.
pub fn web_link(value: &str) -> Option<String> {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    let ok = (lower.starts_with("https://") || lower.starts_with("http://")) && v.len() <= 2048 && !v.chars().any(|c| c.is_whitespace() || c.is_control());
    let host = v.split_once("://").map(|(_, r)| r.split(['/', '?', '#']).next().unwrap_or("")).unwrap_or("");
    (ok && !host.is_empty() && !host.contains('@')).then(|| v.to_string())
}

/// Что внутри: подходит ли это ядру.
pub fn check(body: &str) -> Result<Content, ErrorInfo> {
    match sub::inspect_content(body) {
        Content::Html => Err(ErrorInfo::new("sub.not_subscription")),
        Content::Unknown => Err(ErrorInfo::new("sub.unknown_format")),
        other => Ok(other),
    }
}

/// Текст ошибки без ссылки: ссылка подписки — секрет и в журнал не попадает.
fn without_url(e: &reqwest::Error) -> String {
    let mut text = e.to_string();
    if let Some(url) = e.url() {
        text = text.replace(url.as_str(), "<подписка>");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::web_link;

    #[test]
    fn web_link_only_http() {
        assert_eq!(web_link(" https://panel.example.com/u/1 ").as_deref(), Some("https://panel.example.com/u/1"));
        assert_eq!(web_link("https://t.me/support").as_deref(), Some("https://t.me/support"));
        assert_eq!(web_link("javascript:alert(1)"), None);
        assert_eq!(web_link("tg://resolve?domain=x"), None);
        assert_eq!(web_link("https://user:pass@example.com/"), None);
        assert_eq!(web_link("https:// example.com"), None);
        assert_eq!(web_link("https://"), None);
    }
}
