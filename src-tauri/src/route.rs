//! Объяснение маршрутизации окну: итог каждой строки списка, путь адреса по
//! лесенке правил и что изменится при переключении — до того, как
//! человек нажмёт.
//!
//! Порядок ступеней тот же, что строит `config::rules` для mihomo; тесты
//! ниже сверяют их, чтобы подсказки не расходились с тем, что делает ядро.

use once_cell::sync::Lazy;
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::config::RU_ZONES;
use crate::state::{Action, DefaultRoute, Mode, Settings};

pub const NAME_PROXY: &str = "VPN для всего";
pub const NAME_DIRECT: &str = "VPN для выбранного";

pub fn position_name(r: DefaultRoute) -> &'static str {
    match r {
        DefaultRoute::Proxy => NAME_PROXY,
        DefaultRoute::Direct => NAME_DIRECT,
    }
}

/// Имя exe из полного пути (или то, что записано, если пути нет).
pub fn exe_of(path: &str, exe: &str) -> String {
    let from_path = path.trim().rsplit(['\\', '/']).next().unwrap_or("").trim();
    if from_path.is_empty() { exe.trim().to_string() } else { from_path.to_string() }
}

// ─────────── Набор «Заблокированные в РФ» ───────────

/// Скачанный mihomo набор: домены (DOMAIN-SUFFIX) и когда обновлён.
#[derive(Default, Clone)]
pub struct Blocked {
    pub domains: Vec<String>,
    pub updated: Option<u64>,
}

static BLOCKED: Lazy<Mutex<(Option<SystemTime>, Blocked)>> = Lazy::new(|| Mutex::new((None, Blocked::default())));

/// Читает файл набора из папки ядра; перечитывает, только если он менялся.
pub fn blocked(core_home: &Path) -> Blocked {
    let file = core_home.join(crate::config::BLOCKED_PATH);
    let mtime = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
    let mut cache = BLOCKED.lock().unwrap();
    if cache.0 != mtime || mtime.is_none() {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let domains = text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("DOMAIN-SUFFIX,"))
            .map(|d| d.split(',').next().unwrap_or("").trim().trim_start_matches('.').to_lowercase())
            .filter(|d| !d.is_empty())
            .collect();
        let updated = mtime.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
        *cache = (mtime, Blocked { domains, updated });
    }
    cache.1.clone()
}

fn suffix_of(host: &str, rule: &str) -> bool {
    let rule = rule.trim_start_matches('.');
    host == rule || host.ends_with(&format!(".{rule}"))
}

impl Blocked {
    pub fn contains(&self, host: &str) -> bool {
        self.domains.iter().any(|d| suffix_of(host, d))
    }
}

fn is_ru(host: &str) -> bool {
    let zone = host.rsplit('.').next().unwrap_or("");
    RU_ZONES.contains(&zone) || zone == "рф" || zone == "дети"
}

// ─────────── Итог строки ───────────

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// app | site
    pub kind: &'static str,
    /// exe программы или шаблон сайта — чем окно находит правило.
    pub key: String,
    /// vpn | direct | block | ks
    pub state: &'static str,
    pub result: String,
    /// vpn | direct | block | muted
    pub tone: &'static str,
    pub hint: Option<String>,
    /// Подсказка — предупреждение (правило не действует или ничего не меняет).
    pub warn: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub apps: Vec<Row>,
    pub sites: Vec<Row>,
    pub blocked_count: usize,
    pub blocked_updated: Option<u64>,
}

fn proxy_mode(s: &Settings) -> bool {
    s.mode != Mode::Tun
}

fn state_of(a: Action) -> &'static str {
    match a {
        Action::Proxy => "vpn",
        Action::Direct => "direct",
        Action::Block => "block",
    }
}

fn row(kind: &'static str, key: &str, state: &'static str, s: &Settings, b: &Blocked) -> Row {
    let mut r = Row { kind, key: key.to_string(), state, result: String::new(), tone: "vpn", hint: None, warn: false };
    let ex = s.default_route == DefaultRoute::Proxy;
    if state == "ks" && !s.kill_switch {
        r.result = "как остальные · Kill Switch выключен".into();
        r.tone = "muted";
        r.warn = true;
        r.hint = Some("Список Kill Switch сейчас не действует: программа идёт как всё остальное, а без VPN — напрямую. Включите Kill Switch в «Настройках».".into());
        return r;
    }
    if state == "ks" {
        r.result = "через VPN всегда · без VPN — без сети".into();
        r.hint = Some(if proxy_mode(s) {
            r.warn = true;
            "Системный прокси: если программа не умеет ходить через прокси, она останется без сети.".into()
        } else {
            "Действует в любом положении и при выключенной маршрутизации.".into()
        });
        return r;
    }
    if !s.routing {
        r.result = "через VPN · маршрутизация выключена".into();
        r.tone = "muted";
        r.hint = Some(if state == "block" { "Блок сейчас не действует — включится вместе с маршрутизацией." } else { "Правило сохранено и заработает, когда маршрутизация включена." }.into());
        return r;
    }
    let host = key.trim_start_matches('.').to_lowercase();
    match state {
        "block" => {
            r.result = "заблокировано".into();
            r.tone = "block";
        }
        "vpn" => {
            r.result = "через VPN".into();
            if kind == "site" && ex && s.sets.ru && is_ru(&host) {
                r.hint = Some("Перебивает набор «Россия напрямую».".into());
            } else if ex {
                r.hint = Some("Ничего не меняет: в этом положении всё и так идёт через VPN.".into());
                r.warn = true;
            } else if kind == "site" && s.sets.blocked && b.contains(&host) {
                r.hint = Some("Уже есть в наборе «Заблокированные в РФ».".into());
            }
        }
        _ => {
            r.result = "напрямую".into();
            r.tone = "direct";
            if !ex && kind == "site" && s.sets.blocked && b.contains(&host) {
                r.hint = Some("Перебивает набор «Заблокированные в РФ».".into());
            } else if !ex {
                r.hint = Some("Ничего не меняет: в этом положении всё и так идёт напрямую.".into());
                r.warn = true;
            } else if kind == "site" && s.sets.ru && is_ru(&host) {
                r.hint = Some("Набор «Россия напрямую» и так ведёт его напрямую.".into());
            }
        }
    }
    if kind == "app" && proxy_mode(s) {
        r.hint = Some("Системный прокси: сработает, только если программа ходит через прокси.".into());
        r.warn = true;
    }
    r
}

pub fn view(s: &Settings, b: &Blocked) -> View {
    let ks: Vec<Row> = s.ks_apps.iter().filter(|a| a.on).map(|a| row("app", &a.exe, "ks", s, b)).collect();
    let list = s.list();
    let apps = ks.into_iter().chain(list.apps.iter().map(|a| row("app", &a.exe, state_of(a.action), s, b))).collect();
    let sites = list.sites.iter().map(|x| row("site", &x.pattern, state_of(x.action), s, b)).collect();
    View { apps, sites, blocked_count: b.domains.len(), blocked_updated: b.updated }
}

// ─────────── «Проверить сайт» ───────────

#[derive(Serialize, Clone, Debug)]
pub struct Step {
    pub title: String,
    pub note: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub host: String,
    pub steps: Vec<Step>,
    /// Какая ступень сработала (индекс в steps).
    pub hit: usize,
    pub verdict: String,
    /// vpn | direct | block
    pub tone: &'static str,
}

fn host_of(input: &str) -> String {
    let t = input.trim().to_lowercase();
    let t = t.split("://").last().unwrap_or("");
    let t = t.split(['/', '?', '#']).next().unwrap_or("");
    let t = t.rsplit('@').next().unwrap_or("");
    let t = if t.matches(':').count() == 1 { t.split(':').next().unwrap_or("") } else { t };
    t.trim_start_matches("www.").trim_matches('.').to_string()
}

fn lan(host: &str) -> bool {
    if ["local", "lan", "home.arpa"].iter().any(|z| suffix_of(host, z)) {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v)) => v.is_private() || v.is_loopback() || v.is_link_local(),
        Ok(std::net::IpAddr::V6(v)) => v.is_loopback() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80,
        Err(_) => false,
    }
}

pub fn check(s: &Settings, b: &Blocked, input: &str) -> Option<Check> {
    let host = host_of(input);
    if host.is_empty() {
        return None;
    }
    let step = |t: &str, n: &str| Step { title: t.into(), note: n.into() };
    let mut steps = vec![
        step("Локальная сеть — всегда напрямую", "Роутер, принтер, NAS."),
        step("Программы «Только через VPN»", "Для сайта не срабатывает: решает программа, из которой его открыли."),
    ];
    let mut done: Option<(usize, String, &'static str)> = lan(&host).then(|| (0, "напрямую · локальная сеть".into(), "direct"));
    if !s.routing {
        steps.push(step("Маршрутизация выключена → через VPN", "Списки и наборы ниже не действуют."));
        if done.is_none() {
            done = Some((2, "через VPN · маршрутизация выключена".into(), "vpn"));
        }
    } else {
        let list = s.list();
        steps.push(step("Правила программ", "Главнее сайтов: откроете в программе с правилом — решит оно."));
        let rule = list.sites.iter().find(|r| suffix_of(&host, &r.pattern.to_lowercase()));
        steps.push(step("Правила сайтов", &rule.map(|r| format!("Найдено: {}", r.pattern)).unwrap_or_else(|| "Совпадений нет.".into())));
        if done.is_none() {
            if let Some(r) = rule {
                let (t, tone) = match r.action {
                    Action::Proxy => ("через VPN", "vpn"),
                    Action::Direct => ("напрямую", "direct"),
                    Action::Block => ("заблокирован", "block"),
                };
                done = Some((3, format!("{t} · правило сайта"), tone));
            }
        }
        match s.default_route {
            DefaultRoute::Proxy => {
                steps.push(step("Набор «Россия напрямую»", if s.sets.ru { "Включён: .ru, .su, .рф и российские адреса." } else { "Выключен." }));
                if done.is_none() && s.sets.ru && is_ru(&host) {
                    done = Some((4, "напрямую · набор «Россия»".into(), "direct"));
                }
                steps.push(step("Всё остальное → через VPN", &format!("Положение «{NAME_PROXY}».")));
                if done.is_none() {
                    let note = if s.sets.ru { " Если адрес сайта российский, его отправит напрямую набор «Россия» — это видно только при подключении." } else { "" };
                    steps.last_mut().unwrap().note.push_str(note);
                    done = Some((5, "через VPN · всё остальное".into(), "vpn"));
                }
            }
            DefaultRoute::Direct => {
                let n = if !s.sets.blocked { "Выключен.".to_string() } else if b.domains.is_empty() { "Включён, но ещё не скачан — скачается при подключении.".to_string() } else { format!("Включён: {} доменов, обновляется сам.", b.domains.len()) };
                steps.push(step("Набор «Заблокированные в РФ»", &n));
                if done.is_none() && s.sets.blocked && b.contains(&host) {
                    done = Some((4, "через VPN · набор «Заблокированные»".into(), "vpn"));
                }
                steps.push(step("Всё остальное → напрямую", &format!("Положение «{NAME_DIRECT}».")));
                if done.is_none() {
                    done = Some((5, "напрямую · всё остальное".into(), "direct"));
                }
            }
        }
    }
    let (hit, why, tone) = done.unwrap();
    Some(Check { verdict: format!("{host} → {why}"), host, steps, hit, tone })
}

// ─────────── Что изменится при переключении ───────────

#[derive(Serialize, Clone, Debug)]
pub struct Line {
    /// vpn | direct | muted | warn
    pub tone: &'static str,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Preview {
    pub title: String,
    pub cta: String,
    pub lines: Vec<Line>,
}

/// `routing` и `route` — какими они станут.
pub fn preview(s: &Settings, routing: bool, route: DefaultRoute) -> Preview {
    let line = |tone, text: String| Line { tone, text };
    let ks = s.ks_apps.iter().filter(|a| a.on).count();
    let keep = line("muted", format!("Не меняется: «Только через VPN» ({ks}) и локальная сеть напрямую."));
    let list = match route {
        DefaultRoute::Proxy => &s.lists.proxy,
        DefaultRoute::Direct => &s.lists.direct,
    };
    let count = |a: Action| list.apps.iter().filter(|x| x.action == a).count() + list.sites.iter().filter(|x| x.action == a).count();
    let total = list.apps.len() + list.sites.len();
    let block = count(Action::Block);
    let block_txt = if block > 0 { format!("; блок: {block}") } else { String::new() };
    if !routing {
        return Preview {
            title: "Выключить маршрутизацию?".into(),
            cta: "Выключить".into(),
            lines: vec![
                line("vpn", "Весь трафик пойдёт через VPN, включая российские сайты.".into()),
                line("muted", format!("Правила ({total}) сохранятся, но не будут действовать{}.", if block > 0 { format!(", блок тоже ({block})") } else { String::new() })),
                keep,
            ],
        };
    }
    let mut lines = match route {
        DefaultRoute::Proxy => vec![
            line("vpn", "Через VPN — всё, кроме:".into()),
            line("muted", format!("{}{} правил «напрямую»{block_txt}.", if s.sets.ru { "набор «Россия напрямую»; " } else { "" }, count(Action::Direct))),
        ],
        DefaultRoute::Direct => vec![
            line("direct", "Напрямую — всё, кроме:".into()),
            line("vpn", format!("{}{} правил «через VPN»{block_txt}.", if s.sets.blocked { "набор «Заблокированные в РФ»; " } else { "" }, count(Action::Proxy))),
            line("warn", "Сайты, которых нет ни в наборе, ни в списке, откроются без VPN. Не открылся — добавьте его сюда.".into()),
        ],
    };
    lines.push(keep);
    let name = position_name(route);
    if s.routing {
        Preview { title: format!("Переключить на «{name}»?"), cta: "Переключить".into(), lines }
    } else {
        Preview { title: format!("Включить маршрутизацию: «{name}»?"), cta: "Включить".into(), lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppRule, KsApp, RuleList, SiteRule};

    fn blocked() -> Blocked {
        Blocked { domains: vec!["youtube.com".into(), "discord.com".into()], updated: None }
    }
    fn st(route: DefaultRoute, list: RuleList) -> Settings {
        let mut s = Settings { routing: true, default_route: route, kill_switch: true, ..Default::default() };
        match route {
            DefaultRoute::Proxy => s.lists.proxy = list,
            DefaultRoute::Direct => s.lists.direct = list,
        }
        s
    }

    #[test]
    fn итог_строк_и_подсказки() {
        let s = st(DefaultRoute::Proxy, RuleList {
            apps: vec![AppRule { name: "D".into(), exe: "Discord.exe".into(), action: Action::Proxy, path: String::new() }],
            sites: vec![SiteRule { pattern: "sberbank.ru".into(), action: Action::Proxy }, SiteRule { pattern: "gosuslugi.ru".into(), action: Action::Direct }],
        });
        let v = view(&s, &blocked());
        assert!(v.apps[0].warn && v.apps[0].hint.as_deref().unwrap().contains("Ничего не меняет"));
        assert!(v.sites[0].hint.as_deref().unwrap().contains("Перебивает набор «Россия"));
        assert!(v.sites[1].hint.as_deref().unwrap().contains("и так ведёт"));
        let mut off = s.clone();
        off.routing = false;
        assert_eq!(view(&off, &blocked()).sites[1].tone, "muted");
    }

    #[test]
    fn только_через_vpn_первым() {
        let mut s = st(DefaultRoute::Direct, RuleList::default());
        s.ks_apps = vec![KsApp { name: "qB".into(), exe: "qbittorrent.exe".into(), path: "C:\\qb\\qbittorrent.exe".into(), on: true }];
        let v = view(&s, &blocked());
        assert_eq!(v.apps[0].state, "ks");
        s.mode = Mode::Sysproxy;
        assert!(view(&s, &blocked()).apps[0].warn);
    }

    #[test]
    fn проверка_сайта_совпадает_с_правилами_ядра() {
        let s = st(DefaultRoute::Direct, RuleList { apps: vec![], sites: vec![SiteRule { pattern: "chatgpt.com".into(), action: Action::Proxy }] });
        let b = blocked();
        let c = |x: &str| check(&s, &b, x).unwrap();
        assert_eq!((c("https://www.youtube.com/watch").hit, c("youtube.com").tone), (4, "vpn"));
        assert_eq!(c("chat.chatgpt.com").hit, 3);
        assert_eq!(c("wikipedia.org").tone, "direct");
        assert_eq!(c("nas.lan").hit, 0);
        assert_eq!(c("192.168.1.1").hit, 0);
        // Те же ступени, что в config::rules: набор стоит, правило сайта выше него.
        let r = crate::config::rules(&s, "");
        let pos = |p: &str| r.iter().position(|x| x.starts_with(p)).unwrap();
        assert!(pos("DOMAIN-SUFFIX,chatgpt.com") < pos("RULE-SET") && pos("RULE-SET") < pos("MATCH"));
        let mut off = s.clone();
        off.routing = false;
        assert_eq!(check(&off, &b, "wikipedia.org").unwrap().verdict, "wikipedia.org → через VPN · маршрутизация выключена");
    }

    #[test]
    fn предпросмотр_переключения() {
        let mut s = st(DefaultRoute::Proxy, RuleList::default());
        s.routing = false;
        let p = preview(&s, true, DefaultRoute::Direct);
        assert!(p.title.starts_with("Включить") && p.lines.iter().any(|l| l.tone == "warn"));
        s.routing = true;
        assert!(preview(&s, false, DefaultRoute::Proxy).title.starts_with("Выключить"));
        assert!(preview(&s, true, DefaultRoute::Direct).title.starts_with("Переключить"));
    }

    #[test]
    fn набор_из_файла() {
        let dir = std::env::temp_dir().join("klick-route-test");
        std::fs::create_dir_all(dir.join("rulesets")).unwrap();
        std::fs::write(dir.join(crate::config::BLOCKED_PATH), "DOMAIN-SUFFIX,.ua\nDOMAIN-SUFFIX,youtube.com\n# comment\n").unwrap();
        let b = super::blocked(&dir);
        assert!(b.contains("m.youtube.com") && b.contains("kyiv.ua") && !b.contains("ya.ru"));
    }
}
