//! Kill Switch на уровне ядра macOS: встроенный брандмауэр pf.
//!
//! Пока в Kill Switch есть программы, в интернет мимо адаптера ядра выходит только root — ядро kl!ck,
//! служба и системные службы macOS. Программы пользователя выходят только через адаптер (адрес
//! 198.18.0.1 или fdfe:dcba:9876::1), а там их разбирают правила ядра: защищённые — только через VPN,
//! при выключенном VPN — никуда. Локальная сеть открыта.
//!
//! Правила живут в ядре macOS, а не в kl!ck: если упадёт ядро, служба или VPN, адаптер исчезнет,
//! трафик программ пойдёт на физический интерфейс — и там его остановит pf. Ни одного прямого соединения
//! в промежутке, пока служба перезапускает ядро или сама перезапускается launchd. Правила снимаются
//! только когда Kill Switch выключают, при удалении kl!ck и после перезагрузки (до запуска службы, которая
//! ставит их снова одной из первых операций).
//!
//! pf различает, чей сокет (`user`), но не какая программа — поэтому такая защита общая: в сбое
//! без интернета остаются все программы пользователя, а не только защищённые. Это и есть «закрыто при сбое».

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const PFCTL: &str = "/sbin/pfctl";
/// Якорь внутри `com.apple/*`: стандартный `/etc/pf.conf` macOS уже ссылается на него, свой главный
/// набор правил менять не нужно. Номер — чтобы стоять раньше якорей Apple.
pub const ANCHOR: &str = "com.apple/090.klick";
/// Отметка в папке данных: токен включения pf (`pfctl -E`), по нему pf «отпускается» при снятии.
pub const MARKER: &str = "pf.json";

#[derive(Default, Serialize, Deserialize)]
struct Marker {
    token: Option<String>,
    /// Время загрузки macOS, когда получен токен: после перезагрузки старый токен ничего не значит.
    #[serde(default)]
    boot: Option<i64>,
}

/// Правила якоря. Порядок важен: срабатывает первое правило с `quick`.
pub fn rules() -> String {
    [
        "# kl!ck Kill Switch: в интернет мимо адаптера ядра выходит только root (ядро kl!ck, системные службы).",
        "pass quick on lo0 all",
        // В адаптер ядра — всё: дальше решают правила ядра.
        "pass out quick inet from 198.18.0.0/16 to any",
        "pass out quick inet6 from fdfe:dcba:9876::/64 to any",
        // Ядро kl!ck и служба работают от root: их соединения к серверам VPN и «напрямую» — можно.
        "pass out quick proto { tcp udp } from any to any user root",
        // Локальная сеть, DHCP, соседи IPv6.
        "pass out quick inet from any to { 10.0.0.0/8 172.16.0.0/12 192.168.0.0/16 169.254.0.0/16 100.64.0.0/10 224.0.0.0/4 255.255.255.255 }",
        "pass out quick inet6 from any to { fe80::/10 fc00::/7 ff00::/8 }",
        "pass out quick inet proto udp from any port 68 to any port 67",
        "pass out quick inet6 proto ipv6-icmp all",
        // Остальное — отказ сразу (RST или ICMP), чтобы программы не висели на тайм-ауте.
        "block return out quick all",
        "",
    ]
    .join("\n")
}

fn pfctl(args: &[&str], input: Option<&str>) -> Result<String> {
    if !cfg!(target_os = "macos") {
        bail!("pf есть только на macOS");
    }
    let out = crate::sys::run_within(PFCTL, args, input, std::time::Duration::from_secs(10)).context("pfctl")?;
    // pfctl пишет обычный вывод и в stderr («pf enabled», «Token : …», предупреждения ALTQ).
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        bail!("pfctl {}: {}", args.join(" "), text.trim());
    }
    Ok(text)
}

/// Отметка, если она с этой загрузки macOS; иначе пустая.
fn read_marker(data: &Path) -> Marker {
    let marker: Marker = std::fs::read(data.join(MARKER)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let now = crate::sys::boot_time();
    if marker.boot.is_some_and(|b| (b - now).abs() < 120) {
        marker
    } else {
        Marker::default()
    }
}

/// Главный набор правил спрашивает якоря `com.apple/*` — иначе наш якорь не работает.
fn main_ruleset_ok() -> bool {
    pfctl(&["-s", "rules"], None).is_ok_and(|main| main.contains("anchor \"com.apple/*\""))
}

/// `Token : 12345` из вывода `pfctl -E`.
fn parse_token(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.trim().strip_prefix("Token").map(|r| r.trim_start_matches([' ', ':']).trim().to_string())).filter(|t| !t.is_empty())
}

/// Поставить правила и включить pf. Повторный вызов безопасен.
pub fn apply(data: &Path) -> Result<()> {
    // Главный набор правил должен ссылаться на якоря `com.apple/*` (так в стандартном /etc/pf.conf).
    // Если его заменили или не загружали, загружаем стандартный — иначе наш якорь никто не спросит.
    if !main_ruleset_ok() {
        tracing::warn!("в правилах pf нет якоря com.apple/*, загружаю /etc/pf.conf");
        pfctl(&["-q", "-f", "/etc/pf.conf"], None).context("загрузка /etc/pf.conf")?;
    }
    pfctl(&["-q", "-a", ANCHOR, "-f", "-"], Some(&rules())).context("правила Kill Switch в pf")?;
    let info = pfctl(&["-s", "info"], None).unwrap_or_default();
    let mut marker = read_marker(data);
    // Своя ссылка на pf нужна, даже если его включил кто-то другой: иначе он выключит pf, отпуская свою.
    if !info.contains("Status: Enabled") || marker.token.is_none() {
        let out = pfctl(&["-E"], None).context("включение pf")?;
        if let Some(token) = parse_token(&out) {
            marker.token = Some(token);
            marker.boot = Some(crate::sys::boot_time());
            if let Ok(bytes) = serde_json::to_vec(&marker) {
                let _ = crate::storage::write_atomic(&data.join(MARKER), &bytes);
            }
        }
    }
    Ok(())
}

/// Правила на месте и работают: pf включён, главный набор спрашивает наш якорь. Другая программа могла
/// сбросить pf (`pfctl -F all`, `pfctl -d`) или загрузить свой набор правил без якорей Apple (`pfctl -f`).
pub fn active() -> bool {
    let info = pfctl(&["-s", "info"], None).unwrap_or_default();
    let ours = pfctl(&["-a", ANCHOR, "-s", "rules"], None).unwrap_or_default();
    info.contains("Status: Enabled") && ours.contains("block return out quick all") && main_ruleset_ok()
}

/// Снять правила Kill Switch и отпустить pf. `true` — было что снимать.
pub fn clear(data: &Path) -> bool {
    let had = pfctl(&["-a", ANCHOR, "-s", "rules"], None).is_ok_and(|r| !r.trim().is_empty());
    let _ = pfctl(&["-q", "-a", ANCHOR, "-F", "all"], None);
    let marker = read_marker(data);
    if let Some(token) = &marker.token {
        let _ = pfctl(&["-X", token], None);
    }
    let _ = std::fs::remove_file(data.join(MARKER));
    had || marker.token.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_keep_direct_traffic_for_root_and_lan_only() {
        let r = rules();
        let at = |needle: &str| r.find(needle).unwrap_or_else(|| panic!("нет {needle}"));
        assert!(at("from 198.18.0.0/16") < at("block return out quick all"));
        assert!(at("user root") < at("block return out quick all"));
        assert!(at("192.168.0.0/16") < at("block return out quick all"));
        assert!(r.trim_end().ends_with("block return out quick all"), "последнее правило — отказ");
        assert!(!r.contains("pass out quick all"), "нет правила, которое пропускает всё");
        assert!(r.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).all(|l| l.contains("quick")));
    }

    #[test]
    fn enable_token_is_parsed() {
        let out = "No ALTQ support in kernel\nALTQ related functions disabled\npf enabled\nToken : 13745689234512\n";
        assert_eq!(parse_token(out).as_deref(), Some("13745689234512"));
        assert_eq!(parse_token("pf enabled\n"), None);
    }
}
