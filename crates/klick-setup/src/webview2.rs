//! WebView2 Runtime — движок окон kl!ck и самого установщика. На Windows 11 он есть всегда,
//! а на урезанных сборках Windows 10 и в Песочнице Windows его может не быть: тогда окно не
//! открывается с ошибкой «Could not find the WebView2 Runtime». Ставим официальный
//! загрузчик Microsoft — только с его подписью.

use crate::win::{self, Key, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use std::time::Duration;

/// Код WebView2 Runtime в Edge Update.
const CLIENT: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
/// Загрузчик Evergreen Bootstrapper с сайта Microsoft (~2 МБ, остальное докачивает сам).
const BOOTSTRAPPER: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

/// Стоит ли WebView2 Runtime — для всех или для этого пользователя.
pub fn installed() -> bool {
    [
        (HKEY_LOCAL_MACHINE, format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{CLIENT}")),
        (HKEY_LOCAL_MACHINE, format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{CLIENT}")),
        (HKEY_CURRENT_USER, format!(r"Software\Microsoft\EdgeUpdate\Clients\{CLIENT}")),
    ]
    .iter()
    .any(|(root, path)| Key::open(*root, path).and_then(|k| k.string("pv")).is_some_and(|v| is_version(&v)))
}

fn is_version(v: &str) -> bool {
    let v = v.trim();
    !v.is_empty() && v != "0.0.0.0" && v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Скачать загрузчик, проверить подпись Microsoft и поставить. `quiet` — без окон загрузчика
/// (тихая установка); иначе он показывает свой прогресс. Ошибка — текст для человека.
pub fn install(quiet: bool) -> Result<(), String> {
    let file = std::env::temp_dir().join(format!("klick-webview2-{}.exe", std::process::id()));
    let curl = win::system32("curl.exe");
    let target = file.to_string_lossy().into_owned();
    let code = win::run(&curl, &["-sS", "-L", "--fail", "--max-time", "120", "-o", &target, BOOTSTRAPPER], Duration::from_secs(150))
        .map_err(|e| format!("не удалось запустить curl: {e}"))?;
    if code != 0 || !file.exists() {
        return Err("не скачался загрузчик WebView2 с сайта Microsoft".into());
    }
    // Подпись — действительная и именно Microsoft: чужой файл под этим адресом не запустим.
    let check = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath {}; if ($s.Status -eq 'Valid' -and $s.SignerCertificate.Subject -match 'O=Microsoft Corporation') {{ exit 0 }} else {{ exit 1 }}",
        win::ps_quote(&target)
    );
    let signed = win::powershell(&check, Duration::from_secs(60)).is_ok_and(|c| c == 0);
    if !signed {
        let _ = std::fs::remove_file(&file);
        return Err("загрузчик WebView2 без подписи Microsoft — не запускаю".into());
    }
    let args: &[&str] = if quiet { &["/silent", "/install"] } else { &["/install"] };
    let result = win::run(&file, args, Duration::from_secs(900));
    let _ = std::fs::remove_file(&file);
    match result {
        Ok(_) if installed() => Ok(()),
        Ok(c) => Err(format!("WebView2 не установился (код {c})")),
        Err(e) => Err(format!("WebView2 не установился: {e}")),
    }
}

/// Спросить человека окном Windows — у установщика ещё нет своего окна.
pub fn ask_install() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONINFORMATION, MB_YESNO};
    let title = wide("Установка kl!ck");
    let text = wide(
        "Для окон kl!ck нужен компонент Microsoft Edge WebView2 Runtime — на этом компьютере его нет.\n\n\
         Скачать и установить его сейчас с сайта Microsoft? Это займёт пару минут.",
    );
    unsafe { MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), MB_YESNO | MB_ICONINFORMATION) == IDYES }
}

pub fn tell(text: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let title = wide("Установка kl!ck");
    let text = wide(text);
    unsafe {
        MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn versions() {
        assert!(super::is_version("130.0.2849.80"));
        assert!(!super::is_version("0.0.0.0"));
        assert!(!super::is_version(""));
    }
}
