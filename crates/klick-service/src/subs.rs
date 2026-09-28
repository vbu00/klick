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
    let body = res.text().await.map_err(|_| ErrorInfo::new("sub.fetch_failed"))?;
    let content = check(&body)?;
    Ok(Fetched { body, info, title, interval_hours, content })
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
