//! kl!ck — установщик. Окно по макету (ui/), установку делает вшитый
//! NSIS-установщик kl!ck в тихом режиме (ops.rs).
//!
//!   klick-setup.exe              — установка, а если kl!ck уже стоит —
//!                                  обновление / переустановка / удаление
//!   klick-setup.exe --uninstall  — сразу к удалению (так его зовёт
//!                                  «Удалить» в параметрах Windows)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ops;
mod reg;

use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    version: &'static str,
    installed: Option<ops::Installed>,
    /// −1 — стоит старее, 0 — та же, 1 — новее.
    compare: Option<i32>,
    default_path: String,
    required: u64,
    free: Option<u64>,
    /// Открыт из «Удалить» в параметрах Windows.
    uninstall: bool,
}

#[tauri::command]
fn info() -> Info {
    let installed = ops::installed();
    let default_path = installed.as_ref().map(|i| i.path.clone()).unwrap_or_else(ops::default_path);
    Info {
        version: ops::VERSION,
        compare: installed.as_ref().map(|i| ops::compare(&i.version)),
        free: ops::free_space(&default_path),
        installed,
        default_path,
        required: ops::SIZE,
        uninstall: std::env::args().any(|a| a.eq_ignore_ascii_case("--uninstall")),
    }
}

#[tauri::command]
fn free_space(path: String) -> Option<u64> {
    ops::free_space(&path)
}

/// Системный выбор папки; kl!ck ляжет в подпапку kl!ck выбранной.
#[tauri::command(async)]
fn pick_folder(app: AppHandle, current: String) -> Option<String> {
    let start = std::path::Path::new(&current).parent().map(|p| p.to_path_buf());
    let mut dlg = app.dialog().file().set_title("Куда установить kl!ck");
    if let Some(s) = start.filter(|p| p.exists()) {
        dlg = dlg.set_directory(s);
    }
    let picked = dlg.blocking_pick_folder()?.into_path().ok()?;
    Some(ops::product_dir(&picked).display().to_string())
}

#[derive(Deserialize)]
struct InstallOpts {
    path: String,
    desktop: bool,
    autostart: bool,
    kind: String,
}

#[tauri::command(async)]
fn install(app: AppHandle, opts: InstallOpts) -> Result<(), String> {
    ops::install(&app, &opts.path, opts.desktop, opts.autostart, &opts.kind)
}

#[tauri::command]
fn cancel_install() {
    ops::cancel();
}

#[tauri::command(async)]
fn uninstall(app: AppHandle, wipe: bool) -> Result<(), String> {
    ops::uninstall(&app, wipe)
}

#[tauri::command]
fn license() -> &'static str {
    ops::LICENSE
}

#[tauri::command]
fn minimize(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

/// Закрыть установщик; launch — сначала запустить kl!ck из этой папки.
#[tauri::command]
fn finish(app: AppHandle, launch: Option<String>) -> Result<(), String> {
    if let Some(dir) = launch {
        ops::launch(&dir)?;
    }
    if ops::SELF_DELETE.load(Ordering::SeqCst) {
        ops::schedule_self_delete();
    }
    app.exit(0);
    Ok(())
}

fn main() {
    // Старый установщик kl!ck перед установкой зовёт «удаление» из реестра
    // с /UPDATE — а там теперь мы. Обновлению удаление не нужно: выходим.
    if std::env::args().any(|a| a.eq_ignore_ascii_case("/UPDATE")) {
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Данные WebView — во временной папке: установщику не нужно
            // оставлять следов в %LOCALAPPDATA%.
            let data = std::env::temp_dir().join("klick-setup-webview");
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Установка kl!ck")
                .inner_size(680.0, 440.0)
                .resizable(false)
                .maximizable(false)
                .decorations(false)
                .shadow(true)
                .center()
                .data_directory(data)
                .build()?;
            let _ = app.get_webview_window("main").map(|w| w.set_focus());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![info, free_space, pick_folder, install, cancel_install, uninstall, license, minimize, finish])
        .run(tauri::generate_context!())
        .expect("установщик не запустился");
}
