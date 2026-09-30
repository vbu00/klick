//! Служба kl!ck. Рабочий запуск — через службы Windows или launchd на macOS (`run`),
//! для разработки — `console`.

mod browsers;
mod core;
mod engine;
mod ipcheck;
mod logring;
mod paths;
mod pipe;
mod storage;
mod subs;

// Windows: WFP, реестр, службы Windows.
#[cfg(windows)]
mod killswitch;
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
    let file = std::fs::OpenOptions::new().create(true).append(true).open(paths.logs.join("service.log"))?;
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(std::sync::Mutex::new(file)).with_filter(LevelFilter::INFO))
        .with(logring::RingLayer.with_filter(LevelFilter::INFO))
        .try_init();
    Ok(())
}
