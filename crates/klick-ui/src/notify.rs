//! Уведомления Windows и macOS. Нажатие открывает окно на нужном экране — таблица «Куда ведут уведомления» в навигации.

use tauri::AppHandle;
#[cfg(any(windows, all(target_os = "macos", feature = "mac-notify")))]
use tauri::Emitter;
#[cfg(windows)]
use tauri_winrt_notification::Toast;

/// Установленная kl!ck регистрирует свой AppUserModelID ярлыком в меню «Пуск» (это делает установщик).
/// До установки уведомления идут от имени PowerShell, иначе Windows их не покажет.
#[cfg(windows)]
fn app_id() -> &'static str {
    let installed = std::env::current_exe().ok().is_some_and(|p| p.to_string_lossy().to_lowercase().contains(r"\program files\"));
    if installed {
        "app.klick.desktop"
    } else {
        Toast::POWERSHELL_APP_ID
    }
}

/// Нажали на уведомление: окно на нужном экране.
#[cfg(any(windows, all(target_os = "macos", feature = "mac-notify")))]
fn activate(app: &AppHandle, target: Option<&str>) {
    crate::show_main(app);
    if let Some(t) = target {
        let _ = app.emit_to("main", "klick://navigate", t.to_string());
    }
}

/// `target` — куда вести по нажатию: `servers`, `connection`, `neighbors`, `killswitch`, `log`.
#[cfg(windows)]
#[tauri::command]
pub fn notify(app: AppHandle, title: String, text: String, target: Option<String>) -> Result<(), String> {
    let handle = app.clone();
    Toast::new(app_id())
        .title(&title)
        .text1(&text)
        .on_activated(move |_| {
            activate(&handle, target.as_deref());
            Ok(())
        })
        .show()
        .map_err(|e| e.to_string())
}

/// macOS: уведомление от имени kl!ck. Ответ (нажатие или закрытие) ждём в отдельном потоке:
/// библиотека получает его через главный цикл приложения, который крутит Tauri.
/// Из пакета `.app` уведомление идёт от kl!ck; при запуске из папки сборки — от имени Терминала,
/// иначе macOS его не покажет.
#[cfg(all(target_os = "macos", feature = "mac-notify"))]
#[tauri::command]
pub fn notify(app: AppHandle, title: String, text: String, target: Option<String>) -> Result<(), String> {
    use mac_notification_sys::{Notification, NotificationResponse};
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let bundle = if crate::in_app_bundle() { "app.klick.desktop".to_string() } else { mac_notification_sys::get_bundle_identifier_or_default("Terminal") };
        if let Err(e) = mac_notification_sys::set_application(&bundle) {
            eprintln!("уведомления: {e}");
        }
    });
    std::thread::spawn(move || {
        match Notification::new().title(&title).message(&text).wait_for_click(true).send() {
            Ok(NotificationResponse::Click) | Ok(NotificationResponse::ActionButton(_)) => {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || activate(&handle, target.as_deref()));
            }
            Ok(_) => {}
            Err(e) => eprintln!("уведомление не показано: {e}"),
        }
    });
    Ok(())
}

/// Без части на Objective-C (сборка для проверки на другой системе): уведомление через AppleScript, без перехода по нажатию.
#[cfg(all(not(windows), not(all(target_os = "macos", feature = "mac-notify"))))]
#[tauri::command]
pub fn notify(app: AppHandle, title: String, text: String, target: Option<String>) -> Result<(), String> {
    let _ = (&app, &target);
    let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(format!("display notification {} with title {}", quote(&text), quote(&title)))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
