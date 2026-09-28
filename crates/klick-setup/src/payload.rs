//! Архив с программой внутри установщика: `klick.exe`, `klick-service.exe`, `resources`.

use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.zip"));

/// Сколько места займёт программа, байт.
pub fn size() -> u64 {
    env!("KLICK_PAYLOAD_BYTES").parse().unwrap_or(0)
}

fn archive() -> Result<zip::ZipArchive<Cursor<&'static [u8]>>, String> {
    zip::ZipArchive::new(Cursor::new(PAYLOAD)).map_err(|e| format!("архив установщика повреждён: {e}"))
}

/// Текстовый файл из архива — например, текст лицензии для окна установщика.
pub fn read(name: &str) -> Option<String> {
    let mut zip = archive().ok()?;
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

/// Размер файлов, подходящих под `filter`.
pub fn sum(filter: &dyn Fn(&str) -> bool) -> u64 {
    let Ok(mut zip) = archive() else { return 0 };
    let mut total = 0;
    for i in 0..zip.len() {
        if let Ok(f) = zip.by_index(i) {
            if filter(f.name()) {
                total += f.size();
            }
        }
    }
    total
}

/// Распаковать подходящие файлы в папку. `progress` получает число записанных байт;
/// отмена проверяется между кусками, чтобы большое ядро не держало кнопку «Прервать».
pub fn extract(dir: &Path, filter: &dyn Fn(&str) -> bool, cancel: &AtomicBool, progress: &mut dyn FnMut(u64)) -> Result<(), String> {
    let mut zip = archive()?;
    let mut done = 0u64;
    let mut buf = vec![0u8; 1 << 20];
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        if !filter(&name) || name.ends_with('/') {
            continue;
        }
        if name.split('/').any(|p| p == ".." || p.is_empty()) {
            return Err(format!("странное имя в архиве: {name}"));
        }
        let out = dir.join(name.replace('/', "\\"));
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut w = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", out.display()))?;
            done += n as u64;
            progress(done);
        }
    }
    Ok(())
}
