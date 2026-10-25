//! «Проверить скорость»: сколько скачивается и отдаётся через VPN за несколько секунд. Идёт через
//! вход проверки VPN (`check_vpn_port`), как «Как вас видят сайты», — всегда через выбранный сервер,
//! какие бы ни были списки. Файлы — с speed.cloudflare.com: их же берёт проверка серверов.

use std::time::{Duration, Instant};

/// Больше 50 МБ за запрос Cloudflare не отдаёт (403) — качаем кусками подряд, пока не выйдет время.
const DOWN_URL: &str = "https://speed.cloudflare.com/__down?bytes=25000000";
const UP_URL: &str = "https://speed.cloudflare.com/__up";
/// Сколько качаем: хватит, чтобы скорость устоялась и на медленном канале.
const DOWN_TIME: Duration = Duration::from_secs(8);
/// Первые полсекунды TCP разгоняется — их в скорость не считаем.
const WARMUP: Duration = Duration::from_millis(500);
const UP_BYTES: usize = 4 << 20;
const UP_LIMIT: Duration = Duration::from_secs(20);

fn client(port: u16, timeout: Duration) -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).ok()?)
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .user_agent(concat!("klick/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()
}

fn mbps(bytes: u64, secs: f64) -> Option<f32> {
    (bytes > 0 && secs > 0.0).then(|| (bytes as f64 * 8.0 / secs / 1e6) as f32)
}

/// Скачивание, Мбит/с; `None` — ничего не скачалось.
pub async fn download(port: u16) -> Option<f32> {
    let c = client(port, DOWN_TIME + Duration::from_secs(10))?;
    let started = Instant::now();
    let mut bytes = 0u64;
    let mut warm: Option<(Instant, u64)> = None;
    'pieces: while started.elapsed() < DOWN_TIME {
        let Ok(Ok(resp)) = tokio::time::timeout(DOWN_TIME.saturating_sub(started.elapsed()), c.get(DOWN_URL).send()).await else { break };
        let Ok(mut resp) = resp.error_for_status() else { break };
        while let Some(left) = DOWN_TIME.checked_sub(started.elapsed()) {
            match tokio::time::timeout(left, resp.chunk()).await {
                Ok(Ok(Some(chunk))) => {
                    bytes += chunk.len() as u64;
                    if warm.is_none() && started.elapsed() >= WARMUP {
                        warm = Some((Instant::now(), bytes));
                    }
                }
                Ok(Ok(None)) => continue 'pieces,
                _ => break 'pieces,
            }
        }
    }
    // Разгон не успел закончиться (всё скачалось сразу) — считаем от начала.
    let (from, base) = warm.filter(|(_, b)| bytes > *b).unwrap_or((started, 0));
    mbps(bytes - base, from.elapsed().as_secs_f64())
}

/// Отдача, Мбит/с. Сначала маленький запрос — соединение и TLS готовы, время отдачи их не включает.
pub async fn upload(port: u16) -> Option<f32> {
    let c = client(port, UP_LIMIT)?;
    c.get("https://speed.cloudflare.com/__down?bytes=0").send().await.ok()?;
    let started = Instant::now();
    let resp = c.post(UP_URL).body(vec![0u8; UP_BYTES]).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    mbps(UP_BYTES as u64, started.elapsed().as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::mbps;

    /// Настоящий замер через локальный прокси: `KLICK_SPEED_PORT=7890 cargo test -p klick-service real_speed -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn real_speed_through_local_proxy() {
        let port: u16 = std::env::var("KLICK_SPEED_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7890);
        let down = super::download(port).await;
        let up = super::upload(port).await;
        println!("↓ {down:?} ↑ {up:?} Мбит/с");
        assert!(down.is_some());
    }

    #[test]
    fn megabits_per_second() {
        assert_eq!(mbps(1_250_000, 1.0), Some(10.0));
        assert_eq!(mbps(0, 1.0), None);
        assert_eq!(mbps(10, 0.0), None);
    }
}
