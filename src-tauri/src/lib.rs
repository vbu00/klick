//! kl!ck — VPN-клиент на ядре mihomo.
//!
//! Одно приложение с правами администратора (TUN и брандмауэр их требуют),
//! mihomo — его дочерний процесс, автозапуск — задачей Планировщика без
//! запроса UAC. Всё, что нужно ядру, — в %LOCALAPPDATA%\com.vbu00.klick.

mod api;
mod autostart;
mod commands;
mod config;
mod core;
mod favicon;
mod geo;
mod killswitch;
mod links;
mod notify;
mod procs;
mod state;
mod sub;
mod sys;
mod sysproxy;
mod tray;

use state::AppState;
use tauri::{Manager, WindowEvent};

pub fn run() {
    // Деинсталлятор зовёт `klick.exe --cleanup`: снять правила Kill Switch
    // и вернуть системный прокси.
    if std::env::args().any(|a| a == "--cleanup") {
        killswitch::cleanup();
        if let Some(dir) = std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("com.vbu00.klick")) {
            let port = std::fs::read_to_string(dir.join("settings.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<state::Settings>(&s).ok())
                .map(|s| s.proxy_port)
                .unwrap_or(7890);
            sysproxy::disable(&dir, port);
        }
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = AppState::load(&handle);
            let (ks, ks_apps, ks_sites, port, connect_on_launch) = {
                let s = state.settings.lock().unwrap();
                (s.kill_switch, s.ks_apps.clone(), s.ks_sites.clone(), s.proxy_port, s.connect_on_launch)
            };
            // Системный прокси от прошлого запуска (падение, выключение ПК)
            // указывает в никуда — вернуть как было.
            sysproxy::disable(&state.dir, port);
            app.manage(state);
            tray::build(&handle)?;
            core::start_monitor(handle.clone());

            let autostarted = std::env::args().any(|a| a == "--autostart");
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    if let Some(w) = core::ks_apply(&h, ks, &ks_apps, &ks_sites) {
                        core::note("WARN", &w);
                    }
                    if connect_on_launch && autostarted {
                        if let Err(e) = core::connect(&h) {
                            notify::send(&h, "Не удалось подключиться", &e, true);
                        }
                    }
                    loop {
                        commands::refresh_due(&h);
                        std::thread::sleep(std::time::Duration::from_secs(3600));
                    }
                });
            }
            if autostarted {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == tray::POPUP {
                match event {
                    WindowEvent::Focused(false) => tray::hide_popup(window.app_handle()),
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        tray::hide_popup(window.app_handle());
                    }
                    _ => {}
                }
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_overview,
            commands::connect,
            commands::disconnect,
            commands::select_profile,
            commands::select_server,
            commands::ping,
            commands::add_link,
            commands::add_file,
            commands::refresh_sub,
            commands::remove_profile,
            commands::restore_profile,
            commands::profile_link,
            commands::update_settings,
            commands::set_autostart,
            commands::retry_kill_switch,
            commands::favicon,
            commands::app_info,
            commands::update_geo,
            commands::check_update,
            commands::open_url,
            commands::licenses,
            commands::running_apps,
            commands::app_from_file,
            commands::get_logs,
            commands::clear_logs,
            commands::restore_logs,
            commands::copy_text,
            commands::open_data_dir,
            commands::window_minimize,
            commands::window_close,
            commands::quit_app,
            tray::tray_menu_state,
            tray::tray_menu_ready,
            tray::tray_menu_hide,
            tray::tray_menu_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
