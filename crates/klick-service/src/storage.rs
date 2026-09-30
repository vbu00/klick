//! Настройки и ключи на диске. Ключи (ссылки подписок) — отдельным зашифрованным файлом.

use crate::paths::Paths;
use crate::sys;
use anyhow::{Context, Result};
use klick_core::{Catalog, Settings};
use std::collections::HashMap;
use std::path::Path;

pub fn load_settings(paths: &Paths) -> Settings {
    match std::fs::read_to_string(&paths.settings) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!("settings.json не читается ({e}), начинаю с настроек по умолчанию");
            let _ = std::fs::rename(&paths.settings, paths.settings.with_extension("json.broken"));
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub fn save_settings(paths: &Paths, settings: &Settings) -> Result<()> {
    let text = serde_json::to_string_pretty(settings)?;
    write_atomic(&paths.settings, text.as_bytes())
}

/// Ссылки подписок и одиночные ссылки по id подключения.
#[derive(Default)]
pub struct Secrets(pub HashMap<String, String>);

impl Secrets {
    pub fn load(paths: &Paths) -> Self {
        let Ok(blob) = std::fs::read(&paths.secrets) else { return Self::default() };
        match sys::unprotect(&blob).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            Some(map) => Secrets(map),
            None => {
                tracing::warn!("secrets.bin не расшифровывается: ссылки подписок придётся добавить заново");
                Self::default()
            }
        }
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        let plain = serde_json::to_vec(&self.0)?;
        let blob = sys::protect(&plain)?;
        write_atomic(&paths.secrets, &blob)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&paths.secrets, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

pub fn load_catalog(paths: &Paths) -> Catalog {
    let file = paths.resources.join("catalog.json");
    match std::fs::read_to_string(&file).map(|t| serde_json::from_str::<Catalog>(&t)) {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            tracing::warn!("catalog.json с ошибкой: {e}");
            Catalog::default()
        }
        Err(_) => Catalog::default(),
    }
}

/// Кладёт в домашнюю папку ядра наборы и базу стран из ресурсов, если их там нет или они старее.
pub fn install_core_files(paths: &Paths) -> Result<()> {
    let sets_src = paths.resources.join("sets");
    if let Ok(entries) = std::fs::read_dir(&sets_src) {
        for e in entries.flatten() {
            copy_if_newer(&e.path(), &paths.sets.join(e.file_name()))?;
        }
    }
    let mmdb = paths.resources.join("core").join("Country.mmdb");
    if mmdb.exists() {
        copy_if_newer(&mmdb, &paths.core_home.join("Country.mmdb"))?;
    }
    Ok(())
}

fn copy_if_newer(src: &Path, dst: &Path) -> Result<()> {
    let newer = match (std::fs::metadata(src), std::fs::metadata(dst)) {
        (Ok(s), Ok(d)) => s.modified().ok() > d.modified().ok() || s.len() != d.len(),
        (Ok(_), Err(_)) => true,
        _ => false,
    };
    if newer {
        std::fs::copy(src, dst).with_context(|| format!("копирование {} → {}", src.display(), dst.display()))?;
    }
    Ok(())
}

/// Запись через временный файл: при сбое посреди записи старый файл остаётся целым.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).with_context(|| format!("запись {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("замена {}", path.display()))?;
    Ok(())
}
