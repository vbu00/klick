//! Запущенные программы — для выбора в «Маршрутизации» и Kill Switch.

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Proc {
    /// Человеческое имя из описания файла: «Google Chrome».
    pub name: String,
    pub exe: String,
    pub pid: u32,
    pub path: String,
}

/// По одной записи на программу. Системное из каталога Windows не
/// показываем: правилу для svchost.exe в этом списке не место.
#[cfg(target_os = "windows")]
pub fn running() -> Vec<Proc> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};

    let windir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()).to_lowercase();
    let own = std::process::id();
    let mut out: Vec<Proc> = vec![];
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let pid = e.th32ProcessID;
            let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
            let exe = String::from_utf16_lossy(&e.szExeFile[..len]);
            if pid > 4 && pid != own && !out.iter().any(|p| p.exe.eq_ignore_ascii_case(&exe)) {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if !h.is_null() {
                    let mut buf = [0u16; 1024];
                    let mut size = buf.len() as u32;
                    if QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size) != 0 {
                        let path = String::from_utf16_lossy(&buf[..size as usize]);
                        if !path.to_lowercase().starts_with(&windir) {
                            let name = file_description(&path).unwrap_or_else(|| exe.trim_end_matches(".exe").trim_end_matches(".EXE").to_string());
                            out.push(Proc { name, exe: exe.clone(), pid, path });
                        }
                    }
                    CloseHandle(h);
                }
            }
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

#[cfg(not(target_os = "windows"))]
pub fn running() -> Vec<Proc> {
    vec![]
}

/// FileDescription из ресурса версии exe.
#[cfg(target_os = "windows")]
pub fn file_description(path: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, data.as_mut_ptr() as _) == 0 {
            return None;
        }
        // Первая пара язык/кодировка из \VarFileInfo\Translation.
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let tr: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
        let lang = if VerQueryValueW(data.as_ptr() as _, tr.as_ptr(), &mut ptr, &mut len) != 0 && len >= 4 {
            let p = ptr as *const u16;
            format!("{:04x}{:04x}", *p, *p.add(1))
        } else {
            "040904b0".into()
        };
        let key: Vec<u16> = format!("\\StringFileInfo\\{lang}\\FileDescription\0").encode_utf16().collect();
        if VerQueryValueW(data.as_ptr() as _, key.as_ptr(), &mut ptr, &mut len) == 0 || len == 0 {
            return None;
        }
        let s = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        let end = s.iter().position(|c| *c == 0).unwrap_or(s.len());
        let d = String::from_utf16_lossy(&s[..end]).trim().to_string();
        (!d.is_empty()).then_some(d)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn file_description(_path: &str) -> Option<String> {
    None
}

/// Для программы, выбранной файлом: имя, exe и путь.
pub fn from_path(path: &str) -> Proc {
    let exe = path.rsplit(['\\', '/']).next().unwrap_or(path).to_string();
    let name = file_description(path).unwrap_or_else(|| exe.trim_end_matches(".exe").to_string());
    Proc { name, exe, pid: 0, path: path.to_string() }
}
