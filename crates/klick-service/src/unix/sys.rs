//! Мелкие обёртки над macOS: права на папку данных, шифрование ключей, адаптер TUN, процессы,
//! версия системы и время загрузки. На Linux то же работает настолько, чтобы гонять службу
//! для разработки: рабочая платформа — macOS.

use anyhow::{anyhow, bail, Context, Result};
use std::ffi::CStr;
use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Адрес адаптера TUN, который поднимает ядро: первый адрес диапазона подменных адресов (`198.18.0.1/30`).
pub const TUN_ADDR: Ipv4Addr = Ipv4Addr::new(198, 18, 0, 1);

/// Свободный порт на 127.0.0.1 для служебных входов ядра.
pub fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Папка данных рабочей службы: только root. Там лежат ключи серверов, поэтому обычные программы
/// пользователя их читать не должны. Администратор по-прежнему может открыть её через sudo.
/// DNS-серверы, которыми сейчас пользуется система: основной резолвер из `scutil --dns`.
/// Без подменного адреса kl!ck (198.18.x.x) и адресов с зоной (`fe80::1%en0`).
pub fn system_dns_servers() -> Vec<String> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    match run_within("/usr/sbin/scutil", &["--dns"], None, std::time::Duration::from_secs(5)) {
        Ok(out) => parse_scutil_dns(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => Vec::new(),
    }
}

fn parse_scutil_dns(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_first = false;
    for line in text.lines() {
        let line = line.trim();
        // Первый блок «resolver #1» до раздела для отдельных интерфейсов — тот, что спрашивает система.
        if line.starts_with("DNS configuration (for scoped queries)") {
            break;
        }
        if line.starts_with("resolver #") {
            if in_first {
                break;
            }
            in_first = line == "resolver #1";
            continue;
        }
        if !in_first || !line.starts_with("nameserver[") {
            continue;
        }
        let Some(addr) = line.split_once(':').map(|(_, a)| a.trim()) else { continue };
        let Ok(ip) = addr.parse::<IpAddr>() else { continue };
        let ours = matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 198 && (v4.octets()[1] & 0xFE) == 18);
        if !ours && !ip.is_unspecified() && !out.contains(&ip.to_string()) {
            out.push(ip.to_string());
        }
    }
    out
}

/// Выполнить системную программу, но не дольше `limit`: зависшая (lsof на недоступном сетевом
/// диске, pfctl) убивается, и служба не встаёт вместе с ней.
/// Вывод читается сразу, чтобы большой вывод не упёрся в буфер канала.
pub fn run_within(program: &str, args: &[&str], input: Option<&str>, limit: std::time::Duration) -> std::io::Result<std::process::Output> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let read_all = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = read_all(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err = read_all(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let deadline = std::time::Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            tracing::warn!("{program} {} не ответил за {} с — остановлен", args.first().copied().unwrap_or(""), limit.as_secs());
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, format!("{program}: тайм-аут")));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    Ok(std::process::Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() })
}

/// Снять метку карантина (`com.apple.quarantine`) со всего дерева. Системным вызовом, а не
/// `/usr/bin/xattr`: тот написан на Python и на Mac без инструментов разработчика просит их поставить.
pub fn strip_quarantine(root: &Path) {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let name = c"com.apple.quarantine";
        let mut stack = vec![root.to_path_buf()];
        while let Some(path) = stack.pop() {
            if let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) {
                // Метки может и не быть (ENOATTR) — это нормально.
                unsafe { libc::removexattr(c.as_ptr(), name.as_ptr(), libc::XATTR_NOFOLLOW) };
            }
            // По ссылкам не идём: снимаем только внутри своего дерева.
            if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    stack.extend(entries.flatten().map(|e| e.path()));
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = root;
}

pub fn restrict_to_admins(path: &Path) -> Result<()> {
    std::os::unix::fs::chown(path, Some(0), Some(0)).with_context(|| format!("chown {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).with_context(|| format!("chmod {}", path.display()))
}

/// Папка для сокетов управления ядра: у сокета ядра нет пароля (mihomo не проверяет `secret` на Unix-сокете
/// и открывает его всем), поэтому закрываем саму папку — доступ только у владельца.
pub fn private_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("не создать {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).with_context(|| format!("chmod {}", dir.display()))
}

/// У службы launchd по умолчанию 256 открытых файлов, а ядру с сотнями соединений нужно больше.
/// Ядро наследует предел от службы.
pub fn raise_fd_limit() {
    const WANTED: libc::rlim_t = 16_384;
    unsafe {
        let mut lim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) != 0 {
            return;
        }
        if lim.rlim_cur >= WANTED {
            return;
        }
        let target = WANTED.min(lim.rlim_max);
        let new = libc::rlimit { rlim_cur: target, rlim_max: lim.rlim_max };
        if libc::setrlimit(libc::RLIMIT_NOFILE, &new) != 0 {
            tracing::warn!("не поднять предел открытых файлов до {target}");
        }
    }
}

// ── Ключи подписок ─────────────────────────────────────────────────────────

const SECRETS_MAGIC: &[u8; 4] = b"KLK1";

/// Шифрует данные AES-256-GCM ключом, привязанным к этому компьютеру (аппаратный UUID).
/// Файл при этом лежит в папке, открытой только root: защищает папка, а шифрование
/// не даёт прочитать ключи из копии папки на другом компьютере — как DPAPI на Windows.
pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    let cipher = Aes256Gcm::new_from_slice(&machine_key()?).map_err(|_| anyhow!("ключ шифрования"))?;
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| anyhow!("getrandom: {e}"))?;
    let sealed = cipher.encrypt(Nonce::from_slice(&nonce), data).map_err(|_| anyhow!("шифрование"))?;
    let mut out = Vec::with_capacity(4 + 12 + sealed.len());
    out.extend_from_slice(SECRETS_MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    if blob.len() < 4 + 12 + 16 || &blob[..4] != SECRETS_MAGIC {
        bail!("не тот формат файла ключей");
    }
    let cipher = Aes256Gcm::new_from_slice(&machine_key()?).map_err(|_| anyhow!("ключ шифрования"))?;
    cipher.decrypt(Nonce::from_slice(&blob[4..16]), &blob[16..]).map_err(|_| anyhow!("ключи зашифрованы на другом компьютере или повреждены"))
}

fn machine_key() -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let id = host_uuid().context("нет идентификатора компьютера")?;
    let mut h = Sha256::new();
    h.update(b"kl!ck secrets v1\0");
    h.update(&id);
    Ok(h.finalize().into())
}

#[cfg(target_os = "macos")]
fn host_uuid() -> Option<Vec<u8>> {
    let mut id = [0u8; 16];
    let wait = libc::timespec { tv_sec: 5, tv_nsec: 0 };
    let rc = unsafe { libc::gethostuuid(id.as_mut_ptr(), &wait) };
    (rc == 0 && id != [0u8; 16]).then(|| id.to_vec())
}

#[cfg(not(target_os = "macos"))]
fn host_uuid() -> Option<Vec<u8>> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|f| std::fs::read_to_string(f).ok())
        .map(|s| s.trim().as_bytes().to_vec())
        .filter(|v| !v.is_empty())
}

// ── Сеть ───────────────────────────────────────────────────────────────────

/// Адрес сетевого интерфейса.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IfAddr {
    pub name: String,
    pub addr: IpAddr,
    pub up: bool,
}

/// Все адреса всех интерфейсов (`getifaddrs`).
pub fn interfaces() -> Vec<IfAddr> {
    let mut out = Vec::new();
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return out;
        }
        let mut p = ifap;
        while !p.is_null() {
            let ifa = &*p;
            p = ifa.ifa_next;
            if ifa.ifa_addr.is_null() || ifa.ifa_name.is_null() {
                continue;
            }
            let addr = match i32::from((*ifa.ifa_addr).sa_family) {
                libc::AF_INET => {
                    let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    IpAddr::V4(Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)))
                }
                libc::AF_INET6 => {
                    let sin6 = &*(ifa.ifa_addr as *const libc::sockaddr_in6);
                    IpAddr::V6(std::net::Ipv6Addr::from(sin6.sin6_addr.s6_addr))
                }
                _ => continue,
            };
            let flags = ifa.ifa_flags as libc::c_int;
            let up = flags & libc::IFF_UP != 0 && flags & libc::IFF_RUNNING != 0;
            out.push(IfAddr { name: CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned(), addr, up });
        }
        libc::freeifaddrs(ifap);
    }
    out
}

/// Имя адаптера TUN ядра (`utun5`), если он поднят.
pub fn tun_interface() -> Option<String> {
    interfaces().into_iter().find(|i| i.addr == IpAddr::V4(TUN_ADDR)).map(|i| i.name)
}

pub fn tun_up() -> bool {
    tun_interface().is_some()
}

// ── Процессы ───────────────────────────────────────────────────────────────

/// Все процессы: pid.
#[cfg(target_os = "macos")]
pub fn pids() -> Vec<i32> {
    unsafe {
        let n = libc::proc_listallpids(std::ptr::null_mut(), 0);
        if n <= 0 {
            return Vec::new();
        }
        // Процессы появляются между вызовами: берём с запасом.
        let mut buf = vec![0i32; n as usize + 64];
        let bytes = (buf.len() * std::mem::size_of::<i32>()) as libc::c_int;
        let got = libc::proc_listallpids(buf.as_mut_ptr() as *mut libc::c_void, bytes);
        if got <= 0 {
            return Vec::new();
        }
        buf.truncate((got as usize).min(buf.len()));
        buf.retain(|p| *p > 0);
        buf
    }
}

#[cfg(not(target_os = "macos"))]
pub fn pids() -> Vec<i32> {
    std::fs::read_dir("/proc").map(|d| d.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).collect()).unwrap_or_default()
}

/// Полный путь исполняемого файла процесса.
#[cfg(target_os = "macos")]
pub fn process_path(pid: i32) -> Option<String> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut libc::c_void, buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    buf.truncate(n as usize);
    String::from_utf8(buf).ok()
}

#[cfg(not(target_os = "macos"))]
pub fn process_path(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe")).ok().map(|p| p.to_string_lossy().into_owned())
}

/// Завершить ядро, оставшееся от прошлого запуска (служба упала, а ядро продолжило жить):
/// иначе новое ядро не получит порт и адаптер. По pid-файлу, и только если это действительно ядро kl!ck.
pub fn kill_stale(pidfile: &Path, exe: &Path) {
    let Some(pid) = std::fs::read_to_string(pidfile).ok().and_then(|s| s.trim().parse::<i32>().ok()) else { return };
    let _ = std::fs::remove_file(pidfile);
    let ours = process_path(pid).is_some_and(|p| Path::new(&p) == exe || std::fs::canonicalize(&p).ok() == std::fs::canonicalize(exe).ok());
    if pid <= 1 || !ours {
        return;
    }
    tracing::warn!("осталось ядро от прошлого запуска (pid {pid}), завершаю");
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if unsafe { libc::kill(pid, 0) } != 0 {
            return;
        }
    }
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
}

// ── Система ────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn sysctl_raw(name: &str) -> Option<Vec<u8>> {
    let cname = std::ffi::CString::new(name).ok()?;
    let mut len: libc::size_t = 0;
    unsafe {
        if libc::sysctlbyname(cname.as_ptr(), std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) != 0 || len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        if libc::sysctlbyname(cname.as_ptr(), buf.as_mut_ptr() as *mut libc::c_void, &mut len, std::ptr::null_mut(), 0) != 0 {
            return None;
        }
        buf.truncate(len);
        Some(buf)
    }
}

#[cfg(target_os = "macos")]
fn sysctl_string(name: &str) -> Option<String> {
    let raw = sysctl_raw(name)?;
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    String::from_utf8(raw[..end].to_vec()).ok().filter(|s| !s.is_empty())
}

#[cfg(target_os = "macos")]
fn sysctl_int(name: &str) -> Option<i32> {
    let raw = sysctl_raw(name)?;
    (raw.len() >= 4).then(|| i32::from_ne_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Время загрузки системы, секунды Unix: так служба отличает свой сбой от перезагрузки.
#[cfg(target_os = "macos")]
pub fn boot_time() -> i64 {
    let Some(raw) = sysctl_raw("kern.boottime") else { return 0 };
    if raw.len() < std::mem::size_of::<libc::timeval>() {
        return 0;
    }
    let tv: libc::timeval = unsafe { std::ptr::read_unaligned(raw.as_ptr() as *const libc::timeval) };
    tv.tv_sec as i64
}

#[cfg(not(target_os = "macos"))]
pub fn boot_time() -> i64 {
    std::fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("btime ").and_then(|v| v.trim().parse().ok())))
        .unwrap_or(0)
}

/// «macOS 15.1 Sequoia · Apple Silicon».
#[cfg(target_os = "macos")]
pub fn os_name() -> String {
    let version = sysctl_string("kern.osproductversion").unwrap_or_default();
    let major: u32 = version.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0);
    let codename = match major {
        11 => "Big Sur",
        12 => "Monterey",
        13 => "Ventura",
        14 => "Sonoma",
        15 => "Sequoia",
        26 => "Tahoe",
        _ => "",
    };
    let translated = sysctl_int("sysctl.proc_translated") == Some(1);
    let arch = match std::env::consts::ARCH {
        "aarch64" => "Apple Silicon",
        "x86_64" if translated => "Intel (Rosetta)",
        "x86_64" => "Intel",
        other => other,
    };
    let name = [Some("macOS"), (!version.is_empty()).then_some(version.as_str()), (!codename.is_empty()).then_some(codename)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    format!("{name} · {arch}")
}

#[cfg(not(target_os = "macos"))]
pub fn os_name() -> String {
    let pretty = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())));
    format!("{} · {}", pretty.unwrap_or_else(|| "Linux".into()), std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip() {
        let Ok(sealed) = protect(b"https://panel.example/sub/abc") else { return };
        assert_eq!(&sealed[..4], SECRETS_MAGIC);
        assert_eq!(unprotect(&sealed).unwrap(), b"https://panel.example/sub/abc");
        let mut broken = sealed.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(unprotect(&broken).is_err());
        assert!(unprotect(b"KLK1").is_err());
    }

    #[test]
    fn system_dns_is_the_main_resolver() {
        let text = "\nDNS configuration\n\nresolver #1\n  search domain[0] : lan\n  nameserver[0] : 192.168.1.1\n  nameserver[1] : fe80::1%en0\n  nameserver[2] : 2a02:6b8::feed:0ff\n  if_index : 6 (en0)\n\nresolver #2\n  domain   : local\n  nameserver[0] : 10.0.0.1\n\nDNS configuration (for scoped queries)\n\nresolver #1\n  nameserver[0] : 8.8.4.4\n";
        assert_eq!(parse_scutil_dns(text), vec!["192.168.1.1".to_string(), "2a02:6b8::feed:ff".to_string()]);
        // Подменный DNS kl!ck — не системный.
        assert!(parse_scutil_dns("resolver #1\n  nameserver[0] : 198.18.0.2\n").is_empty());
    }

    #[test]
    fn hung_programs_are_stopped() {
        let t = std::time::Instant::now();
        let r = run_within("/bin/sleep", &["30"], None, std::time::Duration::from_millis(300));
        assert!(r.is_err() && t.elapsed() < std::time::Duration::from_secs(5));
        let ok = run_within("/bin/cat", &[], Some("привет"), std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(String::from_utf8_lossy(&ok.stdout), "привет");
        // Вывод больше буфера канала не подвешивает ожидание.
        let big = run_within("/bin/sh", &["-c", "head -c 300000 /dev/zero"], None, std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(big.stdout.len(), 300_000);
    }

    #[test]
    fn system_facts_are_readable() {
        assert!(boot_time() > 1_000_000_000);
        assert!(!os_name().is_empty());
        assert!(!interfaces().is_empty());
        let me = std::process::id() as i32;
        assert!(pids().contains(&me));
        assert!(process_path(me).is_some());
    }
}
