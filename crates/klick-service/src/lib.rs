//! Служба kl!ck библиотекой: движок (ядро, страж связи, подписки, Kill Switch) и канал управления.
//! На Windows и macOS её запускает `main.rs` — службой Windows или демоном launchd. На iOS служб нет:
//! тот же движок работает внутри приложения (`crates/klick-ios`), а ядро VPN — в расширении.

pub mod core;
pub mod engine;
mod hwid;
mod ipcheck;
pub mod logring;
pub mod paths;
pub mod pipe;
mod probe;
mod speedtest;
mod storage;
mod subs;
mod update;

// Windows: WFP, реестр, службы Windows.
// Профили браузеров по путям Windows; на macOS расширения пока не проверяются.
#[cfg(windows)]
mod browsers;
#[cfg(windows)]
pub mod killswitch;
#[cfg(windows)]
mod kswatch;
#[cfg(windows)]
mod neighbors;
#[cfg(windows)]
mod netwatch;
#[cfg(windows)]
mod programs;
#[cfg(windows)]
pub mod svc;
#[cfg(windows)]
mod userproxy;
#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub use win as sys;

// macOS (и Linux для разработки): launchd, networksetup, ядро-страж Kill Switch. Те же имена модулей, что у Windows.
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/killswitch.rs"]
mod killswitch;
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/neighbors.rs"]
mod neighbors;
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/netconf.rs"]
mod netconf;
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/netwatch.rs"]
mod netwatch;
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/pf.rs"]
mod pf;
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/programs.rs"]
mod programs;
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/launchd.rs"]
pub mod svc;
#[cfg(all(unix, not(target_os = "ios")))]
#[path = "unix/sys.rs"]
pub mod sys;

// iOS: ни службы, ни прав администратора, ни чужих процессов. Прокси, DNS и pf системы не трогаются,
// программ и соседей не видно — те же имена модулей, но без системных настроек (см. `ios`).
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "ios")]
#[path = "unix/killswitch.rs"]
mod killswitch;
#[cfg(target_os = "ios")]
#[path = "unix/netwatch.rs"]
mod netwatch;
#[cfg(target_os = "ios")]
use ios::{neighbors, netconf, pf, programs, sys};

use anyhow::Result;
use paths::Paths;
use std::path::PathBuf;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{filter::LevelFilter, Layer};

/// Журнал рабочей службы: `%ProgramData%\klick\logs\service.log` (на macOS —
/// `/Library/Application Support/klick/logs/service.log`), короткий, без адресов сайтов.
pub fn init_file_log(paths: &Paths) -> Result<()> {
    let file = RotatingLog::open(paths.logs.join("service.log"))?;
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(std::sync::Mutex::new(file)).with_filter(LevelFilter::INFO))
        .with(logring::RingLayer.with_filter(LevelFilter::INFO))
        .try_init();
    Ok(())
}

/// Журнал, который не растёт без меры: дорос до 10 МБ — становится `service.log.1` (прежний удаляется).
/// У других клиентов на mihomo ядро в плохом состоянии сыпало строками до десятков гигабайт.
struct RotatingLog {
    path: PathBuf,
    file: std::fs::File,
    size: u64,
}

impl RotatingLog {
    const LIMIT: u64 = 10 << 20;

    fn open(path: PathBuf) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut log = RotatingLog { path, file, size };
        if log.size > Self::LIMIT {
            log.rotate();
        }
        Ok(log)
    }

    fn rotate(&mut self) {
        let old = self.path.with_extension("log.1");
        let _ = std::fs::remove_file(&old);
        // Файл открыт и у нас: Windows разрешает переименование, потому что std открывает с FILE_SHARE_DELETE.
        let _ = std::fs::rename(&self.path, &old);
        if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            self.file = f;
            self.size = 0;
        }
    }
}

impl std::io::Write for RotatingLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.size + buf.len() as u64 > Self::LIMIT {
            self.rotate();
        }
        let n = self.file.write(buf)?;
        self.size += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    #[test]
    fn log_rotates_at_the_limit() {
        let dir = std::env::temp_dir().join(format!("klick-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("service.log");
        let mut log = super::RotatingLog::open(path.clone()).unwrap();
        let line = vec![b'x'; 1 << 20];
        for _ in 0..12 {
            log.write_all(&line).unwrap();
        }
        let size = |p: &std::path::Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        assert!(size(&path) <= super::RotatingLog::LIMIT && size(&path.with_extension("log.1")) <= super::RotatingLog::LIMIT);
        assert!(size(&path) + size(&path.with_extension("log.1")) >= 11 << 20, "старое не потеряно сверх одной смены");
        drop(log);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
