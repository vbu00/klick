//! Служба kl!ck. Рабочий запуск — через службы Windows или launchd на macOS (`run`),
//! для разработки — `console`.

mod core;
mod engine;
mod hwid;
mod ipcheck;
mod logring;
mod paths;
mod pipe;
mod storage;
mod subs;

// Windows: WFP, реестр, службы Windows.
// Профили браузеров по путям Windows; на macOS расширения пока не проверяются.
#[cfg(windows)]
mod browsers;
#[cfg(windows)]
mod killswitch;
#[cfg(windows)]
mod kswatch;
#[cfg(windows)]
mod neighbors;
#[cfg(windows)]
mod netwatch;
#[cfg(windows)]
mod programs;
#[cfg(windows)]
mod svc;
#[cfg(windows)]
mod userproxy;
#[cfg(windows)]
mod win;
#[cfg(windows)]
use win as sys;

// macOS: launchd, networksetup, ядро-страж Kill Switch. Те же имена модулей, что у Windows.
#[cfg(unix)]
#[path = "unix/killswitch.rs"]
mod killswitch;
#[cfg(unix)]
#[path = "unix/neighbors.rs"]
mod neighbors;
#[cfg(unix)]
#[path = "unix/netconf.rs"]
mod netconf;
#[cfg(unix)]
#[path = "unix/netwatch.rs"]
mod netwatch;
#[cfg(unix)]
#[path = "unix/pf.rs"]
mod pf;
#[cfg(unix)]
#[path = "unix/programs.rs"]
mod programs;
#[cfg(unix)]
#[path = "unix/launchd.rs"]
mod svc;
#[cfg(unix)]
#[path = "unix/sys.rs"]
mod sys;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use paths::{Paths, Profile};
use std::path::PathBuf;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{filter::LevelFilter, EnvFilter, Layer};

#[derive(Parser)]
#[command(name = "klick-service", version, about = "Служба kl!ck")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Запустить в консоли для разработки: свой канал, свои порты, без TUN и брандмауэра.
    Console {
        /// Папка проекта с resources; по умолчанию ищется вверх от exe.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Разрешить адаптер TUN (нужны права администратора).
        #[arg(long)]
        allow_tun: bool,
        /// Разрешить Kill Switch: фильтры WFP на Windows, ядро-страж на macOS (нужны права администратора).
        #[arg(long, alias = "allow-ks")]
        allow_wfp: bool,
    },
    /// Точка входа службы; запускает Windows или launchd.
    Run,
    /// Зарегистрировать службу и запустить её.
    Install,
    /// Остановить и удалить службу.
    Uninstall {
        /// Удалить и данные: подключения, настройки, журнал.
        #[cfg(unix)]
        #[arg(long)]
        wipe: bool,
    },
    /// Убрать все фильтры Kill Switch из брандмауэра.
    #[cfg(windows)]
    CleanupWfp,
    /// Вернуть системный прокси и DNS, которые поменял kl!ck, и снять правила Kill Switch в pf
    /// (после сбоя или перед удалением).
    #[cfg(unix)]
    CleanupNetwork,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Console { root, allow_tun, allow_wfp } => console(root, allow_tun, allow_wfp),
        Cmd::Run => svc::run_dispatcher(),
        Cmd::Install => svc::install(),
        #[cfg(windows)]
        Cmd::Uninstall {} => svc::uninstall(),
        #[cfg(unix)]
        Cmd::Uninstall { wipe } => svc::uninstall(wipe),
        #[cfg(windows)]
        Cmd::CleanupWfp => {
            let removed = killswitch::Wfp::open()?.clear()?;
            println!("фильтров Kill Switch удалено: {removed}");
            Ok(())
        }
        #[cfg(unix)]
        Cmd::CleanupNetwork => svc::cleanup_network(),
    }
}

fn console(root: Option<PathBuf>, allow_tun: bool, allow_wfp: bool) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .with(logring::RingLayer.with_filter(LevelFilter::INFO))
        .init();
    if (allow_tun || allow_wfp) && !sys::is_elevated() {
        bail!("--allow-tun и --allow-wfp требуют запуска от администратора{}", if cfg!(windows) { "" } else { " (sudo)" });
    }
    #[cfg(unix)]
    sys::raise_fd_limit();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let paths = Paths::development(root)?;
        let profile = Profile::development(allow_tun, allow_wfp);
        let (engine, _task) = engine::spawn(paths, profile.clone())?;
        let server = tokio::spawn(pipe::serve(profile.pipe.clone(), false, engine.clone()));
        tracing::info!("служба для разработки работает; Ctrl+C — остановить");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            r = server => { if let Ok(Err(e)) = r { tracing::error!("канал управления упал: {e:#}"); } }
        }
        engine.shutdown().await;
        anyhow::Ok(())
    })
}

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
