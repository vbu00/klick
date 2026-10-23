//! Служба kl!ck. Рабочий запуск — через службы Windows или launchd на macOS (`run`),
//! для разработки — `console`.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use klick_service::paths::{Paths, Profile};
#[cfg(windows)]
use klick_service::killswitch;
use klick_service::{engine, logring, pipe, svc, sys};
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
