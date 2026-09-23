//! Kill Switch: пока VPN выключен (или оборвался), отмеченные программы
//! остаются без интернета — правило брандмауэра Windows на их exe, — а
//! отмеченные сайты не открываются ни в одной программе. Остальное работает
//! как обычно.
//!
//! Брандмауэр Windows не умеет правила по имени домена, только по адресам.
//! Поэтому сайт — это адреса, в которые сейчас резолвятся он и www.-вариант:
//! их закрываем для всех программ и, пока VPN выключен, раз в несколько
//! минут резолвим заново и добавляем новые (у CDN адреса ротируются).
//! Поддомены на других серверах так не закрыть, а сайты за общим CDN делят
//! адреса с чужими — окно об этом предупреждает.
//!
//! Правила живут группой: снять — одной командой, даже если набор программ
//! поменялся. Что выяснено в SingBoxGUI: Remove-NetFirewallRule на
//! отсутствующее правило валит powershell.exe кодом 1 даже с
//! SilentlyContinue, поэтому снимаем через Get-… | Remove-… — пустой
//! конвейер ничего не вызывает.

use once_cell::sync::Lazy;
use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::state::{KsApp, KsSite};
use crate::sys::{powershell, ps_quote};

const GROUP: &str = "klick-killswitch";

/// Что сейчас стоит в брандмауэре — чтобы не дёргать PowerShell зря.
#[derive(PartialEq, Clone, Default)]
struct Applied {
    paths: Vec<String>,
    ips: Vec<String>,
}
static APPLIED: Lazy<Mutex<Option<Applied>>> = Lazy::new(|| Mutex::new(None));
/// Адреса сайтов, накопленные с момента, как защита включилась.
static SITE_IPS: Lazy<Mutex<HashMap<String, BTreeSet<IpAddr>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
/// Когда пора резолвить сайты заново (только пока защита включена).
static NEXT_RESOLVE: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
/// Больше адресов на сайт не копим: столько не бывает даже у CDN, а правило
/// брандмауэра с тысячами адресов применяется медленно.
const MAX_IPS_PER_SITE: usize = 64;
/// Адресов в одном правиле.
const IPS_PER_RULE: usize = 200;
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

/// Домен из того, что ввёл человек: без схемы, пути, порта и «*.».
/// None — если это не имя хоста (зону вроде .ru адресами не описать).
pub fn normalize_domain(input: &str) -> Option<String> {
    let mut d = input.trim().to_lowercase();
    if let Some(i) = d.find("://") {
        d = d[i + 3..].to_string();
    }
    let d = d.split(['/', '?', '#']).next().unwrap_or("");
    let d = d.rsplit('@').next().unwrap_or("");
    let d = d.split(':').next().unwrap_or("");
    let d = d.trim_start_matches("*.").trim_start_matches('.').trim_end_matches('.');
    let ok = d.len() <= 253
        && d.contains('.')
        && d.parse::<IpAddr>().is_err()
        && d.split('.').all(|l| {
            !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    ok.then(|| d.to_string())
}

/// Адрес, который имеет смысл закрывать: не локальный, не служебный и не
/// из диапазона fake-ip mihomo (сразу после отключения в кэше DNS Windows
/// ещё могут лежать его ответы).
fn blockable(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
                || v.is_multicast()
                || o[0] == 0
                || (o[0] == 198 && (o[1] & 0xfe) == 18)
                || (o[0] == 100 && (o[1] & 0xc0) == 64))
        }
        IpAddr::V6(v) => {
            let s0 = v.segments()[0];
            !(v.is_loopback() || v.is_unspecified() || v.is_multicast() || (s0 & 0xfe00) == 0xfc00 || (s0 & 0xffc0) == 0xfe80)
        }
    }
}

fn resolve(host: &str) -> Vec<IpAddr> {
    (host, 443).to_socket_addrs().map(|it| it.map(|a| a.ip()).filter(blockable).collect()).unwrap_or_default()
}

/// Адреса включённых сайтов: свежий резолв плюс накопленное раньше.
fn site_ips(sites: &[KsSite]) -> Vec<String> {
    let domains: Vec<String> = sites.iter().filter(|s| s.on).filter_map(|s| normalize_domain(&s.pattern)).collect();
    let fresh: Vec<(String, Vec<IpAddr>)> = std::thread::scope(|sc| {
        let jobs: Vec<_> = domains
            .iter()
            .map(|d| {
                sc.spawn(move || {
                    let mut ips = resolve(d);
                    if !d.starts_with("www.") {
                        ips.extend(resolve(&format!("www.{d}")));
                    }
                    (d.clone(), ips)
                })
            })
            .collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    });
    let mut known = SITE_IPS.lock().unwrap();
    known.retain(|d, _| domains.contains(d));
    for (d, ips) in fresh {
        let set = known.entry(d).or_default();
        for ip in ips {
            if set.len() >= MAX_IPS_PER_SITE {
                break;
            }
            set.insert(ip);
        }
    }
    let all: BTreeSet<&IpAddr> = known.values().flatten().collect();
    all.into_iter().map(|ip| ip.to_string()).collect()
}

/// Пора ли резолвить сайты заново — спрашивает фоновый присмотр. Ответ
/// «да» сразу отодвигает следующий срок, чтобы повтор не запустился
/// дважды, пока идёт этот.
pub fn resolve_due() -> bool {
    let mut next = NEXT_RESOLVE.lock().unwrap();
    match *next {
        Some(t) if Instant::now() >= t => {
            *next = Some(Instant::now() + Duration::from_secs(300));
            true
        }
        _ => false,
    }
}

/// Привести правила к нужному виду: `engage` — блокировать ли сейчас.
/// Возвращает предупреждение, если защита не работает.
pub fn apply(engage: bool, apps: &[KsApp], sites: &[KsSite]) -> Option<String> {
    let has_sites = sites.iter().any(|s| s.on);
    let want = if engage {
        Applied {
            paths: apps.iter().filter(|a| a.on && !a.path.trim().is_empty()).map(|a| a.path.clone()).collect(),
            ips: if has_sites { site_ips(sites) } else { vec![] },
        }
    } else {
        SITE_IPS.lock().unwrap().clear();
        Applied::default()
    };
    {
        // Первый повтор — скоро: сразу после отключения DNS ещё может
        // отвечать из кэша туннеля. Дальше — раз в пять минут.
        let mut next = NEXT_RESOLVE.lock().unwrap();
        let wait = if next.is_none() { Duration::from_secs(30) } else { Duration::from_secs(300) };
        *next = (engage && has_sites).then(|| Instant::now() + wait);
    }
    let mut applied = APPLIED.lock().unwrap();
    if applied.as_ref() == Some(&want) {
        return issue();
    }
    let mut body = remove_script();
    for path in &want.paths {
        let exe = path.rsplit(['\\', '/']).next().unwrap_or(path);
        body.push_str(&format!(
            "New-NetFirewallRule -DisplayName {} -Group {} -Direction Outbound -Action Block -Program {} -Profile Any -ErrorAction Stop | Out-Null; ",
            ps_quote(&format!("kl!ck Kill Switch: {exe}")),
            ps_quote(GROUP),
            ps_quote(path)
        ));
    }
    for (i, chunk) in want.ips.chunks(IPS_PER_RULE).enumerate() {
        let list: Vec<String> = chunk.iter().map(|ip| ps_quote(ip)).collect();
        body.push_str(&format!(
            "New-NetFirewallRule -DisplayName {} -Group {} -Direction Outbound -Action Block -RemoteAddress @({}) -Profile Any -ErrorAction Stop | Out-Null; ",
            ps_quote(&format!("kl!ck Kill Switch: сайты {}", i + 1)),
            ps_quote(GROUP),
            list.join(",")
        ));
    }
    let blocking = !want.paths.is_empty() || !want.ips.is_empty();
    let result = match powershell(&wrap(&body)) {
        Err(e) => {
            *applied = None;
            Some(format!("Правило брандмауэра не записалось: {e}"))
        }
        Ok(_) => {
            *applied = Some(want);
            if blocking {
                firewall_off_reason()
            } else {
                None
            }
        }
    };
    *ISSUE.lock().unwrap() = result.clone();
    result
}

/// Сбросить запомненное и применить заново — кнопка «Повторить».
pub fn retry(engage: bool, apps: &[KsApp], sites: &[KsSite]) -> Option<String> {
    *APPLIED.lock().unwrap() = None;
    apply(engage, apps, sites)
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
    *APPLIED.lock().unwrap() = Some(Applied::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn домен_из_ввода() {
        assert_eq!(normalize_domain("https://Mail.Google.com/mail/u/0").as_deref(), Some("mail.google.com"));
        assert_eq!(normalize_domain("*.sberbank.ru").as_deref(), Some("sberbank.ru"));
        assert_eq!(normalize_domain(".github.com.").as_deref(), Some("github.com"));
        assert_eq!(normalize_domain("user@host.example:8443").as_deref(), Some("host.example"));
        assert_eq!(normalize_domain(".ru"), None);
        assert_eq!(normalize_domain("localhost"), None);
        assert_eq!(normalize_domain("1.2.3.4"), None);
        assert_eq!(normalize_domain("bad_domain.com"), None);
        assert_eq!(normalize_domain("-x.com"), None);
    }

    #[test]
    fn что_закрывать_нельзя() {
        for ip in ["192.168.1.1", "10.0.0.1", "127.0.0.1", "198.18.0.5", "198.19.3.3", "100.64.1.1", "0.0.0.0", "::1", "fe80::1", "fd00::1"] {
            assert!(!blockable(&ip.parse().unwrap()), "{ip}");
        }
        for ip in ["140.82.121.4", "198.20.0.1", "2a00:1450:4010::65"] {
            assert!(blockable(&ip.parse().unwrap()), "{ip}");
        }
    }
}
