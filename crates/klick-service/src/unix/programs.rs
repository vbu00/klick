//! «Запущено сейчас» на macOS: программы с открытыми соединениями — для выбора в список правил
//! и в Kill Switch. Владельцев сокетов даёт `lsof` (служба работает от root и видит все процессы),
//! программа — пакет `.app` целиком: соединения помощников (`Google Chrome Helper`) идут в счёт программы.

use crate::sys;
use klick_core::{macos::outer_bundle, program_folder_on, Os};
use klick_proto::ProgramView;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const LSOF: &str = if cfg!(target_os = "macos") { "/usr/sbin/lsof" } else { "lsof" };

/// Системные программы: в правила их не добавляют, в списке они только мешают.
const SYSTEM: [&str; 6] = ["/System/", "/usr/libexec/", "/usr/sbin/", "/usr/bin/", "/sbin/", "/bin/"];

/// Программы с сетевой активностью, самые активные сверху. Системные программы и сам kl!ck
/// (`own` — его исполняемые файлы) в список не попадают.
pub fn scan(own: &[PathBuf]) -> Vec<ProgramView> {
    let own_dirs: Vec<&Path> = own.iter().filter_map(|p| p.parent()).collect();
    let mut per_program: HashMap<String, (String, u32)> = HashMap::new();
    for (pid, n) in socket_owners() {
        let Some(path) = sys::process_path(pid) else { continue };
        if SYSTEM.iter().any(|s| path.starts_with(s)) || Path::new(&path).parent().is_some_and(|d| own_dirs.contains(&d)) {
            continue;
        }
        // Ключ — пакет программы, иначе сам файл.
        let key = outer_bundle(&path).map(str::to_string).unwrap_or_else(|| path.clone());
        let entry = per_program.entry(key.clone()).or_insert_with(|| (key, 0));
        entry.1 += n;
    }
    let mut out: Vec<ProgramView> = per_program
        .into_values()
        .map(|(path, connections)| {
            let is_file = outer_bundle(&path).is_none();
            ProgramView { name: describe(&path), folder: program_folder_on(Os::MacOs, &path, Some(is_file)).ok(), path, connections }
        })
        .collect();
    out.sort_by(|a, b| b.connections.cmp(&a.connections).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// Сколько сетевых сокетов у каждого процесса. Слушающие TCP-сокеты не считаем: это не активность.
fn socket_owners() -> Vec<(i32, u32)> {
    let out = match sys::run_within(LSOF, &["-n", "-P", "-w", "-i", "-F", "pfT"], None, std::time::Duration::from_secs(10)) {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("lsof не запустился: {e}");
            return Vec::new();
        }
    };
    parse_lsof(&String::from_utf8_lossy(&out.stdout))
}

/// Вывод `lsof -F pfT`: `p<pid>`, затем по файлу `f<fd>` и строки `TST=<состояние>`.
fn parse_lsof(text: &str) -> Vec<(i32, u32)> {
    let mut counts: HashMap<i32, u32> = HashMap::new();
    let mut pid = 0;
    let mut pending = false;
    let flush = |pid: i32, pending: &mut bool, counts: &mut HashMap<i32, u32>| {
        if *pending && pid > 0 {
            *counts.entry(pid).or_default() += 1;
        }
        *pending = false;
    };
    for line in text.lines() {
        match line.as_bytes().first() {
            Some(b'p') => {
                flush(pid, &mut pending, &mut counts);
                pid = line[1..].parse().unwrap_or(0);
            }
            Some(b'f') => {
                flush(pid, &mut pending, &mut counts);
                pending = true;
            }
            Some(b'T') if line == "TST=LISTEN" => pending = false,
            _ => {}
        }
    }
    flush(pid, &mut pending, &mut counts);
    let mut v: Vec<(i32, u32)> = counts.into_iter().collect();
    v.sort();
    v
}

/// Название программы по папке правила: имя пакета `.app` («Telegram», «Яндекс Музыка»).
pub fn folder_name(folder: &Path) -> Option<String> {
    let s = folder.to_string_lossy();
    outer_bundle(&s).and_then(|b| bundle_name(Path::new(b)))
}

/// Понятное название: имя пакета, иначе имя файла.
fn describe(path: &str) -> String {
    if let Some(name) = outer_bundle(path).and_then(|b| bundle_name(Path::new(b))) {
        return name;
    }
    Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

/// Имя пакета: отображаемое из Info.plist («Google Chrome»), иначе имя папки без `.app`.
/// Имя папки — то, что человек видит в Finder, поэтому оно важнее короткого `CFBundleName`.
pub fn bundle_name(bundle: &Path) -> Option<String> {
    let display = read_info_plist(&bundle.join("Contents").join("Info.plist"))
        .and_then(|v| v.as_dictionary().and_then(|d| d.get("CFBundleDisplayName")).and_then(|n| n.as_string()).map(str::to_string))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().count() <= 48);
    display.or_else(|| bundle.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()))
}

/// Info.plist из папки пользователя служба читает от root: только обычный файл и не больше 1 МБ.
/// `O_NONBLOCK` — чтобы подложенный вместо файла канал (FIFO) не повесил службу на открытии.
fn read_info_plist(path: &Path) -> Option<plist::Value> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    const MAX: u64 = 1 << 20;
    let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX {
        return None;
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(MAX).read_to_end(&mut bytes).ok()?;
    plist::Value::from_reader(std::io::Cursor::new(bytes)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsof_output_counts_active_sockets() {
        let text = "p101\nf12\nTST=ESTABLISHED\nTQR=0\nTQS=0\nf13\nTST=LISTEN\nf14\np202\nf5\nTST=LISTEN\np303\nf7\nTST=CLOSE_WAIT\n";
        assert_eq!(parse_lsof(text), vec![(101, 2), (303, 1)]);
        assert!(parse_lsof("").is_empty());
    }

    #[test]
    fn info_plist_must_be_a_regular_file() {
        let dir = std::env::temp_dir().join(format!("klick-plist-{}", std::process::id()));
        let contents = dir.join("Fifo.app").join("Contents");
        std::fs::create_dir_all(&contents).unwrap();
        let fifo = std::ffi::CString::new(contents.join("Info.plist").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
        // Канал без писателя: обычное открытие зависло бы навсегда.
        assert_eq!(bundle_name(&dir.join("Fifo.app")).as_deref(), Some("Fifo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundle_names_fall_back_to_folder() {
        assert_eq!(bundle_name(Path::new("/nonexistent/Яндекс Музыка.app")).as_deref(), Some("Яндекс Музыка"));
        assert_eq!(describe("/usr/local/Cellar/node/22.1.0/bin/node"), "node");
        assert_eq!(folder_name(Path::new("/Applications/Discord.app")).as_deref(), Some("Discord"));
        assert_eq!(folder_name(Path::new("/opt/homebrew/Cellar/node")), None);
    }
}
