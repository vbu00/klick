//! iOS и Android: то немногое о системе, что нужно движку. Прав администратора и чужих процессов нет,
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
#[cfg(target_os = "ios")]
pub fn os_short() -> (String, String) {
    ("iOS".into(), sysctl_string("kern.osproductversion").unwrap_or_default())
}

/// Модель — ей устройство и подписано в панели: «iPhone 17» (по коду `iPhone18,3`; незнакомая модель —
/// сам код). Имя телефона iOS приложениям не отдаёт.
#[cfg(target_os = "ios")]
pub fn device_name() -> Option<String> {
    let code = std::env::var("SIMULATOR_MODEL_IDENTIFIER").ok().or_else(|| sysctl_string("hw.machine")).filter(|m| !m.is_empty())?;
    Some(klick_core::apple::model_label(&code))
}

/// «iOS 18.1 · iPhone 14 Pro».
pub fn os_name() -> String {
    let (os, version) = os_short();
    format!("{os} {version} · {}", device_name().unwrap_or_else(|| std::env::consts::ARCH.into()))
}

pub fn tun_up() -> bool {
    super::host().is_some_and(|h| (h.tunnel_connected)())
}

/// Время загрузки системы, секунды Unix.
#[cfg(target_os = "ios")]
pub fn boot_time() -> i64 {
    let Some(raw) = sysctl_raw("kern.boottime") else { return 0 };
    if raw.len() < std::mem::size_of::<libc::timeval>() {
        return 0;
    }
    let tv: libc::timeval = unsafe { std::ptr::read_unaligned(raw.as_ptr() as *const libc::timeval) };
    tv.tv_sec as i64
}

#[cfg(target_os = "ios")]
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

#[cfg(target_os = "ios")]
fn sysctl_string(name: &str) -> Option<String> {
    let raw = sysctl_raw(name)?;
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    Some(String::from_utf8_lossy(&raw[..end]).into_owned())
}

/// Для заголовков подписки: («Android», «15»).
#[cfg(target_os = "android")]
pub fn os_short() -> (String, String) {
    ("Android".into(), android_prop("ro.build.version.release").unwrap_or_default())
}

/// «Samsung SM-S921B», «Xiaomi 23078PND5G»: так устройство и подписано в панели.
#[cfg(target_os = "android")]
pub fn device_name() -> Option<String> {
    let model = android_prop("ro.product.model").filter(|m| !m.is_empty())?;
    let brand = android_prop("ro.product.manufacturer").unwrap_or_default();
    if brand.is_empty() || model.to_lowercase().starts_with(&brand.to_lowercase()) {
        return Some(model);
    }
    let mut chars = brand.chars();
    let first = chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    Some(format!("{first}{} {model}", chars.as_str()))
}

/// Время загрузки системы, секунды Unix: сейчас минус время с загрузки (вместе со сном).
#[cfg(target_os = "android")]
pub fn boot_time() -> i64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    now - ts.tv_sec as i64
}

/// Системное свойство Android (`getprop`).
#[cfg(target_os = "android")]
fn android_prop(name: &str) -> Option<String> {
    extern "C" {
        fn __system_property_get(name: *const libc::c_char, value: *mut libc::c_char) -> libc::c_int;
    }
    let cname = std::ffi::CString::new(name).ok()?;
    let mut buf = [0 as libc::c_char; 92]; // PROP_VALUE_MAX
    let n = unsafe { __system_property_get(cname.as_ptr(), buf.as_mut_ptr()) };
    (n > 0).then(|| unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned())
}
