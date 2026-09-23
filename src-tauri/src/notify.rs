//! Уведомления Windows: kl!ck живёт в трее, и об обрыве иначе не узнать.
//! AppUserModelID — только установленной копии: у exe из target/ нет
//! ярлыка с этим ID, и Windows такой тост не покажет.

use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
fn installed_app_id(app: &AppHandle) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_string_lossy().to_lowercase();
    if dir.ends_with(r"target\debug") || dir.ends_with(r"target\release") {
        return None;
    }
    Some(app.config().identifier.clone())
}

/// `important` — показывать, даже когда окно на экране.
pub fn send(app: &AppHandle, title: &str, body: &str, important: bool) {
    if !important {
        if let Some(w) = app.get_webview_window("main") {
            if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
                return;
            }
        }
    }
    let mut n = notify_rust::Notification::new();
    n.summary(title).body(body).auto_icon();
    #[cfg(target_os = "windows")]
    if let Some(id) = installed_app_id(app) {
        n.app_id(&id);
    }
    std::thread::spawn(move || {
        let _ = n.show();
    });
}
