//! «Запущено сейчас»: программы с открытыми соединениями — для выбора в список правил и в Kill Switch.
//! Берём владельцев TCP- и UDP-сокетов, а не все процессы: в списке остаются те, кто ходит в сеть.
//! Служба работает от имени системы, поэтому видит и программы, запущенные от администратора.

use crate::neighbors::process_path;
use crate::win::wide;
use klick_core::program_folder;
use klick_proto::ProgramView;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID, MIB_UDP6TABLE_OWNER_PID, MIB_UDPTABLE_OWNER_PID,
    TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const TCP_STATE_LISTEN: u32 = 2;

/// Программы с сетевой активностью, самые активные сверху. Системные программы Windows
/// и сам kl!ck (`own` — его exe) в список не попадают.
pub fn scan(own: &[PathBuf]) -> Vec<ProgramView> {
    let mut per_pid: HashMap<u32, u32> = HashMap::new();
    for pid in socket_owners() {
        *per_pid.entry(pid).or_default() += 1;
    }
    let windows_dir = format!("{}\\", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()).to_lowercase());
    let own_dirs: Vec<&Path> = own.iter().filter_map(|p| p.parent()).collect();
    let mut per_path: HashMap<String, u32> = HashMap::new();
    for (pid, n) in per_pid {
        if pid <= 4 {
            continue;
        }
        let Some(path) = process_path(pid) else { continue };
        if path.to_lowercase().starts_with(&windows_dir) || Path::new(&path).parent().is_some_and(|d| own_dirs.contains(&d)) {
            continue;
        }
        *per_path.entry(path).or_default() += n;
    }
    let mut out: Vec<ProgramView> = per_path
        .into_iter()
        .map(|(path, connections)| ProgramView { name: describe(&path), folder: program_folder(&path).ok(), path, connections })
        .collect();
    out.sort_by(|a, b| b.connections.cmp(&a.connections).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// Владельцы открытых сокетов, по записи на сокет. Слушающие TCP-сокеты не считаем: это не активность.
fn socket_owners() -> Vec<u32> {
    let mut out = Vec::new();
    unsafe {
        if let Some(buf) = table(|p, size| GetExtendedTcpTable(p, size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0)) {
            let t = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
            let rows = std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize);
            out.extend(rows.iter().filter(|r| r.dwState != TCP_STATE_LISTEN).map(|r| r.dwOwningPid));
        }
        if let Some(buf) = table(|p, size| GetExtendedTcpTable(p, size, false, AF_INET6.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0)) {
            let t = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
            let rows = std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize);
            out.extend(rows.iter().filter(|r| r.dwState != TCP_STATE_LISTEN).map(|r| r.dwOwningPid));
        }
        if let Some(buf) = table(|p, size| GetExtendedUdpTable(p, size, false, AF_INET.0 as u32, UDP_TABLE_OWNER_PID, 0)) {
            let t = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
            out.extend(std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize).iter().map(|r| r.dwOwningPid));
        }
        if let Some(buf) = table(|p, size| GetExtendedUdpTable(p, size, false, AF_INET6.0 as u32, UDP_TABLE_OWNER_PID, 0)) {
            let t = &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
            out.extend(std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize).iter().map(|r| r.dwOwningPid));
        }
    }
    out
}

/// Таблица из IP Helper: сначала узнаём размер, потом читаем. Буфер из u32 — чтобы строки были выровнены.
fn table(read: impl Fn(Option<*mut c_void>, *mut u32) -> u32) -> Option<Vec<u32>> {
    let mut size = 0u32;
    for _ in 0..4 {
        let mut buf = vec![0u32; (size as usize).div_ceil(4).max(1)];
        let rc = read(Some(buf.as_mut_ptr() as *mut c_void), &mut size);
        match rc {
            0 => return Some(buf),
            ERROR_INSUFFICIENT_BUFFER => continue,
            _ => return None,
        }
    }
    None
}

/// Название программы по папке правила: описание её главного exe («Node.js JavaScript Runtime», «Яндекс Музыка»).
/// Главный — тот, чьё имя совпадает с папкой, иначе первый не служебный exe (не установщик и не обновлятор).
pub fn folder_name(folder: &Path) -> Option<String> {
    let exes = crate::killswitch::scan_folder(folder);
    let stem = |p: &PathBuf| p.file_stem().map(|s| s.to_string_lossy().to_lowercase().replace(' ', "")).unwrap_or_default();
    let dir = folder.file_name()?.to_string_lossy().to_lowercase().replace(' ', "");
    let helper = |p: &PathBuf| {
        let s = stem(p);
        ["unins", "update", "crash", "helper", "setup", "install", "elevat", "report", "squirrel"].iter().any(|k| s.contains(k))
    };
    let main = exes
        .iter()
        .find(|p| stem(p) == dir)
        .or_else(|| exes.iter().filter(|p| p.parent() == Some(folder)).find(|p| !helper(p)))
        .or_else(|| exes.iter().find(|p| !helper(p)))?;
    file_description(&main.to_string_lossy())
}

/// Понятное название: описание из свойств exe («Discord», «Google Chrome»), иначе имя файла.
fn describe(path: &str) -> String {
    let stem = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string());
    file_description(path).filter(|d| d.chars().count() <= 48).unwrap_or(stem)
}

fn file_description(path: &str) -> Option<String> {
    unsafe {
        let file = wide(path);
        let size = GetFileVersionInfoSizeW(PCWSTR(file.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(file.as_ptr()), 0, size, data.as_mut_ptr() as *mut c_void).ok()?;
        let query = |key: &str| -> Option<(*const u16, usize)> {
            let key = wide(key);
            let mut ptr: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            let ok = VerQueryValueW(data.as_ptr() as *const c_void, PCWSTR(key.as_ptr()), &mut ptr, &mut len).as_bool();
            (ok && !ptr.is_null() && len > 0).then_some((ptr as *const u16, len as usize))
        };
        let mut langs: Vec<(u16, u16)> = Vec::new();
        // У Translation длина в байтах: пары «язык, кодовая страница» по два u16.
        if let Some((ptr, bytes)) = query(r"\VarFileInfo\Translation") {
            langs.extend(std::slice::from_raw_parts(ptr, bytes / 2).chunks_exact(2).map(|c| (c[0], c[1])));
        }
        langs.extend([(0x0409, 0x04B0), (0x0409, 0x04E4), (0x0000, 0x04B0)]);
        for (lang, cp) in langs {
            // У строк длина в символах, вместе с нулём в конце.
            if let Some((ptr, chars)) = query(&format!(r"\StringFileInfo\{lang:04x}{cp:04x}\FileDescription")) {
                let text = std::slice::from_raw_parts(ptr, chars);
                let end = text.iter().position(|c| *c == 0).unwrap_or(text.len());
                let s = String::from_utf16_lossy(&text[..end]).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}
