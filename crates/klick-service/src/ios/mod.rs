//! iOS: службы нет. Движок работает в приложении, ядро VPN — в расширении Network Extension, проверочное
//! ядро (задержка, локальная проверка серверов) — в самом приложении. Туннель и ядро в процессе умеют
//! только Swift и Go: их даёт приложение через [`Host`] при запуске (`crates/klick-ios`).
//!
//! Системных настроек, которые kl!ck меняет на macOS (прокси, DNS, pf), на iOS нет, программ и соседей
//! не видно — модули с теми же именами ничего не делают.

pub mod sys;

use crate::core::CoreApi;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Что даёт приложение. В тестах его нет: без него туннель и ядро не запускаются.
pub struct Host {
    /// Поднять туннель (конфиг — `config.yaml` в домашней папке ядра) и дождаться, пока система его подключит.
    pub tunnel_start: fn() -> Result<(), String>,
    /// Перечитать конфиг в работающем туннеле целиком — вместе с входом управления и паролем.
    pub tunnel_reload: fn() -> Result<(), String>,
    pub tunnel_stop: fn(),
    pub tunnel_connected: fn() -> bool,
    /// Ядро в процессе приложения: проверочное и локальная проверка серверов.
    pub core_start: fn(home: &str, config: &str) -> Result<(), String>,
    pub core_stop: fn(),
    /// `identifierForVendor`: постоянный, пока kl!ck стоит на устройстве.
    pub device_id: fn() -> Option<String>,
}

static HOST: OnceLock<Host> = OnceLock::new();

pub fn init(host: Host) {
    let _ = HOST.set(host);
}

pub(crate) fn host() -> Option<&'static Host> {
    HOST.get()
}

/// Конфиг туннеля. Остальные ядра (проверочное, локальная проверка) пишут свои файлы и работают в приложении.
pub const TUNNEL_CONFIG: &str = "config.yaml";
/// Конфиг, который сейчас у ядра в расширении: его пишет расширение, применив конфиг.
pub const APPLIED_CONFIG: &str = "config.applied";

type OnExit = Box<dyn FnOnce(Option<i32>) + Send>;

/// Кого позвать, когда туннель остановился не по команде движка.
static ON_EXIT: Mutex<Option<OnExit>> = Mutex::new(None);

/// Туннель остановился сам. `by_user` — его выключили в Настройках или его место занял другой VPN:
/// движок не включает его снова. Иначе (сбой расширения) — как упавшее ядро на macOS.
pub fn tunnel_stopped(by_user: bool) {
    if let Some(f) = ON_EXIT.lock().unwrap().take() {
        f(if by_user { Some(0) } else { None });
    }
}

/// Вход управления и пароль ядра в расширении — из конфига, который оно применило. Приложение могли
/// закрыть, пока VPN работал: открывшись снова, движок подключается к тому же ядру, а не перезапускает его.
pub fn applied_controller(home: &Path) -> Option<(String, String, u16, u16)> {
    let text = std::fs::read(home.join(APPLIED_CONFIG)).ok()?;
    let cfg: serde_json::Value = serde_json::from_slice(&text).ok()?;
    let port = |name: &str| cfg["listeners"].as_array()?.iter().find(|l| l["name"] == name)?["port"].as_u64().map(|p| p as u16);
    Some((cfg["external-controller"].as_str()?.to_string(), cfg["secret"].as_str()?.to_string(), port("klick-check-vpn")?, port("klick-check-direct")?))
}

/// Ядро в процессе приложения одно на процесс (у mihomo всё глобальное): проверочное и локальная
/// проверка серверов ждут друг друга.
fn in_process_lock() -> Arc<tokio::sync::Mutex<()>> {
    static LOCK: OnceLock<Arc<tokio::sync::Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(Default::default).clone()
}

/// Ядро на iOS: туннель в расширении или ядро в процессе приложения. Тот же интерфейс, что у процесса
/// ядра на Windows и macOS.
pub struct CoreProcess {
    pub api: CoreApi,
    pub pid: Option<u32>,
    tunnel: bool,
    _busy: Option<tokio::sync::OwnedMutexGuard<()>>,
}

impl CoreProcess {
    /// Запускает ядро и ждёт, пока ответит API. `on_exit` вызывается, только если туннель остановился сам.
    pub async fn start(
        _exe: &Path,
        home: &Path,
        config: &Path,
        api: CoreApi,
        _verbose: bool,
        on_exit: impl FnOnce(Option<i32>) + Send + 'static,
    ) -> Result<Self> {
        let host = host().context("приложение не подключило ядро")?;
        let tunnel = config.file_name().is_some_and(|n| n == TUNNEL_CONFIG);
        let mut busy = None;
        if tunnel {
            let applied = std::fs::read(home.join(APPLIED_CONFIG)).ok();
            let wanted = std::fs::read(config).ok();
            let result = if (host.tunnel_connected)() {
                // Туннель уже работает (приложение открыли снова или VPN включили в Настройках). Тот же
                // конфиг — просто подключаемся: перечитанный конфиг оборвал бы соединения.
                if applied.is_some() && applied == wanted {
                    Ok(())
                } else {
                    tokio::task::spawn_blocking(host.tunnel_reload).await?
                }
            } else {
                tokio::task::spawn_blocking(host.tunnel_start).await?
            };
            if let Err(e) = result {
                bail!("туннель не поднялся: {e}");
            }
            *ON_EXIT.lock().unwrap() = Some(Box::new(on_exit));
        } else {
            let guard = tokio::time::timeout(Duration::from_secs(20), in_process_lock().lock_owned()).await.context("проверочное ядро занято другой проверкой")?;
            busy = Some(guard);
            let (home, config) = (home.to_string_lossy().into_owned(), config.to_string_lossy().into_owned());
            if let Err(e) = tokio::task::spawn_blocking(move || (host.core_start)(&home, &config)).await? {
                bail!("ядро не запустилось: {e}");
            }
        }
        let process = CoreProcess { api, pid: None, tunnel, _busy: busy };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(if tunnel { 20 } else { 10 });
        loop {
            match process.api.version().await {
                Ok(_) => return Ok(process),
                Err(_) if tokio::time::Instant::now() < deadline => tokio::time::sleep(Duration::from_millis(100)).await,
                Err(e) => {
                    process.stop().await;
                    return Err(e.context("ядро не ответило"));
                }
            }
        }
    }

    /// Строки журнала ядра для отчёта. Ядро на iOS пишет не в вывод процесса, а в API (`/logs`).
    pub fn tail(&self) -> Vec<String> {
        Vec::new()
    }

    pub async fn stop(self) {
        let Some(host) = host() else { return };
        if self.tunnel {
            // Остановка по команде движка — не сбой: не сообщаем о ней.
            ON_EXIT.lock().unwrap().take();
            let _ = tokio::task::spawn_blocking(host.tunnel_stop).await;
        } else {
            let _ = tokio::task::spawn_blocking(host.core_stop).await;
        }
    }
}

/// Системного прокси и DNS на iOS kl!ck не трогает: весь трафик и так идёт в туннель.
pub mod netconf {
    use klick_proto::SystemProxy;
    use std::path::Path;

    pub const DNS_ADDR: &str = "198.18.0.2";
    pub const PROXY_MARKER: &str = "sysproxy.json";
    pub const DNS_MARKER: &str = "dns.json";
    pub const BYPASS: [&str; 0] = [];

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Outcome {
        Done,
        Nothing,
        Failed,
    }

    impl Outcome {
        pub fn ok(self) -> bool {
            self != Outcome::Failed
        }

        pub fn done(self) -> bool {
            self == Outcome::Done
        }
    }

    pub struct Restored {
        pub proxy: Outcome,
        pub dns: Outcome,
    }

    impl Restored {
        pub fn failed(&self) -> bool {
            false
        }
    }

    pub fn network_set() -> Option<String> {
        None
    }

    pub fn proxy_apply(_spec: &SystemProxy, _marker: &Path) -> Outcome {
        Outcome::Nothing
    }

    pub fn proxy_clear(_marker: &Path) -> Outcome {
        Outcome::Nothing
    }

    pub fn has_leftover(_marker: &Path) -> bool {
        false
    }

    pub fn dns_apply(_marker: &Path) -> Outcome {
        Outcome::Nothing
    }

    pub fn dns_restore(_marker: &Path) -> Outcome {
        Outcome::Nothing
    }

    pub fn restore_leftovers(_data: &Path) -> Restored {
        Restored { proxy: Outcome::Nothing, dns: Outcome::Nothing }
    }
}

/// Брандмауэра для программ на iOS нет. «Весь трафик только через VPN» держит сама система
/// (`includeAllNetworks` у туннеля), а не правила kl!ck.
pub mod pf {
    use std::path::Path;

    pub fn apply(_data: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn active() -> bool {
        true
    }

    pub fn clear(_data: &Path) -> bool {
        false
    }
}

/// iOS не говорит, какая программа открыла соединение.
pub mod programs {
    use klick_proto::ProgramView;
    use std::path::{Path, PathBuf};

    pub fn scan(_own: &[PathBuf]) -> Vec<ProgramView> {
        Vec::new()
    }

    pub fn folder_name(_folder: &Path) -> Option<String> {
        None
    }
}

/// Чужие процессы на iOS не видны. Другой VPN система и так не даст включить вместе с kl!ck.
pub mod neighbors {
    pub use klick_proto::NeighborView as Neighbor;
    use std::path::Path;

    pub fn scan(_own_core: &Path) -> Vec<Neighbor> {
        Vec::new()
    }
}
