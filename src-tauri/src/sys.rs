//! Тонкие обёртки над системными утилитами Windows. Взято из Klutz: те же
//! абсолютные пути к утилитам и та же расшифровка вывода консоли.

use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Системный ГСЧ.
#[cfg(target_os = "windows")]
pub fn os_random(buf: &mut [u8]) -> bool {
    use windows_sys::Win32::Security::Cryptography::ProcessPrng;
    unsafe { ProcessPrng(buf.as_mut_ptr(), buf.len()) != 0 }
}

#[cfg(not(target_os = "windows"))]
pub fn os_random(buf: &mut [u8]) -> bool {
    use std::io::Read;
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(buf)).is_ok()
}

/// Случайная hex-строка из `bytes` байт: id профилей, секрет Clash API.
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if !os_random(&mut buf) {
        // ГСЧ ОС не отказывает на практике; на крайний случай — время.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (t >> ((i % 16) * 8)) as u8 ^ (i as u8).wrapping_mul(151);
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Каталог Windows — у самой системы, а не из переменной окружения: её
/// значение наследуется от того, кто нас запустил, а утилиты мы запускаем
/// с правами администратора.
#[cfg(target_os = "windows")]
fn windows_dir() -> std::path::PathBuf {
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0u16; 260];
    let n = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return std::path::PathBuf::from("C:\\Windows");
    }
    let sys32 = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    sys32.parent().map(|p| p.to_path_buf()).unwrap_or(sys32)
}

#[cfg(not(target_os = "windows"))]
fn windows_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("C:\\Windows")
}

/// Абсолютный путь к системной утилите — без поиска по текущему каталогу и PATH.
pub fn system_exe(name: &str) -> std::path::PathBuf {
    let root = windows_dir();
    for candidate in [
        root.join("System32").join(name),
        root.join("System32").join("WindowsPowerShell").join("v1.0").join(name),
        root.join(name),
    ] {
        if candidate.exists() {
            return candidate;
        }
    }
    root.join("System32").join(name)
}

/// Вывод консольной утилиты: UTF-8, а если не вышло — кодовая страница OEM
/// (на русской Windows netsh и sc пишут в CP866).
pub fn decode_console(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    #[cfg(target_os = "windows")]
    if let Some(s) = decode_oem(bytes) {
        return s;
    }
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(target_os = "windows")]
fn decode_oem(bytes: &[u8]) -> Option<String> {
    use windows_sys::Win32::Globalization::MultiByteToWideChar;
    const CP_OEMCP: u32 = 1;
    if bytes.is_empty() {
        return Some(String::new());
    }
    unsafe {
        let need = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        if need <= 0 {
            return None;
        }
        let mut buf = vec![0u16; need as usize];
        let got = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, buf.as_mut_ptr(), need);
        if got <= 0 {
            return None;
        }
        String::from_utf16(&buf[..got as usize]).ok()
    }
}

/// Читает поток построчно, не теряя строки с кириллицей.
pub fn for_each_line<R: std::io::Read>(r: R, mut f: impl FnMut(String)) {
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = decode_console(&buf);
        f(line.trim_end_matches(['\r', '\n']).to_string());
    }
}

pub fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(system_exe(program));
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Запускает утилиту, отдаёт stdout+stderr. Код выхода не проверяет.
pub fn run(program: &str, args: &[&str]) -> String {
    match command(program).args(args).output() {
        Ok(out) => {
            let mut s = decode_console(&out.stdout);
            s.push_str(&decode_console(&out.stderr));
            s
        }
        Err(_) => String::new(),
    }
}

pub fn run_ok(program: &str, args: &[&str]) -> bool {
    command(program).args(args).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Одна команда PowerShell. Ненулевой код выхода — ошибка с текстом stderr.
///
/// Скрипт уходит через -EncodedCommand (UTF-16LE в base64), а не строкой в
/// -Command: так не нужно думать о кавычках, и кириллица в именах адаптеров
/// доезжает целой.
pub fn powershell(script: &str) -> Result<String, String> {
    // Вывод — в UTF-8, иначе имена адаптеров по-русски приходят ромбиками.
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; $ProgressPreference='SilentlyContinue'; {script}");
    let utf16: Vec<u8> = full.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let out = command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &base64(&utf16)])
        .output()
        .map_err(|e| format!("не удалось запустить PowerShell: {e}"))?;
    let stdout = decode_console(&out.stdout);
    if out.status.success() {
        Ok(stdout)
    } else {
        let stderr = decode_console(&out.stderr);
        let msg = if stderr.trim().is_empty() { stdout.trim().to_string() } else { first_error_line(&stderr) };
        Err(if msg.is_empty() { format!("PowerShell завершился с кодом {}", out.status.code().unwrap_or(-1)) } else { msg })
    }
}

/// Из многострочной ошибки PowerShell — только суть, без «At line:1 char:…».
fn first_error_line(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("At line") && !l.starts_with("+"))
        .unwrap_or("")
        .to_string()
}

/// Строка для одинарных кавычек PowerShell: ' удваивается.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

/// base64 в обоих алфавитах (обычный и URL-safe), с паддингом и без,
/// с переводами строк внутри — как их отдают панели подписок.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_туда_и_обратно() {
        for s in ["", "a", "ab", "abc", "abcd", "метод:пароль"] {
            assert_eq!(base64_decode(&base64(s.as_bytes())).unwrap(), s.as_bytes());
        }
        // URL-safe без паддинга
        assert_eq!(base64_decode("YWI_Pz8_").unwrap(), b"ab????");
        assert_eq!(base64_decode("YWI").unwrap(), b"ab");
        assert!(base64_decode("не base64").is_none());
    }
}
