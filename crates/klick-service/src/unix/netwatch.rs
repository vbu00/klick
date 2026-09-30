//! Смена сети и пробуждение на macOS: раз в 2 секунды служба сверяет адреса сетевых интерфейсов,
//! а сон замечает по скачку часов (монотонные часы во сне стоят, настоящие — идут).
//! Страж сразу проверяет связь, а не ждёт плановой проверки.

use crate::sys::{self, TUN_ADDR};
use std::net::IpAddr;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;

const EVERY: Duration = Duration::from_secs(2);

/// Подписка на изменения сети; живёт, пока жив объект.
pub struct NetWatch {
    task: tokio::task::JoinHandle<()>,
}

impl NetWatch {
    /// В канал приходит сигнал на каждое изменение (значение не важно).
    pub fn start(tx: mpsc::UnboundedSender<u64>) -> anyhow::Result<Self> {
        let task = tokio::spawn(async move {
            let mut last = fingerprint();
            let (mut mono, mut wall) = (Instant::now(), SystemTime::now());
            loop {
                tokio::time::sleep(EVERY).await;
                let now = fingerprint();
                let slept = SystemTime::now().duration_since(wall).unwrap_or_default().saturating_sub(mono.elapsed()) > Duration::from_secs(10);
                if now != last || slept {
                    if slept {
                        tracing::info!("компьютер проснулся");
                    }
                    last = now;
                    if tx.send(0).is_err() {
                        break;
                    }
                }
                (mono, wall) = (Instant::now(), SystemTime::now());
            }
        });
        Ok(NetWatch { task })
    }
}

impl Drop for NetWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Какие интерфейсы подняты и с какими адресами. Туннели (свой и чужие `utun`) и локальная петля
/// не считаются: они меняются вместе с VPN, а не с сетью.
fn fingerprint() -> Vec<(String, IpAddr)> {
    let mut v: Vec<(String, IpAddr)> = sys::interfaces()
        .into_iter()
        .filter(|i| i.up && !i.name.starts_with("lo") && !i.name.starts_with("utun") && i.addr != IpAddr::V4(TUN_ADDR))
        .filter(|i| !matches!(i.addr, IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) == 0xfe80))
        .map(|i| (i.name, i.addr))
        .collect();
    v.sort();
    v
}
