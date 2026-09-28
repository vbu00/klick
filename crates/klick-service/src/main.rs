//! Служба kl!ck. Рабочий запуск — через Windows (`run`), для разработки — `console`.

mod core;
mod engine;
mod ipcheck;
mod killswitch;
mod logring;
mod neighbors;
mod netwatch;
mod paths;
mod pipe;
mod programs;
mod storage;
mod subs;
mod svc;
mod userproxy;
mod win;

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
        /// Разрешить фильтры Kill Switch (нужны права администратора).
        #[arg(long)]
        allow_wfp: bool,
    },
    /// Точка входа службы; запускает Windows.
    Run,
    /// Зарегистрировать службу и запустить её.
    Install,
    /// Остановить и удалить службу.
    Uninstall,
    /// Убрать все фильтры Kill Switch из брандмауэра.
    CleanupWfp,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Console { root, allow_tun, allow_wfp } => console(root, allow_tun, allow_wfp),
        Cmd::Run => svc::run_dispatcher(),
        Cmd::Install => svc::install(),
        Cmd::Uninstall => svc::uninstall(),
        Cmd::CleanupWfp => {
            let removed = killswitch::Wfp::open()?.clear()?;
            println!("фильтров Kill Switch удалено: {removed}");
            Ok(())
        }
    }
}

fn console(root: Option<PathBuf>, allow_tun: bool, allow_wfp: bool) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .with(logring::RingLayer.with_filter(LevelFilter::INFO))
        .init();
    if (allow_tun || allow_wfp) && !win::is_elevated() {
        bail!("--allow-tun и --allow-wfp требуют запуска от администратора");
    }
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

/// Журнал рабочей службы: `%ProgramData%\klick\logs\service.log`, короткий, без адресов сайтов.
pub fn init_file_log(paths: &Paths) -> Result<()> {
    let file = std::fs::OpenOptions::new().create(true).append(true).open(paths.logs.join("service.log"))?;
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(std::sync::Mutex::new(file)).with_filter(LevelFilter::INFO))
        .with(logring::RingLayer.with_filter(LevelFilter::INFO))
        .try_init();
    Ok(())
}
