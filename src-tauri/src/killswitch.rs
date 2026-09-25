//! Kill Switch: пока VPN выключен (или оборвался), отмеченные программы
//! остаются без интернета — правило брандмауэра Windows на их exe, — а
//! отмеченные сайты не открываются ни в одной программе. Остальное работает
//! как обычно. При включённом VPN правила сняты (core::ks_engaged): куда
//! идёт трафик тогда — дело «Маршрутизации».
//!
//! Чем Kill Switch может положить сеть сам — и что от этого страхует:
//! системная программа в списке (svchost.exe — это DNS-клиент Windows) —
//! такие не принимаем; сайт за общим CDN — его адреса у Cloudflare и
//! подобных общие с тысячами других сайтов, их не закрываем, а в окне
//! помечаем, что сайт так не защитить.
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
/// Сайты, часть адресов которых общая с чужими — не закрыты.
static SHARED: Lazy<Mutex<BTreeSet<String>>> = Lazy::new(|| Mutex::new(BTreeSet::new()));
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

/// Адреса больших CDN, общие для множества сайтов: закрыть такой — значит
/// закрыть и всех соседей. Cloudflare и Fastly (github, reddit и др.).
const SHARED_V4: [(u32, u8); 17] = [
    (0xADF5_3000, 20), // 173.245.48.0/20
    (0x6715_F400, 22), // 103.21.244.0/22
    (0x6716_C800, 22), // 103.22.200.0/22
    (0x671F_0400, 22), // 103.31.4.0/22
    (0x8D65_4000, 18), // 141.101.64.0/18
    (0x6CA2_C000, 18), // 108.162.192.0/18
    (0xBE5D_F000, 20), // 190.93.240.0/20
    (0xBC72_6000, 20), // 188.114.96.0/20
    (0xC5EA_F000, 22), // 197.234.240.0/22
    (0xC629_8000, 17), // 198.41.128.0/17
    (0xA29E_0000, 15), // 162.158.0.0/15
    (0x6810_0000, 13), // 104.16.0.0/13
    (0x6818_0000, 14), // 104.24.0.0/14
    (0xAC40_0000, 13), // 172.64.0.0/13
    (0x8300_4800, 22), // 131.0.72.0/22
    (0x9765_0000, 16), // 151.101.0.0/16 Fastly
    (0xC7E8_0000, 16), // 199.232.0.0/16 Fastly
];
/// Первые 32 бита IPv6-префиксов Cloudflare и Fastly.
const SHARED_V6: [(u32, u8); 8] = [
    (0x2606_4700, 32),
    (0x2400_CB00, 32),
    (0x2803_F800, 32),
    (0x2405_B500, 32),
    (0x2405_8100, 32),
    (0x2A06_98C0, 29),
    (0x2C0F_F248, 32),
    (0x2A04_4E40, 32), // Fastly
];

fn in_net(addr: u32, (net, bits): (u32, u8)) -> bool {
    let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
    addr & mask == net & mask
}

fn shared(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => SHARED_V4.iter().any(|n| in_net(u32::from(*v), *n)),
        IpAddr::V6(v) => {
            let s = v.segments();
            SHARED_V6.iter().any(|n| in_net(((s[0] as u32) << 16) | s[1] as u32, *n))
        }
    }
}

pub fn shared_sites() -> Vec<String> {
    SHARED.lock().unwrap().iter().cloned().collect()
}

/// Программа, без которой сеть не работает целиком, — ей интернет не
/// закрываем. Возвращает, почему нельзя.
pub fn protected_app(path_or_exe: &str) -> Option<String> {
    let low = path_or_exe.trim().to_lowercase().replace('/', "\\");
    let exe = low.rsplit('\\').next().unwrap_or(&low).to_string();
    let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()).to_lowercase();
    let system = low.starts_with(&(windir + "\\"))
        || ["svchost.exe", "lsass.exe", "services.exe", "wininit.exe", "csrss.exe", "smss.exe", "system", "dnscache", "spoolsv.exe"].contains(&exe.as_str());
    if system {
        return Some(format!("{exe} — часть Windows: через неё работают DNS и сеть всех программ. Закрыть ей интернет — значит отключить его у всего компьютера."));
    }
    match exe.as_str() {
        "klick.exe" | "mihomo.exe" => Some("Самому kl!ck и его ядру закрывать интернет нельзя — VPN перестанет подключаться.".into()),
        "msedgewebview2.exe" => Some("msedgewebview2.exe — общий движок окон множества программ (в том числе kl!ck). Закройте интернет самой программе, а не ему.".into()),
        _ => None,
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
    let mut shared_now = SHARED.lock().unwrap();
    shared_now.retain(|d| domains.contains(d));
    for (d, ips) in fresh {
        let (common, own): (Vec<IpAddr>, Vec<IpAddr>) = ips.into_iter().partition(shared);
        if !common.is_empty() {
            shared_now.insert(d.clone());
        }
        let set = known.entry(d).or_default();
        for ip in own {
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
    fn общие_адреса_cdn() {
        for ip in ["104.16.132.229", "172.67.1.1", "151.101.1.140", "2606:4700::6810:84e5"] {
            assert!(shared(&ip.parse().unwrap()), "{ip}");
        }
        for ip in ["140.82.121.4", "87.250.250.242", "2a00:1450:4010::65"] {
            assert!(!shared(&ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn системное_не_закрываем() {
        assert!(protected_app(r"C:\Windows\System32\svchost.exe").is_some());
        assert!(protected_app("svchost.exe").is_some());
        assert!(protected_app(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application\1\msedgewebview2.exe").is_some());
        assert!(protected_app(r"C:\Games\Roblox\RobloxPlayerBeta.exe").is_none());
        assert!(protected_app("qbittorrent.exe").is_none());
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
