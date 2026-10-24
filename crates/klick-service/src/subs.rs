//! Скачивание подписок. Ядро само за ними не ходит: служба качает, проверяет и кладёт файл.

use klick_core::sub::{self, Content, Unusable};
use klick_core::SubInfo;
use klick_proto::ErrorInfo;
use serde_json::json;
use std::time::Duration;
use reqwest::Url;

/// Так панели вроде Remnawave и Marzban отдают конфиг в формате mihomo.
pub const USER_AGENT: &str = "mihomo/1.19.33";
/// Подписка — десятки килобайт; больше — не подписка, а память службы не резиновая.
const BODY_LIMIT: usize = 16 << 20;

pub struct Fetched {
    pub body: String,
    pub info: Option<SubInfo>,
    pub title: Option<String>,
    pub interval_hours: Option<u32>,
    pub content: Content,
}

/// Срок, трафик, объявление и интервал из ответа, серверы которого взять нельзя (отказ панели).
pub struct Refusal {
    pub info: Option<SubInfo>,
    pub interval_hours: Option<u32>,
}

/// Ошибка скачивания. Если панель отказала ответом-заглушкой, в `refusal` лежит то, что из него можно сохранить:
/// старый файл серверов при этом не трогаем, а срок, трафик и «заходить раз в час» запоминаем.
pub struct FetchError {
    pub error: ErrorInfo,
    pub refusal: Option<Refusal>,
}

impl From<ErrorInfo> for FetchError {
    fn from(error: ErrorInfo) -> Self {
        Self { error, refusal: None }
    }
}

impl From<FetchError> for ErrorInfo {
    fn from(e: FetchError) -> Self {
        e.error
    }
}

/// `via_port` — служебный вход ядра, если VPN включён; иначе напрямую, мимо любых прокси.
pub async fn fetch(url: &str, via_port: Option<u16>) -> Result<Fetched, FetchError> {
    let mut builder = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10));
    builder = match via_port {
        Some(port) => builder.proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).map_err(|_| ErrorInfo::new("sub.fetch_failed"))?),
        None => builder.no_proxy(),
    };
    let client = builder.build().map_err(|_| ErrorInfo::new("sub.fetch_failed"))?;
    let target = sub::fetch_url(url);
    let mut req = client.get(&target);
    for (name, value) in crate::hwid::headers(&target) {
        req = req.header(name, value);
    }
    let mut res = req.send().await.map_err(|e| {
        tracing::warn!("подписка не скачалась: {}", without_url(&e));
        ErrorInfo::with("sub.fetch_failed", json!({ "reason": if e.is_timeout() { "timeout" } else if e.is_connect() { "connect" } else { "other" } }))
    })?;
    let status = res.status();
    if !status.is_success() {
        return Err(ErrorInfo::with("sub.http_status", json!({ "status": status.as_u16() })).into());
    }
    // Значения заголовков читаем как UTF-8: панели присылают и сырой русский текст.
    let header = |name: &str| res.headers().get(name).and_then(|v| String::from_utf8(v.as_bytes().to_vec()).ok());
    let mut info = header("subscription-userinfo").and_then(|h| sub::parse_userinfo(&h));
    let announce = header("announce").and_then(|h| sub::parse_announce(&h));
    let support_url = header("support-url").and_then(|h| sub::parse_https_link(&h));
    let page_url = header("profile-web-page-url").and_then(|h| sub::parse_https_link(&h));
    let probe_report = header("probe-report").and_then(|h| parse_probe_report(&h));
    if announce.is_some() || support_url.is_some() || page_url.is_some() || probe_report.is_some() {
        let i = info.get_or_insert_with(SubInfo::default);
        (i.announce, i.support_url, i.page_url, i.probe_report) = (announce.clone(), support_url, page_url, probe_report);
    }
    let interval_hours = header("profile-update-interval").and_then(|h| sub::parse_update_interval(&h));
    let title = header("profile-title")
        .and_then(|h| sub::parse_title(&h))
        .or_else(|| header("content-disposition").and_then(|h| sub::filename_from_disposition(&h)));
    let mut raw = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|_| ErrorInfo::new("sub.fetch_failed"))? {
        raw.extend_from_slice(&chunk);
        if raw.len() > BODY_LIMIT {
            return Err(ErrorInfo::with("sub.fetch_failed", json!({ "reason": "too_large" })).into());
        }
    }
    let body = String::from_utf8_lossy(&raw).into_owned();
    let content = match check(&body) {
        Ok(c) => c,
        Err(mut error) if error.code == "sub.refused" => {
            // Имени у заглушки нет — причиной станет объявление панели.
            if error.params.get("reason").is_none() {
                if let Some(a) = announce.as_deref().map(sub::sanitize_reason).filter(|a| !a.is_empty()) {
                    error.params = json!({ "reason": a });
                }
            }
            return Err(FetchError { error, refusal: Some(Refusal { info, interval_hours }) });
        }
        Err(error) => return Err(error.into()),
    };
    // sing-box, Xray и прочее — в серверы mihomo: ядро читает файл серверов только в своём формате.
    let body = sub::provider_body(&body, content);
    Ok(Fetched { body, info, title, interval_hours, content })
}

fn parse_probe_report(header: &str) -> Option<u32> {
    header.trim().parse::<u32>().ok().filter(|version| *version > 0)
}

/// Адрес отправки строится только из ссылки той подписки, которая включила отчёты.
pub fn report_url(sub_url: &str) -> Option<String> {
    let mut url = Url::parse(&sub::fetch_url(sub_url)).ok()?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    url.set_query(None);
    url.set_fragment(None);
    url.path_segments_mut().ok()?.pop_if_empty().push("probe");
    Some(url.to_string())
}

/// Что внутри: подходит ли это ядру. Ответ без настоящих серверов отвергаем, чтобы он не стёр рабочий список.
pub fn check(body: &str) -> Result<Content, ErrorInfo> {
    let content = match sub::inspect_content(body) {
        Content::Html => return Err(ErrorInfo::new("sub.not_subscription")),
        Content::Unknown if body.trim().is_empty() => return Err(ErrorInfo::new("sub.no_servers")),
        Content::Unknown => return Err(ErrorInfo::new("sub.unknown_format")),
        other => other,
    };
    match sub::unusable(body, content) {
        None => Ok(content),
        Some(Unusable::NoServers) => Err(ErrorInfo::new("sub.no_servers")),
        Some(Unusable::Placeholder(Some(reason))) => Err(ErrorInfo::with("sub.refused", json!({ "reason": reason }))),
        Some(Unusable::Placeholder(None)) => Err(ErrorInfo::new("sub.refused")),
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
    use super::{parse_probe_report, report_url};

    #[test]
    fn report_url_uses_only_subscription_path() {
        assert_eq!(report_url("https://panel.example/pre/token?format=x#top").as_deref(), Some("https://panel.example/pre/token/probe"));
        assert_eq!(report_url("https://panel.example/pre/token/").as_deref(), Some("https://panel.example/pre/token/probe"));
        assert_eq!(report_url("ssconf://panel.example/pre/key?format=x").as_deref(), Some("https://panel.example/pre/key/probe"));
        assert_eq!(report_url("http://panel.example/pre/token"), None);
        assert_eq!(report_url("https://user:pass@panel.example/pre/token"), None);
    }

    #[test]
    fn panel_report_version_is_bounded_to_positive_numbers() {
        assert_eq!(parse_probe_report(" 1 "), Some(1));
        assert_eq!(parse_probe_report("0"), None);
        assert_eq!(parse_probe_report("v1"), None);
    }
}
