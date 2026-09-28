//! Уведомления Windows. Нажатие открывает окно на нужном экране — таблица «Куда ведут уведомления» в навигации.

use tauri::{AppHandle, Emitter};
use tauri_winrt_notification::Toast;

/// Установленная kl!ck регистрирует свой AppUserModelID ярлыком в меню «Пуск» (это делает установщик).
/// До установки уведомления идут от имени PowerShell, иначе Windows их не покажет.
fn app_id() -> &'static str {
    let installed = std::env::current_exe().ok().is_some_and(|p| p.to_string_lossy().to_lowercase().contains(r"\program files\"));
    if installed {
        "app.klick.desktop"
    } else {
        Toast::POWERSHELL_APP_ID
    }
}

/// `target` — куда вести по нажатию: `servers`, `connection`, `neighbors`, `killswitch`, `log`.
#[tauri::command]
pub fn notify(app: AppHandle, title: String, text: String, target: Option<String>) -> Result<(), String> {
    let handle = app.clone();
    Toast::new(app_id())
        .title(&title)
        .text1(&text)
        .on_activated(move |_| {
            crate::show_main(&handle);
            if let Some(t) = &target {
                let _ = handle.emit_to("main", "klick://navigate", t.clone());
            }
            Ok(())
        })
        .show()
        .map_err(|e| e.to_string())
}
