//! Пути программ на macOS. Программа — пакет `.app` целиком, вместе с помощниками внутри
//! (`Google Chrome Helper.app`, `Discord Helper (Renderer).app`): в сеть часто ходит именно помощник.
//! Программа без пакета (`node`, `curl` из Homebrew) — её папка установки, как на Windows.

use crate::model::{is_version_dir, InputError};

/// Абсолютный путь без завершающего слэша. `~`, относительные пути и `..` не принимаются.
pub fn normalize_folder(input: &str) -> Result<String, InputError> {
    let s = input.trim().trim_matches('"').trim_matches('\'');
    if s.is_empty() {
        return Err(InputError::Empty);
    }
    if !s.starts_with('/') {
        return Err(InputError::BadPath);
    }
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.contains(&"..") {
        return Err(InputError::BadPath);
    }
    Ok(format!("/{}", parts.join("/")))
}

/// Папка программы для правила и Kill Switch.
///
/// Путь внутри `.app` превращается в самый внешний пакет: `/Applications/Discord.app/Contents/Frameworks/
/// Discord Helper.app/Contents/MacOS/Discord Helper` → `/Applications/Discord.app`. Для программы без пакета
/// служба подсказывает `is_file`: у исполняемого файла берётся папка, папки версий над ним пропускаются
/// (`/opt/homebrew/Cellar/node/22.1.0/bin/node` → `/opt/homebrew/Cellar/node`). Без подсказки путь — папка.
/// Общие папки вроде `/Applications`, «Загрузок» и `/usr/local/bin` не подходят: правило задело бы чужие программы.
pub fn program_folder(input: &str, is_file: Option<bool>) -> Result<String, InputError> {
    let path = normalize_folder(input)?;
    let folder = match outer_bundle(&path) {
        Some(bundle) => bundle.to_string(),
        None if is_file == Some(true) => install_folder(parent(&path).ok_or(InputError::BadPath)?),
        None => path,
    };
    if is_broad_folder(&folder) {
        return Err(InputError::TooBroad);
    }
    Ok(folder)
}

/// Самый внешний пакет `.app` в пути, если он есть.
pub fn outer_bundle(path: &str) -> Option<&str> {
    let mut pos = 0;
    for segment in path.split('/') {
        let end = pos + segment.len();
        if segment.len() > 4 && segment.to_ascii_lowercase().ends_with(".app") {
            return Some(&path[..end]);
        }
        pos = end + 1;
    }
    None
}

/// Папка установки над папкой исполняемого файла: версии (`22.1.0`, `app-1.2`) и `bin` внутри версии пропускаются.
fn install_folder(dir: &str) -> String {
    let mut folder = dir.to_string();
    let mut stripped = false;
    while let Some(up) = parent(&folder) {
        let name = last_segment(&folder);
        let bin_of_version = name == "bin" && is_version_dir(last_segment(up));
        if !(is_version_dir(name) || bin_of_version || (stripped && name.eq_ignore_ascii_case("versions"))) {
            break;
        }
        folder = up.to_string();
        stripped = true;
    }
    folder
}

fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(p, _)| p).filter(|p| !p.is_empty())
}

fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Папки, где лежит много чужих программ или данных.
fn is_broad_folder(folder: &str) -> bool {
    const BROAD: [&str; 43] = [
        "applications", "utilities", "system", "library", "users", "shared", "usr", "bin", "sbin", "lib", "libexec", "share", "local",
        "opt", "homebrew", "cellar", "caskroom", "private", "var", "tmp", "etc", "volumes", "desktop", "downloads", "documents", "music",
        "pictures", "movies", "public", "application support", "caches", "frameworks", "privilegedhelpertools", "launchagents",
        "launchdaemons", "containers", "group containers", "mobile documents", "icloud drive", "onedrive", "dropbox", "google drive",
        "yandex.disk",
    ];
    let parts: Vec<&str> = folder.split('/').filter(|p| !p.is_empty()).collect();
    let Some(last) = parts.last() else { return true };
    parts.len() <= 1
        || (parts.len() == 2 && (parts[0].eq_ignore_ascii_case("users") || parts[0].eq_ignore_ascii_case("volumes")))
        || BROAD.contains(&last.to_lowercase().as_str())
}

/// Регулярное выражение для всех файлов внутри папки программы. Файловая система macOS обычно
/// не различает регистр, поэтому и выражение без учёта регистра.
///
/// Пакет `.app` узнаётся и там, куда его переносит App Translocation: программу из «Загрузок» или
/// с образа диска, которую не перенесли в «Программы», macOS запускает из случайной папки
/// `/private/var/folders/…/AppTranslocation/<UUID>/d/Имя.app`, и путь из правила с ней не совпал бы.
pub fn folder_regex(folder: &str) -> String {
    use crate::compile::push_escaped;
    let folder = folder.trim_end_matches('/');
    let mut re = String::from("(?i)^");
    match outer_bundle(folder).filter(|b| *b == folder) {
        Some(bundle) => {
            re.push_str("(?:");
            push_escaped(&mut re, folder);
            re.push_str("|(?:/private)?/var/folders/.+/AppTranslocation/[^/]+/d/");
            push_escaped(&mut re, last_segment(bundle));
            re.push(')');
        }
        None => push_escaped(&mut re, folder),
    }
    re.push_str("/.+$");
    re
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundles_cover_their_helpers() {
        let ok = |p: &str| program_folder(p, None).unwrap();
        assert_eq!(ok("/Applications/Discord.app/Contents/MacOS/Discord"), "/Applications/Discord.app");
        assert_eq!(
            ok("/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/131.0.6778.86/Helpers/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper"),
            "/Applications/Google Chrome.app"
        );
        assert_eq!(ok("/Applications/Telegram.app/"), "/Applications/Telegram.app");
        assert_eq!(ok("\"/Users/a/Applications/Roblox.app\""), "/Users/a/Applications/Roblox.app");
        assert_eq!(ok("/Applications/Utilities/Terminal.app"), "/Applications/Utilities/Terminal.app");
    }

    #[test]
    fn bare_programs_use_their_install_folder() {
        let file = |p: &str| program_folder(p, Some(true));
        assert_eq!(file("/opt/homebrew/Cellar/node/22.1.0/bin/node").unwrap(), "/opt/homebrew/Cellar/node");
        assert_eq!(file("/Users/a/Games/Minecraft/runtime/java").unwrap(), "/Users/a/Games/Minecraft/runtime");
        assert_eq!(program_folder("/Users/a/Games/Minecraft", Some(false)).unwrap(), "/Users/a/Games/Minecraft");
        assert_eq!(file("/usr/local/bin/curl"), Err(InputError::TooBroad));
        assert_eq!(file("/Users/a/Downloads/tool"), Err(InputError::TooBroad));
    }

    #[test]
    fn broad_and_bad_folders_are_refused() {
        assert_eq!(program_folder("/Applications", None), Err(InputError::TooBroad));
        assert_eq!(program_folder("/", None), Err(InputError::TooBroad));
        assert_eq!(program_folder("/Users/a", None), Err(InputError::TooBroad));
        assert_eq!(program_folder("/Volumes/Games", None), Err(InputError::TooBroad));
        assert_eq!(program_folder("/Users/a/Library/Application Support", None), Err(InputError::TooBroad));
        assert_eq!(program_folder("Discord.app", None), Err(InputError::BadPath));
        assert_eq!(program_folder("/Applications/../etc", None), Err(InputError::BadPath));
        assert_eq!(program_folder("  ", None), Err(InputError::Empty));
    }

    #[test]
    fn regex_matches_everything_inside() {
        assert_eq!(
            folder_regex("/Applications/Google Chrome.app"),
            r"(?i)^(?:/Applications/Google Chrome\.app|(?:/private)?/var/folders/.+/AppTranslocation/[^/]+/d/Google Chrome\.app)/.+$"
        );
        assert_eq!(folder_regex("/Users/a/Games/Game, Inc (x64)/"), r"(?i)^/Users/a/Games/Game\x2c Inc \(x64\)/.+$");
    }
}
