//! Автозапуск через Планировщик заданий, как в Klutz: приложению нужны
//! права администратора (TUN), а обычный автозапуск Windows их не даёт и
//! спрашивал бы UAC при каждом входе.

use crate::sys;

pub const TASK_NAME: &str = "klick-Autostart";

pub fn is_enabled() -> bool {
    sys::run_ok("schtasks.exe", &["/Query", "/TN", TASK_NAME])
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    if !enabled {
        let out = sys::run("schtasks.exe", &["/Delete", "/TN", TASK_NAME, "/F"]);
        return if is_enabled() {
            Err(if out.trim().is_empty() { "не удалось удалить задачу автозапуска".into() } else { out })
        } else {
            Ok(())
        };
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let tr = format!("\"{}\" --autostart", exe.display());
    let out = sys::run(
        "schtasks.exe",
        &["/Create", "/TN", TASK_NAME, "/TR", &tr, "/SC", "ONLOGON", "/RL", "HIGHEST", "/F"],
    );
    if is_enabled() {
        Ok(())
    } else {
        Err(if out.trim().is_empty() { "не удалось создать задачу".into() } else { out })
    }
}
