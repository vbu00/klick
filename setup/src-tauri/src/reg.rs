//! Реестр: запись kl!ck в «Установленных приложениях» и системный прокси.

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::*;

pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\kl!ck";
pub const INET_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

pub fn open(root: HKEY, path: &str, write: bool) -> Option<Key> {
    let mut h: HKEY = std::ptr::null_mut();
    let access = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
    let r = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, access | KEY_WOW64_64KEY, &mut h) };
    (r == ERROR_SUCCESS).then_some(Key(h))
}

pub fn hklm(path: &str, write: bool) -> Option<Key> {
    open(HKEY_LOCAL_MACHINE, path, write)
}
pub fn hkcu(path: &str, write: bool) -> Option<Key> {
    open(HKEY_CURRENT_USER, path, write)
}

impl Key {
    pub fn get_string(&self, name: &str) -> Option<String> {
        let n = wide(name);
        let (mut size, mut ty) = (0u32, 0u32);
        let r = unsafe { RegQueryValueExW(self.0, n.as_ptr(), std::ptr::null(), &mut ty, std::ptr::null_mut(), &mut size) };
        if r != ERROR_SUCCESS || (ty != REG_SZ && ty != REG_EXPAND_SZ) {
            return None;
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
        let r = unsafe { RegQueryValueExW(self.0, n.as_ptr(), std::ptr::null(), &mut ty, buf.as_mut_ptr() as *mut u8, &mut size) };
        if r != ERROR_SUCCESS {
            return None;
        }
        let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    pub fn get_dword(&self, name: &str) -> Option<u32> {
        let (mut v, mut size, mut ty) = (0u32, 4u32, 0u32);
        let r = unsafe { RegQueryValueExW(self.0, wide(name).as_ptr(), std::ptr::null(), &mut ty, &mut v as *mut u32 as *mut u8, &mut size) };
        (r == ERROR_SUCCESS && ty == REG_DWORD).then_some(v)
    }

    pub fn set_string(&self, name: &str, v: &str) -> bool {
        let w = wide(v);
        unsafe { RegSetValueExW(self.0, wide(name).as_ptr(), 0, REG_SZ, w.as_ptr() as *const u8, (w.len() * 2) as u32) == ERROR_SUCCESS }
    }

    pub fn set_dword(&self, name: &str, v: u32) -> bool {
        unsafe { RegSetValueExW(self.0, wide(name).as_ptr(), 0, REG_DWORD, &v as *const u32 as *const u8, 4) == ERROR_SUCCESS }
    }

    pub fn delete(&self, name: &str) {
        unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) };
    }
}

/// Браузеры перечитывают настройки прокси только по этому сигналу.
pub fn proxy_changed() {
    use windows_sys::Win32::Networking::WinInet::{InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED};
    unsafe {
        InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null(), 0);
        InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_REFRESH, std::ptr::null(), 0);
    }
}
