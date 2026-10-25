//! iOS: то немногое о системе, что нужно движку. Прав администратора и чужих процессов нет,
//! папки приложения закрыты от других программ самой системой.

use anyhow::{Context, Result};
use std::net::Ipv4Addr;
use std::path::Path;

#[path = "../unix/ifaddrs.rs"]
mod ifaddrs;
pub use ifaddrs::{interfaces, IfAddr};

/// Адрес адаптера, который поднимает расширение VPN (как у ядра на macOS).
pub const TUN_ADDR: Ipv4Addr = Ipv4Addr::new(198, 18, 0, 1);

/// Свободный порт на 127.0.0.1 для служебных входов ядра.
pub fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

pub fn is_elevated() -> bool {
    false
}

/// DNS системы расширению не нужен: адреса серверов ядро ищет через зашифрованный DNS (`compile`).
pub fn system_dns_servers() -> Vec<String> {
    Vec::new()
}

/// Папка данных — в контейнере приложения: её и так не видит ни одна другая программа.
pub fn restrict_to_admins(_path: &Path) -> Result<()> {
    Ok(())
}

/// Папки для сокетов ядра на iOS нет (вход управления — порт), а контейнер приложения и так закрыт.
pub fn private_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("не создать {}", dir.display()))
}

/// Ссылки подписок лежат в контейнере приложения: другие программы его не видят, а на диске он
/// зашифрован системой (Data Protection). Своё шифрование ключом с того же устройства ничего не добавило бы.
pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}

pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>> {
    Ok(blob.to_vec())
}

pub fn machine_id() -> Option<Vec<u8>> {
    let id = (super::host()?.device_id)()?;
    (!id.is_empty()).then(|| id.into_bytes())
}

/// Для заголовков подписки: («iOS», «18.1»).
pub fn os_short() -> (String, String) {
    ("iOS".into(), sysctl_string("kern.osproductversion").unwrap_or_default())
}

/// Модель — ей устройство и подписано в панели: `iPhone15,2`. Имя телефона iOS приложениям не отдаёт.
pub fn device_name() -> Option<String> {
    std::env::var("SIMULATOR_MODEL_IDENTIFIER").ok().or_else(|| sysctl_string("hw.machine")).filter(|m| !m.is_empty())
}

/// «iOS 18.1 · iPhone15,2».
pub fn os_name() -> String {
    let (os, version) = os_short();
    format!("{os} {version} · {}", device_name().unwrap_or_else(|| std::env::consts::ARCH.into()))
}

pub fn tun_up() -> bool {
    super::host().is_some_and(|h| (h.tunnel_connected)())
}

/// Время загрузки системы, секунды Unix.
pub fn boot_time() -> i64 {
    let Some(raw) = sysctl_raw("kern.boottime") else { return 0 };
    if raw.len() < std::mem::size_of::<libc::timeval>() {
        return 0;
    }
    let tv: libc::timeval = unsafe { std::ptr::read_unaligned(raw.as_ptr() as *const libc::timeval) };
    tv.tv_sec as i64
}

fn sysctl_raw(name: &str) -> Option<Vec<u8>> {
    let cname = std::ffi::CString::new(name).ok()?;
    let mut len: libc::size_t = 0;
    unsafe {
        if libc::sysctlbyname(cname.as_ptr(), std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) != 0 || len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        if libc::sysctlbyname(cname.as_ptr(), buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) != 0 {
            return None;
        }
        buf.truncate(len);
        Some(buf)
    }
}

fn sysctl_string(name: &str) -> Option<String> {
    let raw = sysctl_raw(name)?;
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    Some(String::from_utf8_lossy(&raw[..end]).into_owned())
}
