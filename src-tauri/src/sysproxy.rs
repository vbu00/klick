//! Системный прокси Windows (HKCU\...\Internet Settings) для режима
//! «Системный прокси».
//!
//! В отличие от SingBoxGUI, который при отключении просто ставил
//! ProxyEnable=0, здесь прежние значения запоминаются и возвращаются: если у
//! человека был свой прокси (корпоративный, Fiddler), после отключения VPN он
//! остаётся на месте, а не выключается молча. Копия лежит в файле, чтобы
//! пережить и падение приложения.

use serde::{Deserialize, Serialize};
use std::path::Path;

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
const BACKUP: &str = "sysproxy-backup.json";
/// Локальные адреса и интранет — мимо прокси, как делают v2rayN и Hiddify.
const OVERRIDE: &str = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;<local>";

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
struct Saved {
    enable: u32,
    server: Option<String>,
    overrides: Option<String>,
}

fn ours(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

pub fn enable(dir: &Path, port: u16) -> Result<(), String> {
    let cur = read()?;
    // Уже наш — бэкап не перезаписываем, иначе «прежним» станет наш же прокси.
    if !(cur.enable == 1 && cur.server.as_deref() == Some(&ours(port))) {
        let _ = std::fs::write(dir.join(BACKUP), serde_json::to_vec(&cur).unwrap_or_default());
    }
    write(&Saved { enable: 1, server: Some(ours(port)), overrides: Some(OVERRIDE.into()) })?;
    notify();
    Ok(())
}

/// Вернуть как было — только если сейчас стоит наш прокси: чужую настройку,
/// сделанную после нас, не трогаем.
pub fn disable(dir: &Path, port: u16) {
    let Ok(cur) = read() else { return };
    let backup_path = dir.join(BACKUP);
    if cur.server.as_deref() != Some(&ours(port)) || cur.enable == 0 {
        let _ = std::fs::remove_file(&backup_path);
        return;
    }
    let prev: Saved = std::fs::read(&backup_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    // Прежний сервер — наш же (двойное включение без бэкапа) → просто выключить.
    let prev = if prev.server.as_deref() == Some(&ours(port)) { Saved { enable: 0, ..prev } } else { prev };
    // Прежний прокси — локальный порт другой программы (Hiddify, Clash),
    // которая уже закрыта: вернуть его — значит оставить браузеры без сети.
    // Адрес сохраняем, но выключенным.
    let prev = if prev.enable == 1 && prev.server.as_deref().is_some_and(dead_local) { Saved { enable: 0, ..prev } } else { prev };
    if write(&prev).is_ok() {
        let _ = std::fs::remove_file(&backup_path);
        notify();
    }
}

/// «127.0.0.1:12334» или «http=127.0.0.1:1;https=…»: все локальные адреса
/// в строке не отвечают. Нелокальный прокси не проверяем — не наше дело.
fn dead_local(server: &str) -> bool {
    let locals: Vec<std::net::SocketAddr> = server
        .split(';')
        .map(|part| part.rsplit('=').next().unwrap_or(part).trim())
        .filter_map(|hp| {
            let (host, port) = hp.rsplit_once(':')?;
            let port: u16 = port.parse().ok()?;
            let ip: std::net::IpAddr = if host.eq_ignore_ascii_case("localhost") { [127, 0, 0, 1].into() } else { host.parse().ok()? };
            ip.is_loopback().then(|| std::net::SocketAddr::new(ip, port))
        })
        .collect();
    !locals.is_empty() && locals.iter().all(|a| std::net::TcpStream::connect_timeout(a, std::time::Duration::from_millis(400)).is_err())
}

#[cfg(test)]
mod tests {
    #[test]
    fn мёртвый_локальный_прокси() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let alive = format!("127.0.0.1:{}", l.local_addr().unwrap().port());
        assert!(!super::dead_local(&alive));
        drop(l);
        assert!(super::dead_local(&alive));
        assert!(super::dead_local("http=localhost:1;https=127.0.0.1:1"));
        assert!(!super::dead_local("proxy.corp.local:3128"), "чужой сетевой прокси не трогаем");
    }
}

#[cfg(target_os = "windows")]
mod reg {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe { RegCloseKey(self.0) };
        }
    }

    pub fn open(path: &str) -> Result<Key, String> {
        let mut h: HKEY = std::ptr::null_mut();
        let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(path).as_ptr(), 0, KEY_READ | KEY_WRITE, &mut h) };
        if r != ERROR_SUCCESS {
            return Err(format!("реестр: ошибка {r}"));
        }
        Ok(Key(h))
    }

    impl Key {
        pub fn get_dword(&self, name: &str) -> Option<u32> {
            let mut v: u32 = 0;
            let mut size = 4u32;
            let mut ty = 0u32;
            let r = unsafe {
                RegQueryValueExW(self.0, wide(name).as_ptr(), std::ptr::null(), &mut ty, &mut v as *mut u32 as *mut u8, &mut size)
            };
            (r == ERROR_SUCCESS && ty == REG_DWORD).then_some(v)
        }

        pub fn get_string(&self, name: &str) -> Option<String> {
            let n = wide(name);
            let mut size = 0u32;
            let mut ty = 0u32;
            let r = unsafe { RegQueryValueExW(self.0, n.as_ptr(), std::ptr::null(), &mut ty, std::ptr::null_mut(), &mut size) };
            if r != ERROR_SUCCESS || (ty != REG_SZ && ty != REG_EXPAND_SZ) {
                return None;
            }
            let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
            let r = unsafe {
                RegQueryValueExW(self.0, n.as_ptr(), std::ptr::null(), &mut ty, buf.as_mut_ptr() as *mut u8, &mut size)
            };
            if r != ERROR_SUCCESS {
                return None;
            }
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..len]))
        }

        pub fn set_dword(&self, name: &str, v: u32) -> Result<(), String> {
            let r = unsafe { RegSetValueExW(self.0, wide(name).as_ptr(), 0, REG_DWORD, &v as *const u32 as *const u8, 4) };
            if r == ERROR_SUCCESS { Ok(()) } else { Err(format!("реестр: ошибка {r}")) }
        }

        pub fn set_string(&self, name: &str, v: &str) -> Result<(), String> {
            let w = wide(v);
            let r = unsafe { RegSetValueExW(self.0, wide(name).as_ptr(), 0, REG_SZ, w.as_ptr() as *const u8, (w.len() * 2) as u32) };
            if r == ERROR_SUCCESS { Ok(()) } else { Err(format!("реестр: ошибка {r}")) }
        }

        pub fn delete(&self, name: &str) {
            unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) };
        }
    }
}

#[cfg(target_os = "windows")]
fn read() -> Result<Saved, String> {
    let k = reg::open(KEY)?;
    Ok(Saved {
        enable: k.get_dword("ProxyEnable").unwrap_or(0),
        server: k.get_string("ProxyServer"),
        overrides: k.get_string("ProxyOverride"),
    })
}

#[cfg(target_os = "windows")]
fn write(s: &Saved) -> Result<(), String> {
    let k = reg::open(KEY)?;
    k.set_dword("ProxyEnable", s.enable)?;
    match &s.server {
        Some(v) => k.set_string("ProxyServer", v)?,
        None => k.delete("ProxyServer"),
    }
    match &s.overrides {
        Some(v) => k.set_string("ProxyOverride", v)?,
        None => k.delete("ProxyOverride"),
    }
    Ok(())
}

/// Браузеры перечитывают настройку только по этому сигналу.
#[cfg(target_os = "windows")]
fn notify() {
    use windows_sys::Win32::Networking::WinInet::{InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED};
    unsafe {
        InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null(), 0);
        InternetSetOptionW(std::ptr::null(), INTERNET_OPTION_REFRESH, std::ptr::null(), 0);
    }
}

#[cfg(not(target_os = "windows"))]
fn read() -> Result<Saved, String> {
    Err("только Windows".into())
}
#[cfg(not(target_os = "windows"))]
fn write(_: &Saved) -> Result<(), String> {
    Err("только Windows".into())
}
#[cfg(not(target_os = "windows"))]
fn notify() {}
