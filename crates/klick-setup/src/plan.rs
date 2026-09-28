//! Что уже стоит на компьютере: эта kl!ck (обновить, переустановить, удалить) и прежняя kl!ck от vbu00
//! (0.2–0.3 на ядре в папке `bin`) — её установщик находит и убирает со всеми следами.

use crate::win::{self, Key, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\klick";
pub const OLD_UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\kl!ck";
/// Задача Планировщика, через которую прежняя kl!ck запускалась с правами администратора.
pub const OLD_TASK: &str = "klick-Autostart";
/// Группа правил брандмауэра, которой прежняя kl!ck делала Kill Switch.
pub const OLD_FIREWALL_GROUP: &str = "klick-killswitch";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CORE_VERSION: &str = env!("KLICK_CORE_VERSION");

#[derive(Clone, Debug, Serialize)]
pub struct Installed {
    pub version: String,
    pub path: String,
    /// Какая версия стоит относительно этой: `older`, `same`, `newer`.
    pub relation: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct Old {
    pub version: Option<String>,
    /// Папки с программой: из реестра и известные места.
    pub dirs: Vec<String>,
    pub uninstaller: Option<String>,
    /// Настройки прежней версии в профиле пользователя.
    pub data: Option<String>,
    pub task: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Info {
    pub version: String,
    pub core_version: String,
    pub default_path: String,
    /// Сколько займёт программа, байт.
    pub size: u64,
    pub installed: Option<Installed>,
    pub old: Option<Old>,
    /// Где лежат профили и настройки — для галочки «Удалить профили…».
    pub data_path: String,
}

pub fn default_path() -> PathBuf {
    let pf = std::env::var_os("ProgramW6432").or_else(|| std::env::var_os("ProgramFiles")).map(PathBuf::from).unwrap_or_else(|| "C:\\Program Files".into());
    pf.join("klick")
}

pub fn detect() -> Info {
    let installed = installed();
    Info {
        version: VERSION.into(),
        core_version: CORE_VERSION.into(),
        default_path: installed.as_ref().map(|i| i.path.clone()).unwrap_or_else(|| default_path().to_string_lossy().into_owned()),
        // Программа и копия установщика, из которой её потом удаляют.
        size: crate::payload::size() + std::env::current_exe().and_then(std::fs::metadata).map(|m| m.len()).unwrap_or(0),
        installed,
        old: old(),
        data_path: data_dir().to_string_lossy().into_owned(),
    }
}

/// Данные службы: профили, подписки, ключи, настройки, журнал.
pub fn data_dir() -> PathBuf {
    std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| "C:\\ProgramData".into()).join("klick")
}

pub fn installed() -> Option<Installed> {
    if let Some(k) = Key::open(HKEY_LOCAL_MACHINE, UNINSTALL_KEY) {
        if let Some(path) = k.string("InstallLocation").map(|p| p.trim_matches('"').to_string()) {
            let version = k.string("DisplayVersion").unwrap_or_else(|| "?".into());
            return Some(Installed { relation: relation(&version), version, path });
        }
    }
    let p = default_path();
    p.join("klick-service.exe").exists().then(|| Installed { version: "?".into(), path: p.to_string_lossy().into_owned(), relation: "older" })
}

fn parse(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect()
}

fn relation(installed: &str) -> &'static str {
    let (a, b) = (parse(installed), parse(VERSION));
    if a.is_empty() {
        return "older";
    }
    match a.cmp(&b) {
        std::cmp::Ordering::Less => "older",
        std::cmp::Ordering::Equal => "same",
        std::cmp::Ordering::Greater => "newer",
    }
}

fn old() -> Option<Old> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut version = None;
    let mut uninstaller = None;
    // Обычно она стояла на всех пользователей (HKLM); ранние сборки — только на одного (HKCU).
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let Some(k) = Key::open(root, OLD_UNINSTALL_KEY) else { continue };
        version = version.or_else(|| k.string("DisplayVersion"));
        if let Some(loc) = k.string("InstallLocation") {
            dirs.push(PathBuf::from(loc.trim_matches('"')));
        }
        uninstaller = uninstaller.or_else(|| k.string("UninstallString").map(|u| u.trim_matches('"').to_string()).filter(|u| Path::new(u).exists()));
    }
    let pf = std::env::var_os("ProgramW6432").map(PathBuf::from).unwrap_or_else(|| "C:\\Program Files".into());
    let local = std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("kl!ck"));
    for d in [Some(pf.join("kl!ck")), Some(PathBuf::from(r"C:\Programs\kl!ck")), local].into_iter().flatten() {
        if d.join("klick.exe").exists() && !dirs.iter().any(|x| x.to_string_lossy().eq_ignore_ascii_case(&d.to_string_lossy())) {
            dirs.push(d);
        }
    }
    dirs.retain(|d| d.exists());
    let data = old_data().filter(|d| d.exists()).map(|d| d.to_string_lossy().into_owned());
    let task = win::run(&win::system32("schtasks.exe"), &["/Query", "/TN", OLD_TASK], Duration::from_secs(10)).is_ok_and(|c| c == 0);
    if dirs.is_empty() && data.is_none() && !task && uninstaller.is_none() {
        return None;
    }
    Some(Old { version, dirs: dirs.iter().map(|d| d.to_string_lossy().into_owned()).collect(), uninstaller, data, task })
}

/// Настройки и подписки прежней kl!ck.
pub fn old_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("com.vbu00.klick"))
}

/// Проверить папку установки. Ошибка — код для экрана (`path.*`).
///
/// Служба работает от имени системы и запускается из этой папки, поэтому папка не должна
/// лежать там, где обычный пользователь может её подменить: в профилях пользователей
/// (он хозяин всего внутри своего профиля) и в системной папке Windows. Чужая непустая
/// папка тоже не годится: при обновлении и удалении установщик убирает её содержимое.
pub fn check_path(raw: &str) -> Result<PathBuf, &'static str> {
    let s = raw.trim().trim_matches('"').replace('/', "\\");
    let s = s.trim_end_matches('\\');
    let b = s.as_bytes();
    if b.len() < 4 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
        return Err("path.absolute");
    }
    if s.chars().count() > 180 {
        return Err("path.long");
    }
    let rest = &s[3..];
    if rest.chars().any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || (c as u32) < 32)
        || rest.split('\\').any(|p| p.is_empty() || p.ends_with(' ') || p.ends_with('.'))
    {
        return Err("path.chars");
    }
    if !Path::new(&s[..3]).exists() {
        return Err("path.drive");
    }
    let path = PathBuf::from(s);
    let windows = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    if same(&path, &windows) || win::inside(&path, &windows) {
        return Err("path.system");
    }
    if let Some(users) = std::env::var_os("USERPROFILE").map(PathBuf::from).and_then(|p| p.parent().map(Path::to_path_buf)) {
        if same(&path, &users) || win::inside(&path, &users) {
            return Err("path.profile");
        }
    }
    if path.exists() {
        if !path.is_dir() {
            return Err("path.file");
        }
        let ours = path.join("klick-service.exe").exists();
        let empty = std::fs::read_dir(&path).map(|mut d| d.next().is_none()).unwrap_or(false);
        if !ours && !empty {
            return Err("path.not_empty");
        }
    }
    Ok(path)
}

fn same(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_numbers() {
        assert_eq!(parse("0.10.0"), vec![0, 10, 0]);
        assert!(parse("0.4.9") < parse("0.4.10"));
        assert!(parse("v1.2") < parse("1.2.1"));
    }

    #[test]
    fn bad_paths_are_rejected() {
        assert_eq!(check_path("klick"), Err("path.absolute"));
        assert_eq!(check_path(r"\\server\share\klick"), Err("path.absolute"));
        assert_eq!(check_path(r"C:\Program Files\kl|ck"), Err("path.chars"));
        assert_eq!(check_path(r"C:\Program Files\klick."), Err("path.chars"));
        let windows = std::env::var("SystemRoot").unwrap();
        assert_eq!(check_path(&format!(r"{windows}\klick")), Err("path.system"));
        let profile = std::env::var("USERPROFILE").unwrap();
        assert_eq!(check_path(&format!(r"{profile}\klick")), Err("path.profile"));
        assert_eq!(check_path(&windows[..2].to_string()), Err("path.absolute"));
        // Program Files сама по себе — чужая непустая папка.
        assert_eq!(check_path(r"C:\Program Files"), Err("path.not_empty"));
        assert!(check_path(r"C:\Program Files\klick-test-nonexistent").is_ok());
        assert_eq!(check_path(r"C:/Program Files/klick-test-nonexistent/").unwrap(), PathBuf::from(r"C:\Program Files\klick-test-nonexistent"));
    }
}
