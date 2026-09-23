//! Kill Switch по программам: пока VPN выключен (или оборвался), отмеченные
//! программы остаются без интернета — правило брандмауэра Windows на их exe.
//! Остальные работают как обычно.
//!
//! Правила живут группой: снять — одной командой, даже если набор программ
//! поменялся. Что выяснено в SingBoxGUI: Remove-NetFirewallRule на
//! отсутствующее правило валит powershell.exe кодом 1 даже с
//! SilentlyContinue, поэтому снимаем через Get-… | Remove-… — пустой
//! конвейер ничего не вызывает.

use once_cell::sync::Lazy;
use std::sync::Mutex;

use crate::state::KsApp;
use crate::sys::{powershell, ps_quote};

const GROUP: &str = "klick-killswitch";

/// Что сейчас стоит в брандмауэре — чтобы не дёргать PowerShell зря.
static APPLIED: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));
/// Последняя проблема: окно показывает её тостом, точкой на «Настройках» и
/// на строке Kill Switch, пока она не уйдёт.
static ISSUE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

pub fn issue() -> Option<String> {
    ISSUE.lock().unwrap().clone()
}

/// Снять все правила группы. Через переменную, а не конвейером: если
/// правил нет, Get-NetFirewallRule даже с SilentlyContinue помечает команду
/// неуспешной, и powershell.exe выходил с кодом 1 без единого слова — ровно
/// эта ошибка всплывала при первом включении.
fn remove_script() -> String {
    format!(
        "$r = Get-NetFirewallRule -Group {g} -ErrorAction SilentlyContinue; if ($r) {{ $r | Remove-NetFirewallRule -ErrorAction Stop }}; ",
        g = ps_quote(GROUP)
    )
}

/// Скрипт целиком: настоящие ошибки — с текстом и кодом 1, остальное — 0.
fn wrap(body: &str) -> String {
    format!("try {{ {body} }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}; exit 0")
}

/// Привести правила к нужному виду: `engage` — блокировать ли сейчас.
/// Возвращает предупреждение, если защита не работает.
pub fn apply(engage: bool, apps: &[KsApp]) -> Option<String> {
    let want: Vec<String> = if engage {
        apps.iter().filter(|a| a.on && !a.path.trim().is_empty()).map(|a| a.path.clone()).collect()
    } else {
        vec![]
    };
    let mut applied = APPLIED.lock().unwrap();
    if applied.as_ref() == Some(&want) {
        return issue();
    }
    let mut body = remove_script();
    for path in &want {
        let exe = path.rsplit(['\\', '/']).next().unwrap_or(path);
        body.push_str(&format!(
            "New-NetFirewallRule -DisplayName {} -Group {} -Direction Outbound -Action Block -Program {} -Profile Any -ErrorAction Stop | Out-Null; ",
            ps_quote(&format!("kl!ck Kill Switch: {exe}")),
            ps_quote(GROUP),
            ps_quote(path)
        ));
    }
    let result = match powershell(&wrap(&body)) {
        Err(e) => {
            *applied = None;
            Some(format!("Правило брандмауэра не записалось: {e}"))
        }
        Ok(_) => {
            *applied = Some(want.clone());
            if want.is_empty() { None } else { firewall_off_reason() }
        }
    };
    *ISSUE.lock().unwrap() = result.clone();
    result
}

/// Сбросить запомненное и применить заново — кнопка «Повторить».
pub fn retry(engage: bool, apps: &[KsApp]) -> Option<String> {
    *APPLIED.lock().unwrap() = None;
    apply(engage, apps)
}

/// Правило бессильно, если брандмауэр остановлен или выключен в профиле.
fn firewall_off_reason() -> Option<String> {
    let svc = powershell("(Get-Service -Name mpssvc).Status").unwrap_or_default();
    if svc.trim() != "Running" {
        return Some("Служба брандмауэра Windows остановлена — Kill Switch не действует.".into());
    }
    let off = powershell("(Get-NetFirewallProfile | Where-Object { -not $_.Enabled } | Select-Object -ExpandProperty Name) -join ', '").unwrap_or_default();
    let off = off.trim();
    (!off.is_empty()).then(|| format!("Брандмауэр выключен для профилей: {off} — там Kill Switch не действует."))
}

/// Снять всё — при выходе и из деинсталлятора.
pub fn cleanup() {
    let _ = powershell(&wrap(&remove_script()));
    *APPLIED.lock().unwrap() = Some(vec![]);
}
