//! Смена сети и пробуждение на macOS: раз в 2 секунды служба сверяет адреса сетевых интерфейсов и набор
//! сетевых служб (смена размещения, Location, не меняет интерфейс и IP, но меняет службы), а сон замечает
//! по расхождению часов: с учётом сна они идут, без сна — стоят. Скачок системных часов сном не считается.
//! Страж сразу проверяет связь, а не ждёт плановой проверки.

use crate::netconf;
use crate::sys::{self, IfAddr, TUN_ADDR};
use std::net::IpAddr;
use std::time::Duration;
use tokio::sync::mpsc;

const EVERY: Duration = Duration::from_secs(2);
/// Часы разошлись больше, чем на это, — компьютер спал.
const SLEPT: Duration = Duration::from_secs(10);

/// Подписка на изменения сети; живёт, пока жив объект.
pub struct NetWatch {
    task: tokio::task::JoinHandle<()>,
}

impl NetWatch {
    /// В канал приходит сигнал на каждое изменение: 1 — сеть сменилась или сон был долгим (старые соединения
    /// мертвы: NAT роутера и сервера их забыл), 0 — короткий сон на той же сети.
    pub fn start(tx: mpsc::UnboundedSender<u64>) -> anyhow::Result<Self> {
        let task = tokio::spawn(async move {
            let mut last = Fingerprint::read(None);
            let mut before = clocks();
            loop {
                tokio::time::sleep(EVERY).await;
                let now = Fingerprint::read(Some(&last));
                let after = clocks();
                let gap = sleep_gap(before, after);
                before = after;
                if now != last || gap > SLEPT {
                    if gap > SLEPT {
                        tracing::info!("компьютер проснулся");
                    }
                    let moved = now != last || gap >= crate::engine::LONG_SLEEP;
                    last = now;
                    if tx.send(u64::from(moved)).is_err() {
                        break;
                    }
                }
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

/// Есть ли вообще выход в сеть: хоть один поднятый интерфейс с адресом (без петли, туннелей и link-local IPv6).
/// Без сети страж не проверяет сервер и не переподключается.
pub fn has_network() -> bool {
    !physical(sys::interfaces()).is_empty()
}

/// Состояние сети: что сменилось, то и повод проверить связь и досыпать настройки новым службам.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fingerprint {
    /// Какие интерфейсы подняты и с какими адресами.
    addrs: Vec<(String, IpAddr)>,
    /// ID текущего размещения и его сетевых служб; пусто, пока прочитать не удавалось.
    netset: String,
}

impl Fingerprint {
    fn read(prev: Option<&Fingerprint>) -> Self {
        Self::of(sys::interfaces(), netconf::network_set(), prev)
    }

    /// Не прочитался набор служб (настройки заняты) — оставляем прежний: ошибка чтения не смена сети.
    fn of(interfaces: Vec<IfAddr>, netset: Option<String>, prev: Option<&Fingerprint>) -> Self {
        let netset = netset.or_else(|| prev.map(|p| p.netset.clone())).unwrap_or_default();
        Fingerprint { addrs: physical(interfaces), netset }
    }
}

/// Подняты ли интерфейсы с адресами, по которым ходит сеть. Туннели (свой и чужие `utun`) и локальная
/// петля не считаются: они меняются вместе с VPN, а не с сетью. Link-local IPv6 есть у любого интерфейса.
fn physical(interfaces: Vec<IfAddr>) -> Vec<(String, IpAddr)> {
    let mut v: Vec<(String, IpAddr)> = interfaces
        .into_iter()
        .filter(|i| i.up && !i.name.starts_with("lo") && !i.name.starts_with("utun") && i.addr != IpAddr::V4(TUN_ADDR))
        .filter(|i| !matches!(i.addr, IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) == 0xfe80))
        .map(|i| (i.name, i.addr))
        .collect();
    v.sort();
    v
}

/// Время с учётом сна и время бодрствования. Во сне первое идёт, второе стоит; перевод системных
/// часов не двигает ни то, ни другое.
type Clocks = (Duration, Duration);

fn clocks() -> Clocks {
    #[cfg(target_os = "macos")]
    {
        let read = |id: libc::clockid_t| {
            let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
            unsafe { libc::clock_gettime(id, &mut ts) };
            Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32)
        };
        // mach_continuous_time и mach_absolute_time.
        (read(libc::CLOCK_MONOTONIC_RAW), read(libc::CLOCK_UPTIME_RAW))
    }
    // Не macOS (сборка для разработки): сон видно по настоящим часам против монотонных; перевод часов даст ложный сигнал.
    #[cfg(not(target_os = "macos"))]
    {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let wall = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        (wall, START.get_or_init(std::time::Instant::now).elapsed())
    }
}

/// Сколько компьютер проспал между замерами: время с учётом сна ушло вперёд сильнее, чем время бодрствования.
fn sleep_gap(before: Clocks, after: Clocks) -> Duration {
    let total = after.0.saturating_sub(before.0);
    let awake = after.1.saturating_sub(before.1);
    total.saturating_sub(awake)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn up(name: &str, addr: IpAddr) -> IfAddr {
        IfAddr { name: name.into(), addr, up: true }
    }

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn physical_skips_loopback_tunnels_and_link_local() {
        let down = IfAddr { name: "en1".into(), addr: v4(10, 0, 0, 9), up: false };
        let found = physical(vec![
            up("lo0", v4(127, 0, 0, 1)),
            up("utun3", v4(10, 8, 0, 2)),
            up("utun5", IpAddr::V4(TUN_ADDR)),
            up("en0", IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4))),
            down,
            up("en0", v4(192, 168, 1, 5)),
        ]);
        assert_eq!(found, vec![("en0".to_string(), v4(192, 168, 1, 5))]);
        assert!(physical(vec![up("lo0", v4(127, 0, 0, 1)), up("en0", IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)))]).is_empty(), "без сети");
    }

    #[test]
    fn location_switch_with_same_interface_changes_the_fingerprint() {
        let ifs = || vec![up("en0", v4(192, 168, 1, 5))];
        let a = Fingerprint::of(ifs(), Some("setA:wifi+".into()), None);
        let same = Fingerprint::of(ifs(), Some("setA:wifi+".into()), Some(&a));
        let b = Fingerprint::of(ifs(), Some("setB:wifi+".into()), Some(&a));
        let off = Fingerprint::of(ifs(), Some("setA:wifi-".into()), Some(&a));
        assert_eq!(a, same);
        assert_ne!(a, b, "смена размещения");
        assert_ne!(a, off, "службу выключили");
    }

    #[test]
    fn unreadable_service_set_is_not_a_change() {
        let a = Fingerprint::of(vec![up("en0", v4(192, 168, 1, 5))], Some("setA:wifi+".into()), None);
        let busy = Fingerprint::of(vec![up("en0", v4(192, 168, 1, 5))], None, Some(&a));
        assert_eq!(a, busy);
    }

    #[test]
    fn sleep_is_the_gap_between_clocks_not_a_clock_jump() {
        let s = Duration::from_secs;
        // Обычные 2 с: оба идут одинаково.
        assert!(!(sleep_gap((s(100), s(50)), (s(102), s(52))) > SLEPT));
        // Сон на час: с учётом сна — вперёд на час, бодрствование — на 2 с.
        assert!((sleep_gap((s(100), s(50)), (s(3702), s(52))) > SLEPT));
        // Небольшая разница не считается.
        assert!(!(sleep_gap((s(100), s(50)), (s(109), s(52))) > SLEPT));
    }
}
