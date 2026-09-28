//! Системный прокси пользователя, когда окна kl!ck нет: служба правит раздел реестра того,
//! кто сейчас вошёл в Windows. Обычно это делает окно — оно ещё и сразу оповещает программы;
//! служба — страховка: чтобы режим «Системный прокси» работал без окна и прокси не остался
//! смотреть на выключенный порт.

use crate::win::wide;
use klick_proto::SystemProxy;
use std::ffi::c_void;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_USER};
use windows::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_USERS, KEY_READ, KEY_WRITE, REG_DWORD, REG_SZ, REG_VALUE_TYPE};
use windows::Win32::System::RemoteDesktop::{
    WTSEnumerateSessionsW, WTSFreeMemory, WTSGetActiveConsoleSessionId, WTSQueryUserToken, WTSActive, WTS_CURRENT_SERVER_HANDLE, WTS_SESSION_INFOW,
};

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

/// Что стояло у пользователя до kl!ck — чтобы вернуть как было. Лежит и в папке данных службы:
/// по этой отметке служба после перезапуска знает, что прокси ставила она, а не другая программа.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Saved {
    enable: u32,
    server: Option<String>,
    bypass: Option<String>,
}

/// Поставить прокси пользователю, который сейчас за компьютером. `None` — никто не вошёл
/// (или служба запущена не от имени системы, как в разработке).
pub fn apply(p: &SystemProxy) -> Option<Saved> {
    let key = open_user_key()?;
    let saved = Saved { enable: key.dword("ProxyEnable").unwrap_or(0), server: key.string("ProxyServer"), bypass: key.string("ProxyOverride") };
    let ours = format!("{}:{}", p.host, p.port);
    if saved.enable == 1 && saved.server.as_deref() == Some(ours.as_str()) {
        return Some(Saved::default());
    }
    key.set_string("ProxyServer", &ours);
    key.set_string("ProxyOverride", &p.bypass.join(";"));
    key.set_dword("ProxyEnable", 1);
    Some(saved)
}

/// Снять наш прокси: вернуть то, что было, или просто выключить. Чужой прокси не трогаем.
pub fn clear(port: u16, saved: Option<Saved>) -> bool {
    let Some(key) = open_user_key() else { return false };
    let ours = key.string("ProxyServer").is_some_and(|s| s == format!("127.0.0.1:{port}"));
    if !ours || key.dword("ProxyEnable") != Some(1) {
        return false;
    }
    match saved {
        Some(s) if s.server.is_some() => {
            key.set_string("ProxyServer", s.server.as_deref().unwrap_or_default());
            key.set_string("ProxyOverride", s.bypass.as_deref().unwrap_or_default());
            key.set_dword("ProxyEnable", s.enable);
        }
        _ => key.set_dword("ProxyEnable", 0),
    }
    true
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

impl Key {
    fn dword(&self, name: &str) -> Option<u32> {
        let n = wide(name);
        let mut ty = REG_VALUE_TYPE::default();
        let mut v = 0u32;
        let mut size = 4u32;
        let rc = unsafe { RegQueryValueExW(self.0, PCWSTR(n.as_ptr()), None, Some(&mut ty), Some(&mut v as *mut u32 as *mut u8), Some(&mut size)) };
        (rc == ERROR_SUCCESS && ty == REG_DWORD).then_some(v)
    }

    fn string(&self, name: &str) -> Option<String> {
        let n = wide(name);
        let mut ty = REG_VALUE_TYPE::default();
        let mut buf = [0u16; 2048];
        let mut size = (buf.len() * 2) as u32;
        let rc = unsafe { RegQueryValueExW(self.0, PCWSTR(n.as_ptr()), None, Some(&mut ty), Some(buf.as_mut_ptr() as *mut u8), Some(&mut size)) };
        if rc != ERROR_SUCCESS || ty != REG_SZ {
            return None;
        }
        let len = (size as usize / 2).min(buf.len());
        let end = buf[..len].iter().position(|c| *c == 0).unwrap_or(len);
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    fn set_dword(&self, name: &str, v: u32) {
        let n = wide(name);
        unsafe {
            let _ = RegSetValueExW(self.0, PCWSTR(n.as_ptr()), 0, REG_DWORD, Some(&v.to_le_bytes()));
        }
    }

    fn set_string(&self, name: &str, v: &str) {
        let n = wide(name);
        let data: Vec<u8> = wide(v).iter().flat_map(|c| c.to_le_bytes()).collect();
        unsafe {
            let _ = RegSetValueExW(self.0, PCWSTR(n.as_ptr()), 0, REG_SZ, Some(&data));
        }
    }
}

fn open_user_key() -> Option<Key> {
    let sid = active_user_sid()?;
    let path = wide(&format!(r"{sid}\{KEY}"));
    let mut key = HKEY::default();
    let rc = unsafe { RegOpenKeyExW(HKEY_USERS, PCWSTR(path.as_ptr()), 0, KEY_READ | KEY_WRITE, &mut key) };
    (rc == ERROR_SUCCESS).then_some(Key(key))
}

/// Сеанс, в котором сейчас работают: активный, а не обязательно «консольный» — через удалённый
/// рабочий стол (и в Песочнице Windows) человек сидит в удалённом сеансе.
fn active_session() -> Option<u32> {
    unsafe {
        let mut list: *mut WTS_SESSION_INFOW = std::ptr::null_mut();
        let mut count = 0u32;
        let mut found = None;
        if WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut list, &mut count).is_ok() && !list.is_null() {
            found = std::slice::from_raw_parts(list, count as usize).iter().find(|s| s.State == WTSActive && s.SessionId != 0).map(|s| s.SessionId);
            WTSFreeMemory(list as *mut c_void);
        }
        found.or_else(|| Some(WTSGetActiveConsoleSessionId()).filter(|s| *s != u32::MAX))
    }
}

/// SID пользователя, который сидит за компьютером. Токен сеанса выдают только службе от имени системы.
fn active_user_sid() -> Option<String> {
    unsafe {
        let session = active_session()?;
        let mut token = HANDLE::default();
        WTSQueryUserToken(session, &mut token).ok()?;
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let ok = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as *mut c_void), len, &mut len).is_ok();
        let _ = CloseHandle(token);
        if !ok {
            return None;
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut s).ok()?;
        let out = s.to_string().ok();
        let _ = LocalFree(HLOCAL(s.0 as *mut c_void));
        out
    }
}
