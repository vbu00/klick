//! Kill Switch на macOS: что есть в папке программы. Блокирует не служба напрямую, а ядро-страж
//! (`compile::compile_guard`) по правилам с путём программы: постоянных фильтров по программам,
//! как WFP на Windows, у macOS без Network Extension нет. Здесь — только проверка, что программа на месте.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Папки внутри пакета, где исполняемых файлов не бывает, а файлов — тысячи.
const SKIP: [&str; 4] = ["Resources", "_CodeSignature", "Headers", "Modules"];

/// Исполняемые файлы программы: у пакета `.app` — главные файлы и помощники
/// (`Contents/MacOS`, вложенные `.app`), у папки — всё с правом на запуск.
pub fn scan_folder(folder: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
        if depth > 10 || out.len() >= 500 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || SKIP.contains(&name.as_ref()) || name.ends_with(".lproj") {
                continue;
            }
            // Символьные ссылки не проходим: в пакетах фреймворков они ведут на ту же версию.
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_dir() {
                walk(&path, depth + 1, out);
            } else if ft.is_file() && e.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0) && !is_library(&name) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(folder, 0, &mut out);
    out.sort();
    out
}

/// Библиотеки с правом на запуск — не программы.
fn is_library(name: &str) -> bool {
    [".dylib", ".so", ".node", ".sh", ".py"].iter().any(|x| name.ends_with(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_executables_in_a_bundle() {
        let root = std::env::temp_dir().join(format!("klick-ks-{}", std::process::id()));
        let app = root.join("Test.app/Contents");
        std::fs::create_dir_all(app.join("MacOS")).unwrap();
        std::fs::create_dir_all(app.join("Resources")).unwrap();
        std::fs::create_dir_all(app.join("Frameworks/Test Helper.app/Contents/MacOS")).unwrap();
        let exe = |p: &Path| {
            std::fs::write(p, b"x").unwrap();
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        exe(&app.join("MacOS/Test"));
        exe(&app.join("Frameworks/Test Helper.app/Contents/MacOS/Test Helper"));
        exe(&app.join("Resources/tool"));
        exe(&app.join("Frameworks/libx.dylib"));
        std::fs::write(app.join("Info.plist"), b"x").unwrap();
        let found = scan_folder(&root.join("Test.app"));
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(scan_folder(&root.join("Missing.app")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
