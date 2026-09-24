//! Жизнь mihomo: запуск, остановка, лог, слежение, переподключение.
//!
//! Ядро — дочерний процесс в Job Object с KILL_ON_JOB_CLOSE: упадёт kl!ck —
//! Windows прибьёт и mihomo. Остановка сперва мягкая: Ctrl+Break строго в
//! группу процессов mihomo (его PID), не в группу 0 — туда входим и мы сами.

use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::api::Api;
use crate::config::{self, GROUP};
use crate::state::{now_secs, AppState, Mode, Profile};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub ok: bool,
    pub ms: Option<u32>,
    pub at: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// off | connecting | on | reconnecting | error
    pub state: &'static str,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub profile_id: Option<String>,
    /// Сервер, через который идёт трафик.
    pub server: Option<String>,
    pub mode: Mode,
    pub since: Option<u64>,
    pub health: Option<Health>,
}

impl Default for Status {
    fn default() -> Self {
        Status { state: "off", error: None, warning: None, profile_id: None, server: None, mode: Mode::Tun, since: None, health: None }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct LogEntry {
    pub time: String,
    /// INFO | WARN | ERR | DEBUG
    pub level: &'static str,
    pub text: String,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Traffic {
    pub up: u64,
    pub down: u64,
    pub up_total: u64,
    pub down_total: u64,
}

static CHILD: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));
static API: Lazy<Mutex<Option<Api>>> = Lazy::new(|| Mutex::new(None));
static STATUS: Lazy<Mutex<Status>> = Lazy::new(|| Mutex::new(Status::default()));
static STOP: AtomicBool = AtomicBool::new(false);
static GEN: AtomicU64 = AtomicU64::new(0);
static OP: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static LOG: Lazy<Mutex<VecDeque<LogEntry>>> = Lazy::new(|| Mutex::new(VecDeque::new()));
static PENDING: Lazy<Mutex<Vec<LogEntry>>> = Lazy::new(|| Mutex::new(vec![]));
static LOG_FILE: Lazy<Mutex<Option<std::fs::File>>> = Lazy::new(|| Mutex::new(None));
static TRAFFIC: Lazy<Mutex<Traffic>> = Lazy::new(|| Mutex::new(Traffic::default()));
/// Последние замеры задержки: подключение → сервер → мс (None — не ответил).
/// Меню трея показывает их сразу, без нового замера.
static PINGS: Lazy<Mutex<HashMap<String, HashMap<String, Option<u32>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn cached_pings(profile_id: &str) -> HashMap<String, Option<u32>> {
    PINGS.lock().unwrap().get(profile_id).cloned().unwrap_or_default()
}

fn remember_pings(profile_id: &str, m: &HashMap<String, Option<u32>>) {
    PINGS.lock().unwrap().entry(profile_id.to_string()).or_default().extend(m.iter().map(|(k, v)| (k.clone(), *v)));
}
const LOG_MAX: usize = 500;

pub fn status() -> Status {
    STATUS.lock().unwrap().clone()
}

pub fn traffic() -> Traffic {
    TRAFFIC.lock().unwrap().clone()
}

pub fn is_on() -> bool {
    STATUS.lock().unwrap().state == "on"
}

pub fn api() -> Option<Api> {
    API.lock().unwrap().clone()
}

fn set_status(app: &AppHandle, f: impl FnOnce(&mut Status)) {
    let s = {
        let mut st = STATUS.lock().unwrap();
        f(&mut st);
        st.clone()
    };
    let _ = app.emit("core-status", &s);
    crate::tray::refresh(app);
}

// ─────────── Лог ───────────

pub fn logs() -> Vec<LogEntry> {
    LOG.lock().unwrap().iter().cloned().collect()
}

pub fn clear_logs() {
    LOG.lock().unwrap().clear();
}

pub fn restore_logs(list: Vec<LogEntry>) {
    let mut log = LOG.lock().unwrap();
    for e in list.into_iter().rev() {
        log.push_front(e);
    }
    while log.len() > LOG_MAX {
        log.pop_front();
    }
}

fn clock() -> String {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        let mut t = unsafe { std::mem::zeroed() };
        unsafe { GetLocalTime(&mut t) };
        return format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond);
    }
    #[allow(unreachable_code)]
    String::new()
}

fn push(entry: LogEntry) {
    if let Some(f) = LOG_FILE.lock().unwrap().as_mut() {
        let _ = writeln!(f, "{} {} {}", entry.time, entry.level, entry.text);
    }
    {
        let mut log = LOG.lock().unwrap();
        log.push_back(entry.clone());
        while log.len() > LOG_MAX {
            log.pop_front();
        }
    }
    PENDING.lock().unwrap().push(entry);
}

/// Строка от самого kl!ck.
pub fn note(level: &'static str, text: &str) {
    push(LogEntry { time: clock(), level, text: text.to_string() });
}

/// `time="2026-09-23T13:45:01.12+03:00" level=warning msg="…"` → запись.
fn parse_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let field = |key: &str| -> Option<String> {
        let start = line.find(&format!("{key}="))? + key.len() + 1;
        let rest = &line[start..];
        if let Some(r) = rest.strip_prefix('"') {
            let mut out = String::new();
            let mut chars = r.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(n) = chars.next() {
                            out.push(n);
                        }
                    }
                    '"' => break,
                    c => out.push(c),
                }
            }
            Some(out)
        } else {
            Some(rest.split_whitespace().next().unwrap_or("").to_string())
        }
    };
    let (Some(level), Some(msg)) = (field("level"), field("msg")) else {
        return Some(LogEntry { time: clock(), level: "INFO", text: line.to_string() });
    };
    let time = field("time").and_then(|t| t.get(11..19).map(String::from)).unwrap_or_else(clock);
    let level = match level.as_str() {
        "warning" | "warn" => "WARN",
        "error" | "fatal" | "panic" => "ERR",
        "debug" => "DEBUG",
        _ => "INFO",
    };
    Some(LogEntry { time, level, text: msg })
}

// ─────────── Пути ───────────

pub fn bin_path(app: &AppHandle, name: &str) -> PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        let p = dir.join("bin").join(name);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin").join(name)
}

pub fn mihomo_exe(app: &AppHandle) -> PathBuf {
    bin_path(app, "mihomo.exe")
}

/// Домашняя папка mihomo: конфиг, кэш, GeoIP-база.
pub fn home(app: &AppHandle) -> PathBuf {
    let h = app.state::<AppState>().dir.join("core");
    let _ = std::fs::create_dir_all(&h);
    // База GeoIP лежит рядом с программой; mihomo ищет её у себя дома.
    crate::geo::ensure(app, &h);
    h
}

pub fn mihomo_version(app: &AppHandle) -> Option<String> {
    let mut cmd = Command::new(mihomo_exe(app));
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::sys::CREATE_NO_WINDOW);
    }
    let out = cmd.arg("-v").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.split_whitespace().find(|w| w.starts_with('v') && w[1..].starts_with(|c: char| c.is_ascii_digit())).map(String::from)
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).unwrap_or(9097)
}

fn spawn(app: &AppHandle, home: &std::path::Path, cfg: &std::path::Path, quiet: bool) -> Result<Child, String> {
    let exe = mihomo_exe(app);
    if !exe.exists() {
        return Err(format!("Не найден mihomo.exe ({}) — переустановите kl!ck.", exe.display()));
    }
    let mut cmd = Command::new(&exe);
    cmd.arg("-d").arg(home).arg("-f").arg(cfg);
    cmd.stdin(Stdio::null());
    if quiet {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    } else {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(crate::sys::CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = cmd.spawn().map_err(|e| format!("mihomo не запустился: {e}"))?;
    job::assign(&child);
    if !quiet {
        for pipe in [
            child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
            child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            std::thread::spawn(move || crate::sys::for_each_line(pipe, |l| {
                if let Some(e) = parse_line(&l) {
                    push(e);
                }
            }));
        }
    }
    Ok(child)
}

fn write_config(app: &AppHandle, profile: &Profile, port: u16, secret: &str) -> Result<(PathBuf, usize), String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let built = config::build(profile, &settings, port, secret)?;
    let path = home(app).join("config.yaml");
    std::fs::write(&path, serde_json::to_vec_pretty(&built.config).unwrap_or_default()).map_err(|e| format!("не удалось записать конфиг: {e}"))?;
    Ok((path, built.rules))
}

fn server_name(p: &Profile) -> Option<String> {
    p.active_proxy().and_then(|v| v.get("name")).and_then(Value::as_str).map(String::from)
}

// ─────────── Подключение ───────────

pub fn connect(app: &AppHandle) -> Result<(), String> {
    let _op = OP.lock().unwrap();
    connect_locked(app, false)
}

fn connect_locked(app: &AppHandle, reconnecting: bool) -> Result<(), String> {
    if !reconnecting {
        STOP.store(false, Ordering::SeqCst);
    } else if STOP.load(Ordering::SeqCst) {
        return Err("Переподключение отменено.".into());
    }
    if CHILD.lock().unwrap().as_mut().map(|c| c.try_wait().ok().flatten().is_none()).unwrap_or(false) {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let Some(profile) = state.active_profile() else {
        return Err("Нет подключений — сначала добавьте подписку или конфигурацию.".into());
    };

    set_status(app, |s| {
        s.state = if reconnecting { "reconnecting" } else { "connecting" };
        s.error = None;
        s.warning = None;
        s.profile_id = Some(profile.id.clone());
        s.server = server_name(&profile);
        s.mode = settings.mode;
        s.health = None;
    });

    let fail = |app: &AppHandle, msg: String| -> Result<(), String> {
        note("ERR", &format!("не подключилось: {msg}"));
        after_down(app);
        set_status(app, |s| {
            s.state = "error";
            s.error = Some(msg.clone());
            s.since = None;
        });
        Err(msg)
    };

    if settings.mode != Mode::Tun && std::net::TcpListener::bind(("127.0.0.1", settings.proxy_port)).is_err() {
        return fail(app, format!("Порт {} занят другой программой — смените его или закройте её.", settings.proxy_port));
    }

    let port = free_port();
    let secret = crate::sys::random_hex(16);
    let (cfg, rules) = match write_config(app, &profile, port, &secret) {
        Ok(v) => v,
        Err(e) => return fail(app, e),
    };

    let mut warning = None;

    *LOG_FILE.lock().unwrap() = std::fs::File::create(state.dir.join("mihomo.log")).ok();
    *TRAFFIC.lock().unwrap() = Traffic::default();
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    note(
        "INFO",
        &format!(
            "Подключение → {} [{}/{}], правил: {rules}",
            server_name(&profile).unwrap_or_default(),
            match settings.mode {
                Mode::Tun => "tun",
                Mode::Proxy => "proxy",
                Mode::Sysproxy => "sysproxy",
            },
            settings.route_mode.as_str()
        ),
    );

    let child = match spawn(app, &home(app), &cfg, false) {
        Ok(c) => c,
        Err(e) => return fail(app, e),
    };
    *CHILD.lock().unwrap() = Some(child);

    let api = Api { port, secret };
    let started = Instant::now();
    loop {
        if api.alive() {
            break;
        }
        if STOP.load(Ordering::SeqCst) {
            kill_child();
            note("INFO", "Подключение отменено");
            after_down(app);
            set_status(app, |s| {
                s.state = "off";
                s.since = None;
            });
            return Err("Подключение отменено.".into());
        }
        let exited = CHILD.lock().unwrap().as_mut().and_then(|c| c.try_wait().ok().flatten());
        if exited.is_some() || started.elapsed() > Duration::from_secs(20) {
            if exited.is_none() {
                kill_child();
            }
            std::thread::sleep(Duration::from_millis(200));
            return fail(app, explain_failure(&logs()));
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    // Kill Switch снимаем только теперь, когда туннель поднят: раньше
    // защищённые программы на эти секунды уходили бы напрямую. Снимаем
    // лишь там, где он мешал бы (см. ks_engaged).
    if let Some(w) = ks_apply(app, ks_engaged(&settings, true), &settings.ks_apps, &settings.ks_sites) {
        note("WARN", &w);
    }

    if settings.mode == Mode::Sysproxy {
        if let Err(e) = crate::sysproxy::enable(&state.dir, settings.proxy_port) {
            warning.get_or_insert(format!("Не удалось включить системный прокси: {e}"));
        }
    }
    *API.lock().unwrap() = Some(api.clone());
    note("INFO", "Туннель поднят");
    set_status(app, |s| {
        s.state = "on";
        s.since = Some(now_secs());
        s.warning = warning.clone();
    });

    {
        let app = app.clone();
        let api = api.clone();
        std::thread::spawn(move || {
            api.stream_traffic(
                |up, down| {
                    let t = {
                        let mut t = TRAFFIC.lock().unwrap();
                        t.up = up;
                        t.down = down;
                        t.up_total += up;
                        t.down_total += down;
                        t.clone()
                    };
                    let _ = app.emit("traffic", t);
                },
                || GEN.load(Ordering::SeqCst) == gen,
            );
        });
    }
    spawn_health(app.clone(), gen);
    Ok(())
}

/// Понятная причина по последним строкам лога.
fn explain_failure(log: &[LogEntry]) -> String {
    let tail: Vec<&LogEntry> = log.iter().rev().take(25).collect();
    let low: String = tail.iter().map(|e| e.text.to_lowercase()).collect::<Vec<_>>().join("\n");
    if low.contains("access is denied") || low.contains("отказано в доступе") {
        return "Не удалось создать VPN-адаптер: нет прав администратора. Запустите kl!ck от администратора или выберите режим Proxy.".into();
    }
    if low.contains("only one usage of each socket address") || low.contains("address already in use") {
        return "Порт уже занят другой программой.".into();
    }
    if low.contains("wintun") || low.contains("configure tun") {
        return "Не удалось поднять VPN-адаптер. Возможно, мешает другой VPN — отключите его и попробуйте снова.".into();
    }
    if let Some(e) = tail.iter().find(|e| e.level == "ERR") {
        return format!("mihomo остановился: {}", e.text);
    }
    "mihomo не запустился — подробности в логах.".into()
}

fn kill_child() {
    if let Some(mut c) = CHILD.lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// После любого падения туннеля: Kill Switch снова блокирует, системный
/// прокси возвращается как был.
fn after_down(app: &AppHandle) {
    *API.lock().unwrap() = None;
    let state = app.state::<AppState>();
    let (ks, apps, sites, port) = {
        let s = state.settings.lock().unwrap();
        (s.kill_switch, s.ks_apps.clone(), s.ks_sites.clone(), s.proxy_port)
    };
    if let Some(w) = ks_apply(app, ks, &apps, &sites) {
        note("WARN", &w);
    }
    crate::sysproxy::disable(&state.dir, port);
    // В кэше DNS Windows остались ответы fake-ip (198.18.x.x) — без туннеля
    // они ведут в никуда.
    flush_dns();
}

fn flush_dns() {
    let mut cmd = Command::new("ipconfig");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::sys::CREATE_NO_WINDOW);
    }
    let _ = cmd.arg("/flushdns").stdout(Stdio::null()).stderr(Stdio::null()).status();
}

/// Должен ли Kill Switch блокировать прямо сейчас. Выключен VPN — да.
/// Включён — снимается только в TUN, где трафик защищённых программ и так
/// идёт в туннель, а правило брандмауэра закрыло бы и его. В Proxy и
/// «Системном proxy» программа, которая прокси игнорирует (игры, торренты),
/// иначе ушла бы напрямую; loopback брандмауэр не фильтрует, так что
/// ходящие через 127.0.0.1 продолжают работать. В режиме «Напрямую» ядро
/// ничего не заворачивает в туннель — защита остаётся.
pub fn ks_engaged(s: &crate::state::Settings, on: bool) -> bool {
    use crate::state::RouteMode;
    s.kill_switch && !(on && s.mode == Mode::Tun && s.route_mode != RouteMode::Direct)
}

/// Привести Kill Switch к текущим настройкам и состоянию.
pub fn ks_sync(app: &AppHandle) -> Option<String> {
    let s = app.state::<AppState>().settings.lock().unwrap().clone();
    ks_apply(app, ks_engaged(&s, is_on()), &s.ks_apps, &s.ks_sites)
}

/// Kill Switch с отчётом окну: проблема появилась или ушла — событие
/// «killswitch» (строка или null).
pub fn ks_apply(app: &AppHandle, engage: bool, apps: &[crate::state::KsApp], sites: &[crate::state::KsSite]) -> Option<String> {
    let before = crate::killswitch::issue();
    let w = crate::killswitch::apply(engage, apps, sites);
    if w != before {
        let _ = app.emit("killswitch", w.clone());
    }
    w
}

pub fn disconnect(app: &AppHandle) {
    STOP.store(true, Ordering::SeqCst);
    let _op = OP.lock().unwrap();
    GEN.fetch_add(1, Ordering::SeqCst);
    if let Some(mut c) = CHILD.lock().unwrap().take() {
        if c.try_wait().ok().flatten().is_none() {
            let graceful = ctrl::send_break(c.id());
            let deadline = Instant::now() + Duration::from_secs(if graceful { 6 } else { 0 });
            while Instant::now() < deadline && c.try_wait().ok().flatten().is_none() {
                std::thread::sleep(Duration::from_millis(100));
            }
            if c.try_wait().ok().flatten().is_none() {
                let _ = c.kill();
            }
            let _ = c.wait();
        }
    }
    after_down(app);
    note("INFO", "Отключено, VPN-адаптер удалён");
    set_status(app, |s| {
        s.state = "off";
        s.error = None;
        s.warning = None;
        s.since = None;
        s.health = None;
    });
    *TRAFFIC.lock().unwrap() = Traffic::default();
}

pub fn reconnect(app: &AppHandle) -> Result<(), String> {
    if !is_on() {
        return Ok(());
    }
    disconnect(app);
    connect(app)
}

/// Сервер внутри подключения — на лету.
pub fn select_server(app: &AppHandle, profile_id: &str, server: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut v = state.vault.lock().unwrap();
        let p = v.profiles.iter_mut().find(|p| p.id == profile_id).ok_or("Такого подключения нет.")?;
        if !p.proxies.iter().any(|x| x.get("name").and_then(Value::as_str) == Some(server)) {
            return Err("Такого сервера нет.".into());
        }
        p.active = Some(server.to_string());
    }
    state.save_vault()?;
    let live = is_on() && STATUS.lock().unwrap().profile_id.as_deref() == Some(profile_id);
    if live {
        if let Some(api) = api() {
            api.select(GROUP, server)?;
        }
        note("INFO", &format!("Сервер: {server}"));
        set_status(app, |s| {
            s.server = Some(server.to_string());
            s.health = None;
        });
        spawn_health(app.clone(), GEN.load(Ordering::SeqCst));
    }
    Ok(())
}

/// Другое подключение: при работающем туннеле — переподключение.
pub fn select_profile(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !state.vault.lock().unwrap().profiles.iter().any(|p| p.id == id) {
        return Err("Такого подключения нет.".into());
    }
    state.settings.lock().unwrap().active_profile = Some(id.to_string());
    state.save_settings();
    if is_on() && STATUS.lock().unwrap().profile_id.as_deref() != Some(id) {
        return reconnect(app);
    }
    let p = state.active_profile();
    set_status(app, |s| {
        if s.state != "on" {
            s.profile_id = Some(id.to_string());
            s.server = p.as_ref().and_then(server_name);
        }
    });
    Ok(())
}

/// Правила/пресеты поменялись: перечитать конфиг на лету.
pub fn apply_config(app: &AppHandle) -> Result<(), String> {
    let Some(api) = api() else { return Ok(()) };
    let state = app.state::<AppState>();
    let Some(profile) = state.active_profile() else { return Ok(()) };
    let (path, rules) = write_config(app, &profile, api.port, &api.secret)?;
    api.reload(&path).map_err(|e| format!("mihomo не принял конфиг: {e}"))?;
    // После перезагрузки группа снова на первом — это и есть выбранный.
    note("INFO", &format!("Правила обновлены: {rules}"));
    Ok(())
}

pub fn set_route_mode(mode: &str) -> Result<(), String> {
    match api() {
        Some(api) => api.set_mode(mode),
        None => Ok(()),
    }
}

fn spawn_health(app: AppHandle, gen: u64) {
    std::thread::spawn(move || check_health(&app, gen));
}

fn check_health(app: &AppHandle, gen: u64) {
    let Some(api) = api() else { return };
    let r = api.delay(GROUP, 6000);
    if GEN.load(Ordering::SeqCst) != gen {
        return;
    }
    let h = Health { ok: r.is_ok(), ms: r.as_ref().ok().copied(), at: now_secs() };
    let was_ok = STATUS.lock().unwrap().health.as_ref().map(|h| h.ok);
    let notify = app.state::<AppState>().settings.lock().unwrap().notify_drops;
    if was_ok == Some(true) && !h.ok {
        note("WARN", "Сервер перестал отвечать");
        if notify {
            crate::notify::send(app, "Сервер не отвечает", "Туннель поднят, но через сервер ничего не открывается.", true);
        }
    } else if was_ok == Some(false) && h.ok {
        note("INFO", "Сервер снова отвечает");
    }
    // Сервер, через который идёт трафик, — для задержки на карточке.
    let (server, pid) = {
        let st = STATUS.lock().unwrap();
        (st.server.clone(), st.profile_id.clone())
    };
    if let (Some(server), Some(pid)) = (&server, &pid) {
        remember_pings(pid, &HashMap::from([(server.clone(), h.ms)]));
    }
    if let (Some(ms), Some(server)) = (h.ms, server) {
        let _ = app.emit("pings", HashMap::from([(server, ms)]));
    }
    set_status(app, |s| s.health = Some(h.clone()));
}

/// Задержка до каждого сервера подключения. Подключено к нему —
/// замер через работающее ядро; нет — через отдельный временный mihomo
/// без TUN и прокси, только с API: так честно проверяются и UDP-протоколы.
pub fn ping_profile(app: &AppHandle, id: &str) -> Result<HashMap<String, Option<u32>>, String> {
    let profile = app.state::<AppState>().vault.lock().unwrap().profiles.iter().find(|p| p.id == id).cloned().ok_or("Такого подключения нет.")?;
    let names: Vec<String> = profile.proxies.iter().filter_map(|p| p.get("name").and_then(Value::as_str).map(String::from)).collect();
    let live = is_on() && STATUS.lock().unwrap().profile_id.as_deref() == Some(id);
    let measured = if live {
        api().map(|a| a.group_delay(GROUP, 5000)).unwrap_or_default()
    } else {
        probe(app, &profile.proxies)?
    };
    let result: HashMap<String, Option<u32>> = names.into_iter().map(|n| {
        let ms = measured.get(&n).copied();
        (n, ms)
    }).collect();
    remember_pings(id, &result);
    Ok(result)
}

fn probe(app: &AppHandle, proxies: &[Value]) -> Result<HashMap<String, u32>, String> {
    let dir = app.state::<AppState>().dir.join("probe");
    let _ = std::fs::create_dir_all(&dir);
    let port = free_port();
    let secret = crate::sys::random_hex(8);
    let cfg = dir.join("config.yaml");
    std::fs::write(&cfg, serde_json::to_vec(&config::probe(proxies, port, &secret)).unwrap_or_default()).map_err(|e| e.to_string())?;
    let mut child = spawn(app, &dir, &cfg, true)?;
    let api = Api { port, secret };
    let started = Instant::now();
    while !api.alive() {
        if child.try_wait().ok().flatten().is_some() || started.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            return Err("Не удалось проверить задержку: ядро не запустилось.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let r = api.group_delay(GROUP, 5000);
    let _ = child.kill();
    let _ = child.wait();
    Ok(r)
}

/// Скачано программами за последнюю секунду, байт/с, по имени exe.
pub fn process_activity() -> HashMap<String, u64> {
    let Some(api) = api() else { return HashMap::new() };
    let a = api.download_by_process();
    std::thread::sleep(Duration::from_millis(1000));
    let b = api.download_by_process();
    b.into_iter().map(|(k, v)| {
        let prev = a.get(&k).copied().unwrap_or(v);
        (k, v.saturating_sub(prev))
    }).collect()
}

/// Фоновый присмотр: лог в окно, падение ядра, проверка связи раз в минуту.
pub fn start_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_health = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(300));
            let batch = std::mem::take(&mut *PENDING.lock().unwrap());
            if !batch.is_empty() {
                let _ = app.emit("logs", batch);
            }
            let crashed = {
                let mut child = CHILD.lock().unwrap();
                match child.as_mut().map(|c| c.try_wait()) {
                    Some(Ok(Some(code))) if !STOP.load(Ordering::SeqCst) && is_on() => {
                        *child = None;
                        Some(code)
                    }
                    _ => None,
                }
            };
            if let Some(code) = crashed {
                let app = app.clone();
                std::thread::spawn(move || on_crash(&app, code));
                continue;
            }
            if is_on() && last_health.elapsed() > Duration::from_secs(60) {
                last_health = Instant::now();
                spawn_health(app.clone(), GEN.load(Ordering::SeqCst));
            }
            // Сайты Kill Switch: пока защита действует, их адреса резолвим заново.
            if crate::killswitch::resolve_due() {
                let app = app.clone();
                std::thread::spawn(move || {
                    let _op = OP.lock().unwrap();
                    ks_sync(&app);
                });
            }
        }
    });
}

fn on_crash(app: &AppHandle, code: std::process::ExitStatus) {
    let _op = OP.lock().unwrap();
    GEN.fetch_add(1, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(200));
    let reason = explain_failure(&logs());
    note("ERR", &format!("mihomo неожиданно завершился (код {})", code.code().unwrap_or(-1)));
    after_down(app);
    let notify = app.state::<AppState>().settings.lock().unwrap().notify_drops;
    if notify {
        crate::notify::send(app, "Соединение оборвалось", "Пробую подключиться снова…", true);
    }
    for attempt in 1..=3 {
        std::thread::sleep(Duration::from_secs(3));
        if STOP.load(Ordering::SeqCst) {
            return;
        }
        note("INFO", &format!("Переподключение, попытка {attempt} из 3"));
        if connect_locked(app, true).is_ok() {
            return;
        }
    }
    set_status(app, |s| {
        s.state = "error";
        s.error = Some(reason.clone());
        s.since = None;
    });
}

/// Выход: ядро остановить, прокси вернуть. Kill Switch остаётся: это его
/// работа — не выпустить защищённые программы без VPN.
pub fn shutdown(app: &AppHandle) {
    disconnect(app);
}

mod ctrl {
    #[cfg(target_os = "windows")]
    pub fn send_break(pid: u32) -> bool {
        use once_cell::sync::Lazy;
        use std::sync::Mutex;
        use windows_sys::Win32::System::Console::{AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler, CTRL_BREAK_EVENT};
        static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
        let _g = LOCK.lock().unwrap();
        unsafe {
            FreeConsole();
            if AttachConsole(pid) == 0 {
                return false;
            }
            SetConsoleCtrlHandler(None, 1);
            let ok = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid) != 0;
            FreeConsole();
            ok
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn send_break(_pid: u32) -> bool {
        false
    }
}

mod job {
    #[cfg(target_os = "windows")]
    pub fn assign(child: &std::process::Child) {
        use once_cell::sync::Lazy;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;

        struct Job(isize);
        unsafe impl Send for Job {}
        unsafe impl Sync for Job {}
        static JOB: Lazy<Job> = Lazy::new(|| unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if !h.is_null() {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(h, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32);
            }
            Job(h as isize)
        });
        if JOB.0 != 0 {
            unsafe { AssignProcessToJobObject(JOB.0 as _, child.as_raw_handle() as _) };
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn assign(_child: &std::process::Child) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn строки_лога_mihomo() {
        let e = parse_line(r#"time="2026-09-23T13:45:01.123456+03:00" level=warning msg="[TCP] dial PROXY (match Match/) 127.0.0.1:1 --> \"x\" error""#).unwrap();
        assert_eq!(e.time, "13:45:01");
        assert_eq!(e.level, "WARN");
        assert!(e.text.contains("\"x\""));
        let e = parse_line("time=\"2026-09-23T13:45:02+03:00\" level=fatal msg=\"Parse config error: bad\"").unwrap();
        assert_eq!(e.level, "ERR");
        assert_eq!(parse_line("просто строка").unwrap().level, "INFO");
        assert!(parse_line("   ").is_none());
    }

    #[test]
    fn причины_падения() {
        let log = |t: &str| vec![LogEntry { time: String::new(), level: "ERR", text: t.into() }];
        assert!(explain_failure(&log("Start TUN listening error: configure tun interface: Access is denied.")).contains("администратора"));
        assert!(explain_failure(&log("listen tcp 127.0.0.1:7890: bind: Only one usage of each socket address")).contains("Порт"));
        assert_eq!(explain_failure(&log("что-то")), "mihomo остановился: что-то");
    }
}
