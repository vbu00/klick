//! Служба Windows: регистрация, запуск по команде SCM, остановка.

use crate::paths::{Paths, Profile};
use crate::{engine, pipe};
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceFailureActions, ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

pub const NAME: &str = "klick";
const DISPLAY: &str = "kl!ck";
const DESCRIPTION: &str = "kl!ck: VPN-подключение, маршрутизация и Kill Switch";

define_windows_service!(ffi_service_main, service_main);

pub fn run_dispatcher() -> Result<()> {
    service_dispatcher::start(NAME, ffi_service_main).context("служба запущена не через Windows")?;
    Ok(())
}

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = run_service() {
        tracing::error!("служба остановилась с ошибкой: {e:#}");
    }
}

fn status(state: ServiceState, code: u32) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running { ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN } else { ServiceControlAccept::empty() },
        exit_code: ServiceExitCode::Win32(code),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    }
}

fn run_service() -> Result<()> {
    let paths = Paths::production()?;
    paths.ensure_dirs()?;
    crate::init_file_log(&paths)?;

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let stop = Arc::new(Mutex::new(Some(stop_tx)));
    let handler = move |event| match event {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            if let Some(tx) = stop.lock().unwrap().take() {
                let _ = tx.send(());
            }
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let status_handle = service_control_handler::register(NAME, handler)?;
    status_handle.set_service_status(status(ServiceState::Running, 0))?;

    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async move {
        let profile = Profile::production();
        let (engine, _task) = engine::spawn(paths, profile.clone())?;
        let server = tokio::spawn(pipe::serve(profile.pipe.clone(), true, engine.clone()));
        let _ = stop_rx.await;
        engine.shutdown().await;
        server.abort();
        anyhow::Ok(())
    });
    status_handle.set_service_status(status(ServiceState::Stopped, if result.is_ok() { 0 } else { 1 }))?;
    result
}

pub fn install() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
        .context("нужны права администратора")?;
    let exe = std::env::current_exe()?;
    let info = ServiceInfo {
        name: NAME.into(),
        display_name: DISPLAY.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec!["run".into()],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let service = manager.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)?;
    service.set_description(DESCRIPTION)?;
    service.update_failure_actions(ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 3600)),
        reboot_msg: None,
        command: None,
        actions: Some(vec![
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(1) },
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(5) },
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(30) },
        ]),
    })?;
    service.set_failure_actions_on_non_crash_failures(true)?;
    service.start::<&str>(&[])?;
    println!("служба {NAME} установлена и запущена");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).context("нужны права администратора")?;
    let service = manager.open_service(NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE)?;
    if service.query_status()?.current_state != ServiceState::Stopped {
        let _ = service.stop();
        for _ in 0..50 {
            if service.query_status()?.current_state == ServiceState::Stopped {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    service.delete()?;
    println!("служба {NAME} удалена");
    Ok(())
}
