//! Ядро mihomo: запуск, готовность, остановка и API через именованный канал (Windows)
//! или Unix-сокет (macOS).

#[cfg(windows)]
use crate::win::Job;
use anyhow::{bail, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, HOST};
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tokio::sync::oneshot;

#[cfg(windows)]
const ERROR_PIPE_BUSY: i32 = 231;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Обычный запрос к ядру отвечает за миллисекунды; дольше — значит, ядро зависло.
const CALL_LIMIT: Duration = Duration::from_secs(10);

/// API ядра. Каждый запрос — отдельное подключение к каналу.
#[derive(Clone, Debug)]
pub struct CoreApi {
    pipe: String,
    secret: String,
}

impl CoreApi {
    pub fn new(pipe: impl Into<String>, secret: impl Into<String>) -> Self {
        Self { pipe: pipe.into(), secret: secret.into() }
    }

    #[cfg(windows)]
    async fn open(&self) -> Result<NamedPipeClient> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match ClientOptions::new().open(&self.pipe) {
                Ok(c) => return Ok(c),
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => return Err(e).context("канал ядра недоступен"),
            }
        }
    }

    #[cfg(unix)]
    async fn open(&self) -> Result<tokio::net::UnixStream> {
        tokio::net::UnixStream::connect(&self.pipe).await.context("сокет ядра недоступен")
    }

    async fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<hyper::Response<hyper::body::Incoming>> {
        let io = TokioIo::new(self.open().await?);
        let (mut tx, conn) = hyper::client::conn::http1::handshake(io).await.context("handshake с ядром")?;
        tokio::spawn(async move {
            let _ = conn.await;
        });
        let payload = match body {
            Some(v) => Bytes::from(serde_json::to_vec(v)?),
            None => Bytes::new(),
        };
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header(HOST, "klick-core")
            .header(AUTHORIZATION, format!("Bearer {}", self.secret))
            .header(CONTENT_TYPE, "application/json")
            .body(Full::new(payload))?;
        Ok(tx.send_request(req).await.context("запрос к ядру")?)
    }

    pub async fn call(&self, method: Method, path: &str, body: Option<&Value>) -> Result<(StatusCode, Bytes)> {
        self.call_within(method, path, body, CALL_LIMIT).await
    }

    /// Запрос целиком — подключение, заголовки, тело — не дольше `limit`. Зависшее ядро не должно
    /// останавливать службу: пока она ждёт ответа, не работает даже кнопка «Отключить».
    pub async fn call_within(&self, method: Method, path: &str, body: Option<&Value>, limit: Duration) -> Result<(StatusCode, Bytes)> {
        let request = async {
            let res = self.send(method, path, body).await?;
            let status = res.status();
            let bytes = res.into_body().collect().await?.to_bytes();
            Ok::<_, anyhow::Error>((status, bytes))
        };
        tokio::time::timeout(limit, request).await.map_err(|_| anyhow::anyhow!("ядро не ответило за {} с", limit.as_secs()))?
    }

    pub async fn get_json(&self, path: &str) -> Result<Value> {
        let (status, body) = self.call(Method::GET, path, None).await?;
        if !status.is_success() {
            bail!("ядро ответило {status} на {path}: {}", String::from_utf8_lossy(&body));
        }
        Ok(serde_json::from_slice(&body)?)
    }

    pub async fn version(&self) -> Result<String> {
        let v = self.get_json("/version").await?;
        Ok(v["version"].as_str().unwrap_or_default().to_string())
    }

    /// Серверы провайдера: имя и протокол.
    pub async fn provider_proxies(&self, provider: &str) -> Result<Vec<(String, String)>> {
        let v = self.get_json(&format!("/providers/proxies/{}", enc(provider))).await?;
        let list = v["proxies"].as_array().cloned().unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|p| Some((p["name"].as_str()?.to_string(), p["type"].as_str().unwrap_or("").to_lowercase())))
            .collect())
    }

    pub async fn selected(&self, group: &str) -> Result<Option<String>> {
        let v = self.get_json(&format!("/proxies/{}", enc(group))).await?;
        Ok(v["now"].as_str().map(str::to_string))
    }

    pub async fn select(&self, group: &str, name: &str) -> Result<()> {
        let (status, body) = self.call(Method::PUT, &format!("/proxies/{}", enc(group)), Some(&json!({ "name": name }))).await?;
        if !status.is_success() {
            bail!("ядро не переключило сервер ({status}): {}", String::from_utf8_lossy(&body));
        }
        Ok(())
    }

    /// Задержка через сервер или группу; `None` — нет ответа за `timeout_ms`.
    pub async fn proxy_delay(&self, name: &str, url: &str, timeout_ms: u32) -> Result<Option<u32>> {
        let path = format!("/proxies/{}/delay?url={}&timeout={timeout_ms}", enc(name), enc(url));
        let (status, body) = self.call_within(Method::GET, &path, None, Duration::from_millis(u64::from(timeout_ms) + 2_000)).await?;
        if !status.is_success() {
            return Ok(None);
        }
        let v: Value = serde_json::from_slice(&body)?;
        Ok(v["delay"].as_u64().filter(|d| *d > 0).map(|d| d as u32))
    }

    /// Задержка всех серверов группы разом.
    pub async fn group_delay(&self, group: &str, url: &str, timeout_ms: u32) -> Result<HashMap<String, u32>> {
        let path = format!("/group/{}/delay?url={}&timeout={timeout_ms}", enc(group), enc(url));
        let (status, body) = self.call_within(Method::GET, &path, None, Duration::from_millis(u64::from(timeout_ms) + 3_000)).await?;
        if !status.is_success() {
            return Ok(HashMap::new());
        }
        let v: Value = serde_json::from_slice(&body)?;
        Ok(v.as_object()
            .map(|m| m.iter().filter_map(|(k, d)| Some((k.clone(), d.as_u64().filter(|d| *d > 0)? as u32))).collect())
            .unwrap_or_default())
    }

    /// Перечитать конфиг без перезапуска: тумблер, списки, Kill Switch.
    pub async fn reload(&self, config: &Path) -> Result<()> {
        let body = json!({ "path": config.to_string_lossy() });
        let (status, text) = self.call_within(Method::PUT, "/configs?force=true", Some(&body), Duration::from_secs(20)).await?;
        if !status.is_success() {
            bail!("ядро не приняло конфиг ({status}): {}", String::from_utf8_lossy(&text));
        }
        Ok(())
    }

    /// Перечитать файл серверов после обновления подписки.
    pub async fn refresh_provider(&self, provider: &str) -> Result<()> {
        let (status, text) = self.call(Method::PUT, &format!("/providers/proxies/{}", enc(provider)), None).await?;
        if !status.is_success() {
            bail!("ядро не обновило серверы ({status}): {}", String::from_utf8_lossy(&text));
        }
        Ok(())
    }

    /// Потоковый ответ построчно: `/traffic`, `/connections`, `/logs`. Возвращается, когда поток закрылся.
    pub async fn stream<F: FnMut(Value) + Send>(&self, path: &str, mut on_item: F) -> Result<()> {
        let mut res = tokio::time::timeout(CALL_LIMIT, self.send(Method::GET, path, None)).await.context("ядро не ответило")??;
        if !res.status().is_success() {
            bail!("ядро ответило {} на {path}", res.status());
        }
        let mut buf: Vec<u8> = Vec::new();
        while let Some(frame) = res.body_mut().frame().await {
            let frame = frame?;
            if let Some(data) = frame.data_ref() {
                buf.extend_from_slice(data);
                while let Some(i) = buf.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=i).collect();
                    if let Ok(v) = serde_json::from_slice::<Value>(&line) {
                        on_item(v);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Строка журнала ядра без адресов сайтов: `--> youtube.com:443` превращается в `--> <адрес>`.
/// Журнал рабочей службы по брифу не хранит, куда ходил человек.
pub fn redact(line: &str) -> String {
    let line = redact_rule_payload(line);
    let mut out = String::with_capacity(line.len());
    let mut rest = line.as_str();
    while let Some(i) = rest.find("-->") {
        out.push_str(&rest[..i + 3]);
        let after = &rest[i + 3..];
        let trimmed = after.trim_start();
        let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        out.push_str(" <адрес>");
        rest = &trimmed[end..];
    }
    out.push_str(rest);
    scrub_addresses(&out)
}

/// Адреса в любом месте строки, в том числе в тексте ошибки: IP → `<ip>`, имена сайтов → `<адрес>`.
/// Свои адреса (127.x, подменные 198.18.0.0/15) остаются: по ним видно, откуда соединение.
fn scrub_addresses(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut token = String::new();
    for ch in line.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | ':' | '_' | '[' | ']') {
            token.push(ch);
        } else {
            out.push_str(&scrub_token(&token));
            token.clear();
            out.push(ch);
        }
    }
    out.push_str(&scrub_token(&token));
    out
}

fn scrub_token(token: &str) -> String {
    let body = token.trim_end_matches(':');
    let tail = &token[body.len()..];
    let t = body.trim_start_matches('[');
    let host = match t.split_once(']') {
        Some((h, _)) => h,
        None if t.matches(':').count() == 1 => t.split_once(':').map_or(t, |(h, _)| h),
        None => t,
    };
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        let own = ip.is_loopback() || ip.is_unspecified() || matches!(ip, std::net::IpAddr::V4(v4) if v4.octets()[0] == 198 && (v4.octets()[1] & 0xfe) == 18);
        return if own { token.to_string() } else { format!("<ip>{tail}") };
    }
    if looks_like_domain(host) {
        return format!("<адрес>{tail}");
    }
    token.to_string()
}

/// `www.youtube.com` — да; `v1.19.31`, `config.yaml`, `klick-vpn` — нет.
fn looks_like_domain(h: &str) -> bool {
    const FILES: [&str; 8] = ["yaml", "yml", "txt", "json", "exe", "mmdb", "dat", "log"];
    let labels: Vec<&str> = h.split('.').collect();
    let Some(tld) = labels.last() else { return false };
    labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty())
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_ascii_alphabetic())
        && !FILES.contains(&tld.to_ascii_lowercase().as_str())
}

/// `(match DomainSuffix/youtube.com)` → `(match DomainSuffix/…)`: тип правила полезен, домен — нет.
fn redact_rule_payload(line: &str) -> String {
    let Some(start) = line.find("(match ") else { return line.to_string() };
    let Some(len) = line[start..].find(')') else { return line.to_string() };
    let inner = &line[start..start + len];
    match inner.find('/') {
        Some(slash) if slash + 1 < inner.len() => format!("{}/…{}", &line[..start + slash], &line[start + len..]),
        _ => line.to_string(),
    }
}

/// Процентное кодирование для имён серверов в пути: пробелы, эмодзи, кириллица.
pub fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Работающий процесс ядра.
pub struct CoreProcess {
    pub api: CoreApi,
    pub pid: Option<u32>,
    kill: Option<oneshot::Sender<()>>,
    done: Option<oneshot::Receiver<()>>,
    #[cfg(windows)]
    _job: Job,
    /// macOS: pid-файл ядра, по нему следующий запуск завершит ядро, оставшееся после сбоя.
    #[cfg(unix)]
    pidfile: std::path::PathBuf,
    tail: Arc<Mutex<VecDeque<String>>>,
}

impl CoreProcess {
    /// Запускает ядро и ждёт, пока ответит API (до 10 с).
    /// `on_exit` вызывается, только если ядро завершилось само, а не по `stop`.
    pub async fn start(
        exe: &Path,
        home: &Path,
        config: &Path,
        api: CoreApi,
        verbose: bool,
        on_exit: impl FnOnce(Option<i32>) + Send + 'static,
    ) -> Result<Self> {
        if !exe.exists() {
            bail!("нет файла ядра {}", exe.display());
        }
        #[cfg(unix)]
        let pidfile = std::path::PathBuf::from(format!("{}.pid", api.pipe));
        #[cfg(unix)]
        crate::sys::kill_stale(&pidfile, exe);
        let mut cmd = tokio::process::Command::new(exe);
        cmd.arg("-d")
            .arg(home)
            .arg("-f")
            .arg(config)
            .current_dir(home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        for var in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] {
            cmd.env_remove(var);
        }
        let mut child = cmd.spawn().with_context(|| format!("не запустить {}", exe.display()))?;
        #[cfg(windows)]
        let job = Job::kill_on_close()?;
        #[cfg(windows)]
        if let Some(h) = child.raw_handle() {
            job.assign(h)?;
        }
        let pid = child.id();
        #[cfg(unix)]
        if let Some(pid) = pid {
            let _ = std::fs::write(&pidfile, pid.to_string());
        }

        let tail = Arc::new(Mutex::new(VecDeque::with_capacity(32)));
        for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>), child.stderr.take().map(|s| Box::new(s) as _)]
            .into_iter()
            .flatten()
        {
            let tail = tail.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = if verbose { line } else { redact(&line) };
                    if verbose || line.contains("level=warning") || line.contains("level=error") || line.contains("level=fatal") {
                        tracing::info!(target: "core", "{line}");
                    } else {
                        tracing::debug!(target: "core", "{line}");
                    }
                    let mut t = tail.lock().unwrap();
                    if t.len() == 32 {
                        t.pop_front();
                    }
                    t.push_back(line);
                }
            });
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait()? {
                let last = tail.lock().unwrap().iter().rev().take(3).cloned().collect::<Vec<_>>().join(" | ");
                bail!("ядро завершилось при запуске ({status}): {last}");
            }
            match api.version().await {
                Ok(_) => break,
                Err(_) if Instant::now() < deadline => tokio::time::sleep(Duration::from_millis(100)).await,
                Err(e) => {
                    let _ = child.kill().await;
                    return Err(e.context("ядро не ответило за 10 с"));
                }
            }
        }

        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let (done_tx, done_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            tokio::select! {
                status = child.wait() => {
                    let code = status.ok().and_then(|s| s.code());
                    let _ = done_tx.send(());
                    on_exit(code);
                }
                _ = kill_rx => {
                    // macOS: сначала просим ядро завершиться само — оно уберёт маршруты адаптера и сохранит кэш.
                    #[cfg(unix)]
                    if let Some(pid) = child.id() {
                        unsafe {
                            libc::kill(pid as libc::pid_t, libc::SIGTERM);
                        }
                        if tokio::time::timeout(Duration::from_secs(3), child.wait()).await.is_ok() {
                            let _ = done_tx.send(());
                            return;
                        }
                    }
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = done_tx.send(());
                }
            }
        });

        Ok(CoreProcess {
            api,
            pid,
            kill: Some(kill_tx),
            done: Some(done_rx),
            #[cfg(windows)]
            _job: job,
            #[cfg(unix)]
            pidfile,
            tail,
        })
    }

    /// Последние строки журнала ядра: для кода ошибки и отчёта.
    pub fn tail(&self) -> Vec<String> {
        self.tail.lock().unwrap().iter().cloned().collect()
    }

    pub async fn stop(mut self) {
        if let Some(kill) = self.kill.take() {
            let _ = kill.send(());
        }
        if let Some(done) = self.done.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), done).await;
        }
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.pidfile);
    }
}

pub fn random_secret() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}


#[cfg(test)]
mod tests {
    use super::{enc, redact};

    #[test]
    fn destinations_are_redacted() {
        assert_eq!(
            redact(r#"level=warning msg="[TCP] dial klick-vpn (match Match/) 127.0.0.1:5 --> youtube.com:443 error: timeout""#),
            r#"level=warning msg="[TCP] dial klick-vpn (match Match/) 127.0.0.1:5 --> <адрес> error: timeout""#
        );
        assert_eq!(
            redact("[TCP] dial klick-vpn (match DomainSuffix/api.ipify.org) 198.18.0.1:5 --> api.ipify.org:443 error: x"),
            "[TCP] dial klick-vpn (match DomainSuffix/…) 198.18.0.1:5 --> <адрес> error: x"
        );
        assert_eq!(redact("no arrows here"), "no arrows here");
    }

    #[test]
    fn error_text_loses_addresses_too() {
        assert_eq!(
            redact("[TCP] dial klick-vpn (match RuleSet/x) 198.18.0.1:5 --> www.gstatic.com:443 error: 192.168.238.20:1080 connect error: dial tcp 192.168.238.20:1080: i/o timeout"),
            "[TCP] dial klick-vpn (match RuleSet/…) 198.18.0.1:5 --> <адрес> error: <ip> connect error: dial tcp <ip>: i/o timeout"
        );
        assert_eq!(redact("lookup www.example.com: no such host"), "lookup <адрес>: no such host");
        assert_eq!(redact("dial [2001:db8::1]:443 failed"), "dial <ip> failed");
        assert_eq!(redact("Mihomo Meta v1.19.31, config.yaml, 127.0.0.1:9090, klick-vpn"), "Mihomo Meta v1.19.31, config.yaml, 127.0.0.1:9090, klick-vpn");
        assert_eq!(redact("time=\"2026-09-26T06:34:54+04:00\""), "time=\"2026-09-26T06:34:54+04:00\"");
    }

    #[test]
    fn names_are_percent_encoded() {
        assert_eq!(enc("NL Amsterdam"), "NL%20Amsterdam");
        assert_eq!(enc("🇳🇱"), "%F0%9F%87%B3%F0%9F%87%B1");
        assert_eq!(enc("klick-vpn"), "klick-vpn");
    }
}
