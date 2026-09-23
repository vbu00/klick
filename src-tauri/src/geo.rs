//! База GeoIP для пресета «Российские IP — напрямую».
//!
//! В установщик входит country.mmdb из MetaCubeX/meta-rules-dat,
//! привязанный к коммиту и сверенный по sha256. Но адреса переезжают между
//! странами, и через пару месяцев такая база начинает ошибаться — поэтому
//! её надо обновлять:
//!
//! - пока VPN включён и пресет стоит, mihomo сам раз в `INTERVAL_HOURS`
//!   часов скачивает свежую базу по `geox-url` (через свои же правила),
//!   проверяет её и подхватывает на лету;
//! - кнопкой в «О приложении» — при включённом VPN тем же API mihomo, при
//!   выключенном сами: curl, затем `mihomo -t` на копии. Битую базу mihomo
//!   на проверке отвергает, и на место она не встаёт.
//!
//! Базу из установщика кладём в папку ядра, только если там её нет или она
//! старее: прежде копия при каждом подключении затирала обновлённую.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

pub const MMDB_URL: &str = "https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/country.mmdb";
pub const INTERVAL_HOURS: u32 = 72;
const FILE: &str = "Country.mmdb";
/// Настоящая база весит 5–10 МБ; всё, что больше, — не она.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MIN_BYTES: u64 = 256 * 1024;
/// Подпись метаданных формата MaxMind DB — в конце любого .mmdb.
const MARKER: &[u8] = b"\xAB\xCD\xEFMaxMind.com";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeoInfo {
    /// Когда база скачана (время файла), секунды Unix.
    pub updated: Option<u64>,
    pub size: u64,
    /// Обновлялась ли после установки.
    pub fresh: bool,
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn bundled(app: &AppHandle) -> PathBuf {
    crate::core::bin_path(app, FILE)
}

pub fn path(home: &Path) -> PathBuf {
    home.join(FILE)
}

/// База в папке ядра: из установщика, если своей нет или своя старее.
/// CopyFile сохраняет время файла — по нему mihomo решает, пора ли
/// обновляться.
pub fn ensure(app: &AppHandle, home: &Path) {
    let db = path(home);
    let src = bundled(app);
    let copy = match (mtime(&db), mtime(&src)) {
        (None, Some(_)) => true,
        (Some(have), Some(ship)) => ship > have || !valid_file(&db),
        _ => false,
    };
    if copy {
        let _ = std::fs::copy(&src, &db);
    }
}

pub fn info(app: &AppHandle) -> GeoInfo {
    let home = app.state::<AppState>().dir.join("core");
    let db = path(&home);
    let have = mtime(&db);
    let ship = mtime(&bundled(app));
    GeoInfo {
        updated: have.or(ship).map(secs),
        size: std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0),
        fresh: matches!((have, ship), (Some(h), Some(s)) if h > s),
    }
}

/// Быстрая проверка без mihomo: размер и подпись формата.
fn valid_bytes(b: &[u8]) -> bool {
    (b.len() as u64) >= MIN_BYTES && (b.len() as u64) <= MAX_BYTES && b[b.len().saturating_sub(128 * 1024)..].windows(MARKER.len()).any(|w| w == MARKER)
}

fn valid_file(p: &Path) -> bool {
    std::fs::read(p).map(|b| valid_bytes(&b)).unwrap_or(false)
}

/// Обновить сейчас. Возвращает время новой базы.
pub fn update(app: &AppHandle) -> Result<GeoInfo, String> {
    if let Some(api) = crate::core::api() {
        api.update_geo().map_err(|e| format!("mihomo не обновил базу: {}", e.trim()))?;
        crate::core::note("INFO", "База GeoIP обновлена");
        return Ok(info(app));
    }
    let home = app.state::<AppState>().dir.join("core");
    let _ = std::fs::create_dir_all(&home);
    let tmp = home.join(format!("geo-{}", crate::sys::random_hex(4)));
    let result = download_and_check(app, &tmp).and_then(|file| {
        let db = path(&home);
        std::fs::copy(&file, &db).map_err(|e| format!("не удалось записать базу: {e}"))?;
        Ok(())
    });
    let _ = std::fs::remove_dir_all(&tmp);
    result?;
    crate::core::note("INFO", "База GeoIP обновлена");
    Ok(info(app))
}

fn download_and_check(app: &AppHandle, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let file = dir.join(FILE);
    let out = crate::sys::command("curl.exe")
        .args(["-sS", "-L", "--fail", "--proto", "=https", "--proto-redir", "=https", "--max-time", "120", "--max-filesize"])
        .arg(MAX_BYTES.to_string())
        .args(["-A", "klick", "-o"])
        .arg(&file)
        .arg(MMDB_URL)
        .output()
        .map_err(|e| format!("не удалось запустить curl: {e}"))?;
    if !out.status.success() {
        let err = crate::sys::decode_console(&out.stderr);
        let err = err.trim().trim_start_matches("curl: ").trim().to_string();
        return Err(if err.is_empty() { "База не скачалась.".into() } else { format!("База не скачалась: {err}") });
    }
    if !valid_file(&file) {
        return Err("Скачался не тот файл — база не изменена.".into());
    }
    // Последнее слово за mihomo: он откроет базу так же, как при
    // подключении. geox-url — в никуда, чтобы на битой базе он не пошёл
    // скачивать свою, а честно провалил проверку.
    let cfg = dir.join("check.yaml");
    let body = serde_json::json!({
        "log-level": "warning",
        "geo-auto-update": false,
        "geox-url": { "mmdb": "https://127.0.0.1:1/none.mmdb" },
        "proxies": [],
        "rules": ["GEOIP,RU,DIRECT", "MATCH,DIRECT"],
    });
    std::fs::write(&cfg, body.to_string()).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(crate::core::mihomo_exe(app));
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::sys::CREATE_NO_WINDOW);
    }
    let out = cmd.arg("-t").arg("-d").arg(dir).arg("-f").arg(&cfg).output().map_err(|e| format!("mihomo не запустился: {e}"))?;
    if !out.status.success() || !file.exists() {
        return Err("mihomo отверг скачанную базу — оставляю прежнюю.".into());
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn подпись_mmdb() {
        let mut b = vec![0u8; MIN_BYTES as usize];
        assert!(!valid_bytes(&b));
        b.extend_from_slice(MARKER);
        b.extend_from_slice(&[0u8; 300]);
        assert!(valid_bytes(&b));
        assert!(!valid_bytes(MARKER));
    }

    /// База из установщика проходит ту же проверку.
    #[test]
    fn база_из_комплекта_валидна() {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin").join(FILE);
        if p.exists() {
            assert!(valid_file(&p));
        }
    }
}
