//! Движок службы: единственный владелец состояния. Команды приходят по очереди,
//! поэтому гонок между «подключить», «сменить сервер» и падением ядра нет.

use crate::core::{random_secret, CoreApi, CoreProcess};
use crate::ipcheck::{self, Plan};
use crate::killswitch;
#[cfg(windows)]
use crate::killswitch::Wfp;
use crate::logring;
use crate::neighbors;
#[cfg(unix)]
use crate::netconf;
#[cfg(unix)]
use crate::pf;
use crate::netwatch::NetWatch;
use crate::paths::{Paths, Profile};
use crate::programs;
use crate::storage::{self, Secrets};
use crate::subs;
use crate::sys;
#[cfg(windows)]
use crate::userproxy;
use klick_core::compile::{self, Capture, CompileInput, CoreLayout, SetFiles, PROBE_URL, PROVIDER, VPN_GROUP};
use klick_core::guard::{Action, Guard, Health, Notice};
use klick_core::sub::{self as subparse, Content, SourceKind};
use klick_core::{
    normalize_cidr, normalize_domain, program_folder_on, Catalog, Connection, ConnectionKind, InputError, KsProgram, Mode, Os, Routing, Rule,
    ServerDownPolicy, Settings, Target,
};
use klick_proto::{
    AboutView, Command, ConnectionView, ErrorInfo, Event, FailureView, KsProgramView, Preferences, ServerView, StateView, SystemProxy, UpdateView, VpnState,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::Instant;

pub const TUN_DEVICE: &str = "klick";
const PROBE_TIMEOUT_MS: u32 = 5_000;

pub enum Msg {
    Cmd(Command, oneshot::Sender<Result<Value, ErrorInfo>>),
    /// Что нужно проверке IP: сама проверка идёт вне очереди, чтобы не задерживать команды.
    IpPlan(oneshot::Sender<Plan>),
    /// Windows сообщила об изменении сети (после сна, смены Wi-Fi, отключения кабеля).
    NetworkChanged,
    CoreExited { generation: u64, code: Option<i32> },
    /// Итог проверки сервера, которую страж связи запустил в фоне.
    Probed { generation: u64, retry: bool, ok: bool },
    /// macOS: ядро-страж Kill Switch завершилось само.
    #[cfg(unix)]
    KsCoreExited { generation: u64 },
    /// macOS: пора снова запустить стража после сбоя.
    #[cfg(unix)]
    KsRetry,
    /// macOS: раз в 10 с проверить, что правила pf и страж на месте.
    #[cfg(unix)]
    KsWatchdog,
    Shutdown(oneshot::Sender<()>),
    /// Ещё раз попробовать снять прокси, оставшийся с прошлого запуска: служба стартует
    /// раньше входа в Windows, и ветки реестра пользователя тогда ещё нет.
    #[cfg(windows)]
    RetryLeftoverProxy,
}

#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<Msg>,
    pub events: broadcast::Sender<Event>,
}

impl EngineHandle {
    pub async fn call(&self, cmd: Command) -> Result<Value, ErrorInfo> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Msg::Cmd(cmd, tx)).await.map_err(|_| ErrorInfo::new("service.stopping"))?;
        rx.await.map_err(|_| ErrorInfo::new("service.stopping"))?
    }

    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Shutdown(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }

    /// «Как вас видят сайты»: движок только говорит, через какие входы идти.
    pub async fn ip_check(&self) -> Result<Value, ErrorInfo> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Msg::IpPlan(tx)).await.map_err(|_| ErrorInfo::new("service.stopping"))?;
        let plan = rx.await.map_err(|_| ErrorInfo::new("service.stopping"))?;
        serde_json::to_value(ipcheck::run(plan).await).map_err(internal)
    }
}

struct Running {
    process: CoreProcess,
    generation: u64,
    capture: Capture,
    layout: CoreLayout,
    traffic: tokio::task::JoinHandle<()>,
    logs: tokio::task::JoinHandle<()>,
    started: Instant,
}

impl Running {
    fn abort_tasks(&self) {
        self.traffic.abort();
        self.logs.abort();
    }
}

/// macOS: работающее ядро-страж Kill Switch.
#[cfg(unix)]
struct KsCore {
    process: CoreProcess,
    generation: u64,
    layout: CoreLayout,
    /// Какие папки программ оно закрывает.
    folders: Vec<String>,
    started: Instant,
}

/// Последние неудачные соединения для «Не открывается?». Только в памяти.
type Failures = Arc<Mutex<VecDeque<FailureView>>>;
const FAILURES_KEPT: usize = 50;

pub struct Engine {
    paths: Paths,
    profile: Profile,
    settings: Settings,
    secrets: Secrets,
    catalog: Catalog,
    vpn: VpnState,
    since: Option<i64>,
    error: Option<String>,
    running: Option<Running>,
    generation: u64,
    guard: Option<Guard>,
    epoch: Instant,
    restarts: u8,
    /// Проверка сервера идёт в фоне (поколение ядра): страж ждёт её итога, а не запускает новую.
    probing: Option<u64>,
    proxy_applied: bool,
    /// Прокси поставила служба, потому что окна не было: что стояло у пользователя до этого.
    #[cfg(windows)]
    user_proxy_saved: Option<userproxy::Saved>,
    /// macOS: ядро-страж Kill Switch — держит TUN, пока его не держит основное ядро.
    #[cfg(unix)]
    ks_core: Option<KsCore>,
    #[cfg(unix)]
    ks_generation: u64,
    /// macOS: сколько раз подряд страж не запустился или упал — от этого пауза перед следующей попыткой.
    #[cfg(unix)]
    ks_failures: u8,
    #[cfg(unix)]
    ks_retry_pending: bool,
    /// macOS: стоят ли правила Kill Switch в pf; `None` — ещё не сверяли (при запуске службы).
    #[cfg(unix)]
    pf_on: Option<bool>,
    /// macOS: DNS сетевых служб подменён на адрес внутри адаптера (режим VPN).
    #[cfg(unix)]
    dns_overridden: bool,
    /// macOS: DNS-серверы системы, какими они были до подмены: через них ядро ищет адреса серверов VPN.
    /// Запоминаются при каждом запуске ядра, пока DNS не подменён; перечитывание конфига берёт последние.
    server_dns: Vec<String>,
    servers: HashMap<String, Vec<ServerView>>,
    failures: Failures,
    /// Следующая плановая работа: обновление подписок, предупреждения о сроке и трафике.
    next_maintenance: Instant,
    /// О чём уже предупредили: (подключение, повод), чтобы не повторяться.
    warned: HashSet<(String, &'static str)>,
    /// Восстановить VPN после сбоя службы (не после перезагрузки Windows).
    restore_pending: bool,
    /// VPN был включён, когда Windows выключили: окно вернёт его после входа, если включено «Восстанавливать подключение».
    resume_on_logon: bool,
    /// Какие exe сейчас закрыты Kill Switch: по нему видно, что программа обновилась или пропала.
    ks_applied: Vec<PathBuf>,
    core_version: Option<String>,
    _netwatch: Option<NetWatch>,
    events: broadcast::Sender<Event>,
    tx: mpsc::Sender<Msg>,
}

/// Что было перед остановкой службы: нужно, чтобы после сбоя вернуть VPN.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Runtime {
    connected: bool,
    /// Время загрузки Windows, секунды Unix: так отличаем сбой службы от перезагрузки.
    boot: i64,
}

#[cfg(windows)]
fn boot_time() -> i64 {
    let uptime = unsafe { windows::Win32::System::SystemInformation::GetTickCount64() } / 1000;
    unix_now() - uptime as i64
}

#[cfg(unix)]
fn boot_time() -> i64 {
    sys::boot_time()
}

/// Папка программы из пути: служба видит диск и подсказывает, файл это или папка —
/// на macOS у программы без пакета `.app` нет расширения, по которому её узнать.
fn program_folder(input: &str) -> Result<String, InputError> {
    let path = Path::new(input.trim().trim_matches('"'));
    let is_file = path.is_file();
    // macOS: правило сравнивают с настоящим путём процесса, поэтому ссылки раскрываем
    // (`/tmp` → `/private/tmp`, программа по символьной ссылке с другого диска).
    #[cfg(unix)]
    if let Some(real) = std::fs::canonicalize(path).ok().and_then(|p| p.to_str().map(str::to_string)) {
        return program_folder_on(Os::CURRENT, &real, Some(is_file));
    }
    program_folder_on(Os::CURRENT, input, Some(is_file))
}

/// Дождаться адаптера ядра (адрес 198.18.0.1).
#[cfg(unix)]
async fn wait_tun(limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while !sys::tun_up() {
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    true
}

fn read_runtime(paths: &Paths) -> Option<Runtime> {
    let text = std::fs::read(paths.data.join("runtime.json")).ok()?;
    serde_json::from_slice(&text).ok()
}

/// VPN был включён, и Windows с тех пор не перезагружалась: значит, упала служба.
fn should_restore(paths: &Paths) -> bool {
    read_runtime(paths).is_some_and(|rt| rt.connected && (boot_time() - rt.boot).abs() < 120)
}

/// VPN был включён, когда Windows выключили или перезагрузили.
fn was_on_before_reboot(paths: &Paths) -> bool {
    read_runtime(paths).is_some_and(|rt| rt.connected && (boot_time() - rt.boot).abs() >= 120)
}

pub fn spawn(paths: Paths, profile: Profile) -> anyhow::Result<(EngineHandle, tokio::task::JoinHandle<()>)> {
    paths.ensure_dirs()?;
    if !profile.dev {
        sys::restrict_to_admins(&paths.data)?;
    }
    #[cfg(unix)]
    sys::private_dir(&profile.run_dir())?;
    storage::install_core_files(&paths)?;
    let settings = storage::load_settings(&paths);
    let secrets = Secrets::load(&paths);
    let catalog = storage::load_catalog(&paths);
    tracing::info!(
        "данные: {} · подключений: {} · сервисов в каталоге: {}",
        paths.data.display(),
        settings.connections.len(),
        catalog.services.len()
    );
    let (tx, rx) = mpsc::channel(64);
    let (events, _) = broadcast::channel(256);
    let restore_pending = !profile.dev && should_restore(&paths);
    let resume_on_logon = !profile.dev && was_on_before_reboot(&paths);

    // Смена сети: пачку событий за 3 секунды превращаем в одно.
    let (net_tx, mut net_rx) = mpsc::unbounded_channel::<u64>();
    let netwatch = match NetWatch::start(net_tx) {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::warn!("не подписаться на смену сети: {e:#}");
            None
        }
    };
    // Прокси, оставшийся от прошлого запуска (компьютер выключили с включённым VPN), снимаем
    // и после входа в Windows: первые 15 минут пробуем каждые 3 секунды, пока есть отметка.
    #[cfg(windows)]
    if paths.data.join("user-proxy.json").exists() {
        let retry_tx = tx.clone();
        tokio::spawn(async move {
            for _ in 0..300 {
                tokio::time::sleep(Duration::from_secs(3)).await;
                if retry_tx.send(Msg::RetryLeftoverProxy).await.is_err() {
                    break;
                }
            }
        });
    }
    // macOS: сторож Kill Switch — правила pf могла сбросить другая программа. Проверка — три вызова
    // pfctl и только пока правила стоят; чем чаще, тем короче окно, если pf выключит кто-то другой.
    #[cfg(unix)]
    {
        let watch_tx = tx.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(10));
            tick.tick().await;
            loop {
                tick.tick().await;
                if watch_tx.send(Msg::KsWatchdog).await.is_err() {
                    break;
                }
            }
        });
    }
    let engine_tx = tx.clone();
    tokio::spawn(async move {
        while net_rx.recv().await.is_some() {
            while let Ok(Some(_)) = tokio::time::timeout(Duration::from_secs(3), net_rx.recv()).await {}
            if engine_tx.send(Msg::NetworkChanged).await.is_err() {
                break;
            }
        }
    });
    let engine = Engine {
        paths,
        profile,
        settings,
        secrets,
        catalog,
        vpn: VpnState::Off,
        since: None,
        error: None,
        running: None,
        generation: 0,
        guard: None,
        epoch: Instant::now(),
        restarts: 0,
        probing: None,
        proxy_applied: false,
        #[cfg(windows)]
        user_proxy_saved: None,
        #[cfg(unix)]
        ks_core: None,
        #[cfg(unix)]
        ks_generation: 0,
        #[cfg(unix)]
        ks_failures: 0,
        #[cfg(unix)]
        ks_retry_pending: false,
        #[cfg(unix)]
        pf_on: None,
        #[cfg(unix)]
        dns_overridden: false,
        server_dns: Vec::new(),
        servers: HashMap::new(),
        failures: Arc::new(Mutex::new(VecDeque::with_capacity(FAILURES_KEPT))),
        next_maintenance: Instant::now() + Duration::from_secs(60),
        warned: HashSet::new(),
        restore_pending,
        resume_on_logon,
        ks_applied: Vec::new(),
        core_version: None,
        _netwatch: netwatch,
        events: events.clone(),
        tx: tx.clone(),
    };
    let task = tokio::spawn(engine.run(rx));
    Ok((EngineHandle { tx, events }, task))
}

/// Имя значения, как его видит окно: `connected`, `sys_proxy`, `next_working`.
fn code_name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|x| x.as_str().map(str::to_string)).unwrap_or_default()
}

/// Версия ядра из `mihomo -v`: «Mihomo Meta v1.19.31 windows amd64 …» → «v1.19.31».
fn core_version(exe: &Path) -> Option<String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("-v");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace().find(|w| w.len() > 1 && w.starts_with('v') && w[1..].starts_with(|c: char| c.is_ascii_digit())).map(str::to_string)
}

/// Репозиторий с выпусками (`владелец/имя`) задаётся при сборке для публикации: `KLICK_RELEASES_REPO=vbu00/klick`.
/// До публикации проверка обновлений выключена (архитектура, следствия А2.1).
const RELEASES_REPO: Option<&str> = option_env!("KLICK_RELEASES_REPO");

/// Последний выпуск на GitHub. Выпусков нет — значит, обновляться не на что.
async fn check_update() -> Result<UpdateView, ErrorInfo> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let Some(repo) = RELEASES_REPO.filter(|r| !r.is_empty()) else {
        return Err(ErrorInfo::new("update.disabled"));
    };
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("klick/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(internal)?;
    let resp = client.get(&url).header("Accept", "application/vnd.github+json").send().await.map_err(|_| ErrorInfo::new("update.unreachable"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateView { current, latest: None, url: None, newer: false });
    }
    if !resp.status().is_success() {
        return Err(ErrorInfo::with("update.http_status", json!({ "status": resp.status().as_u16() })));
    }
    let v: Value = resp.json().await.map_err(|_| ErrorInfo::new("update.bad_answer"))?;
    let tag = v["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    let url = v["html_url"].as_str().map(str::to_string);
    let newer = version_newer(&tag, &current);
    Ok(UpdateView { current, latest: (!tag.is_empty()).then_some(tag), url, newer })
}

/// `0.4.10` новее `0.4.9`: сравниваем числа, а не строки.
fn version_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| v.split(['.', '-', '+']).take(3).map(|x| x.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(latest) > parse(current)
}

fn internal(e: impl std::fmt::Display) -> ErrorInfo {
    tracing::error!("внутренняя ошибка: {e:#}");
    ErrorInfo::new("service.internal")
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Engine {
    async fn run(mut self, mut rx: mpsc::Receiver<Msg>) {
        self.apply_killswitch().await;
        // macOS: служба перезапустилась (сбой, обновление, перезагрузка), а прокси и DNS остались
        // такими, какими их ставил kl!ck, — вернуть. Если VPN надо восстановить, он поставит их заново.
        #[cfg(unix)]
        if self.netconf_allowed() {
            let data = self.paths.data.clone();
            let (proxy, dns) = tokio::task::spawn_blocking(move || netconf::restore_leftovers(&data)).await.unwrap_or((false, false));
            if proxy {
                tracing::info!("снят оставшийся системный прокси kl!ck");
            }
            if dns {
                tracing::info!("возвращён DNS, который стоял до VPN");
            }
        }
        // Служба перезапустилась, а у пользователя остался прокси, который ставила она, — снять.
        #[cfg(windows)]
        let marker = self.paths.data.join("user-proxy.json");
        #[cfg(windows)]
        if let Some(saved) = std::fs::read(&marker).ok().and_then(|b| serde_json::from_slice::<userproxy::Saved>(&b).ok()) {
            if self.restore_pending {
                self.user_proxy_saved = Some(saved);
            } else {
                self.clear_leftover_proxy();
            }
        }
        if self.restore_pending {
            tracing::info!("служба перезапустилась после сбоя, а VPN был включён: подключаюсь снова");
            if let Err(e) = self.connect().await {
                tracing::warn!("восстановить подключение не вышло: {}", e.code);
            }
        }
        loop {
            let deadline = self.guard_deadline();
            let sleep_to = deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));
            let maintenance_at = self.next_maintenance;
            tokio::select! {
                // Команды окна — первыми: обслуживание и страж подождут, «Отключить» — нет.
                biased;
                msg = rx.recv() => {
                    let Some(msg) = msg else { break };
                    match msg {
                        Msg::Cmd(cmd, reply) => {
                            let res = self.handle(cmd).await;
                            let _ = reply.send(res);
                        }
                        Msg::IpPlan(reply) => {
                            let _ = reply.send(self.ip_plan());
                        }
                        Msg::NetworkChanged => self.on_network_change().await,
                        #[cfg(windows)]
                        Msg::RetryLeftoverProxy => self.clear_leftover_proxy(),
                        Msg::CoreExited { generation, code } => self.on_core_exit(generation, code).await,
                        Msg::Probed { generation, retry, ok } => {
                            if self.probing == Some(generation) {
                                self.probing = None;
                            }
                            if self.guard.is_some() && self.running.as_ref().map(|r| r.generation) == Some(generation) {
                                self.probed(retry, ok).await;
                            }
                        }
                        #[cfg(unix)]
                        Msg::KsCoreExited { generation } => self.on_ks_core_exit(generation).await,
                        #[cfg(unix)]
                        Msg::KsRetry => {
                            self.ks_retry_pending = false;
                            self.sync_ks_core().await;
                        }
                        #[cfg(unix)]
                        Msg::KsWatchdog => self.ks_watchdog().await,
                        Msg::Shutdown(done) => {
                            self.shutdown().await;
                            let _ = done.send(());
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep_until(maintenance_at) => self.maintenance().await,
                _ = tokio::time::sleep_until(sleep_to), if deadline.is_some() => self.on_guard().await,
            }
        }
    }

    async fn handle(&mut self, cmd: Command) -> Result<Value, ErrorInfo> {
        match cmd {
            Command::Status | Command::Subscribe => Ok(self.view_json()),
            Command::Connect => {
                self.connect().await?;
                Ok(self.view_json())
            }
            Command::Disconnect => {
                self.disconnect().await;
                Ok(self.view_json())
            }
            Command::SetMode { mode } => {
                if self.settings.mode != mode {
                    self.settings.mode = mode;
                    self.save()?;
                    if self.running.is_some() {
                        self.reconnect().await?;
                    } else {
                        self.emit_state();
                    }
                }
                Ok(self.view_json())
            }
            Command::SetRouting { routing } => {
                if self.settings.routing != routing {
                    self.settings.routing = routing;
                    self.save()?;
                    self.reload_rules().await;
                    self.emit_state();
                }
                Ok(self.view_json())
            }
            Command::AddConnection { source, name } => self.add_connection(source, name).await,
            Command::FindConnection { source } => {
                let found = self.find_connection(&source).map(|c| ConnectionView { id: c.id.clone(), name: c.name.clone(), info: c.info.clone() });
                serde_json::to_value(found).map_err(internal)
            }
            Command::ImportFile { file_name, content } => self.import_file(file_name, content).await,
            Command::RefreshConnection { id } => self.refresh(&id).await,
            Command::RemoveConnection { id } => self.remove(&id).await,
            Command::SelectConnection { id } => self.select_connection(&id).await,
            Command::Servers => self.list_servers(false).await,
            Command::TestLatency => self.list_servers(true).await,
            Command::SelectServer { name } => self.select_server(name).await,
            Command::ListAdd { position, rule } => {
                let rule = self.validate_rule(rule)?;
                let list = self.settings.lists.get_mut(position);
                if list.iter().any(|r| r.target == rule.target) {
                    return Err(ErrorInfo::new("list.duplicate"));
                }
                list.push(rule);
                self.lists_changed(position).await
            }
            Command::ListRemove { position, index } => {
                let list = self.settings.lists.get_mut(position);
                if index >= list.len() {
                    return Err(ErrorInfo::new("list.bad_index"));
                }
                list.remove(index);
                self.lists_changed(position).await
            }
            Command::ListSetRoute { position, index, route } => {
                let list = self.settings.lists.get_mut(position);
                let rule = list.get_mut(index).ok_or_else(|| ErrorInfo::new("list.bad_index"))?;
                rule.route = route;
                self.lists_changed(position).await
            }
            Command::ListSetEnabled { position, index, enabled } => {
                let list = self.settings.lists.get_mut(position);
                let rule = list.get_mut(index).ok_or_else(|| ErrorInfo::new("list.bad_index"))?;
                rule.enabled = enabled;
                self.lists_changed(position).await
            }
            Command::KillSwitchSet { enabled } => {
                self.settings.kill_switch.enabled = enabled;
                self.killswitch_changed().await
            }
            Command::KillSwitchAdd { folder } => {
                let folder = program_folder(&folder).map_err(|e| ErrorInfo::new(e.code()))?;
                if self.settings.kill_switch.programs.iter().any(|p| p.folder.eq_ignore_ascii_case(&folder)) {
                    return Err(ErrorInfo::new("list.duplicate"));
                }
                let exes = killswitch::scan_folder(Path::new(&folder));
                if exes.is_empty() {
                    return Err(ErrorInfo::with("killswitch.no_programs", json!({ "folder": folder })));
                }
                self.settings.kill_switch.programs.push(KsProgram { folder, enabled: true });
                self.killswitch_changed().await
            }
            Command::KillSwitchRemove { folder } => {
                let before = self.settings.kill_switch.programs.len();
                self.settings.kill_switch.programs.retain(|p| !p.folder.eq_ignore_ascii_case(folder.trim()));
                if self.settings.kill_switch.programs.len() == before {
                    return Err(ErrorInfo::new("list.not_found"));
                }
                self.killswitch_changed().await
            }
            Command::KillSwitchProgram { folder, enabled } => {
                let p = self.settings.kill_switch.programs.iter_mut().find(|p| p.folder.eq_ignore_ascii_case(folder.trim())).ok_or_else(|| ErrorInfo::new("list.not_found"))?;
                p.enabled = enabled;
                self.killswitch_changed().await
            }
            Command::KillSwitchStatus => {
                let programs = self.settings.kill_switch.programs.clone();
                let list = tokio::task::spawn_blocking(move || {
                    programs
                        .into_iter()
                        .map(|p| KsProgramView { exes: killswitch::scan_folder(Path::new(&p.folder)).len() as u32, folder: p.folder, enabled: p.enabled })
                        .collect::<Vec<_>>()
                })
                .await
                .map_err(internal)?;
                serde_json::to_value(list).map_err(internal)
            }
            Command::Resume => {
                if std::mem::take(&mut self.resume_on_logon) && self.settings.restore_on_logon && self.vpn == VpnState::Off {
                    tracing::info!("VPN был включён до перезагрузки, «Восстанавливать подключение» включено: подключаюсь");
                    self.connect().await?;
                }
                Ok(self.view_json())
            }
            Command::About => serde_json::to_value(self.about().await).map_err(internal),
            Command::Log => serde_json::to_value(logring::recent()).map_err(internal),
            Command::LogClear => {
                logring::clear();
                Ok(json!([]))
            }
            Command::Report => Ok(Value::String(self.report().await)),
            Command::CheckUpdate => serde_json::to_value(check_update().await?).map_err(internal),
            Command::SetPreferences { prefs } => self.set_preferences(prefs).await,
            Command::IpCheck => serde_json::to_value(ipcheck::run(self.ip_plan()).await).map_err(internal),
            Command::Connections => {
                let Some(api) = self.api() else { return Ok(json!([])) };
                let v = api.get_json("/connections").await.map_err(internal)?;
                serde_json::to_value(ipcheck::connections(&v)).map_err(internal)
            }
            Command::Failures => {
                let list: Vec<FailureView> = self.failures.lock().unwrap().iter().rev().cloned().collect();
                serde_json::to_value(list).map_err(internal)
            }
            Command::Neighbors => serde_json::to_value(neighbors::scan(&self.paths.core_exe)).map_err(internal),
            Command::Settings => serde_json::to_value(&self.settings).map_err(internal),
            Command::Catalog => serde_json::to_value(&self.catalog).map_err(internal),
            Command::ProgramNames { mut folders } => {
                folders.truncate(200);
                let names = tokio::task::spawn_blocking(move || {
                    folders.into_iter().filter_map(|f| programs::folder_name(Path::new(&f)).map(|n| (f, n))).collect::<HashMap<String, String>>()
                })
                .await
                .map_err(internal)?;
                serde_json::to_value(names).map_err(internal)
            }
            Command::Programs => {
                let own = vec![self.paths.core_exe.clone(), std::env::current_exe().unwrap_or_default()];
                let list = tokio::task::spawn_blocking(move || programs::scan(&own)).await.map_err(internal)?;
                serde_json::to_value(list).map_err(internal)
            }
        }
    }

    /// Раз в 10 минут: обновить подписки по расписанию, предупредить о сроке и трафике,
    /// заметить, что программа из Kill Switch обновилась в новую папку или пропала.
    async fn maintenance(&mut self) {
        self.next_maintenance = Instant::now() + Duration::from_secs(600);
        self.check_killswitch_programs().await;
        let now = unix_now();
        let due: Vec<String> = self
            .settings
            .connections
            .iter()
            .filter(|c| c.kind == ConnectionKind::Subscription && self.settings.auto_update)
            .filter(|c| c.updated_at.map_or(true, |t| now - t >= i64::from(c.update_interval_hours.unwrap_or(12)) * 3600))
            .map(|c| c.id.clone())
            .collect();
        for id in due {
            match self.refresh(&id).await {
                Ok(_) => tracing::info!("подписка обновлена по расписанию"),
                Err(e) => tracing::warn!("подписка по расписанию не обновилась: {}", e.code),
            }
        }
        let mut notices = Vec::new();
        for c in &self.settings.connections {
            let Some(info) = &c.info else { continue };
            match info.expire {
                Some(e) if e <= now => {
                    if self.warned.insert((c.id.clone(), "expired")) {
                        notices.push(("sub.expired", c.name.clone()));
                    }
                }
                Some(e) if e - now <= 3 * 86_400 => {
                    if self.warned.insert((c.id.clone(), "expiring")) {
                        notices.push(("sub.expiring", c.name.clone()));
                    }
                }
                _ => {}
            }
            if info.total > 0 && info.upload + info.download >= info.total && self.warned.insert((c.id.clone(), "traffic")) {
                notices.push(("sub.traffic_out", c.name.clone()));
            }
        }
        for (code, name) in notices {
            self.notice(code, json!({ "name": name }));
        }
    }

    /// После сна, смены Wi-Fi или кабеля: сразу проверить связь, а не ждать плановой проверки.
    async fn on_network_change(&mut self) {
        // macOS: появилась новая сетевая служба (USB-модем, другой адаптер) — ей тоже прокси и DNS.
        #[cfg(unix)]
        self.refresh_netconf().await;
        let Some(started) = self.running.as_ref().map(|r| r.started) else { return };
        if started.elapsed() < Duration::from_secs(10) {
            return;
        }
        let Some(health) = self.guard.as_ref().map(Guard::health) else { return };
        if matches!(health, Health::Retrying { .. }) {
            return;
        }
        tracing::info!("сеть изменилась, проверяю связь");
        let ok = self.probe().await;
        let now = self.now_ms();
        let notice = self.guard.as_mut().and_then(|g| g.report(ok, now));
        self.after_guard(notice);
    }

    fn save_runtime(&self, connected: bool) {
        if self.profile.dev {
            return;
        }
        let rt = Runtime { connected, boot: boot_time() };
        if let Ok(bytes) = serde_json::to_vec(&rt) {
            let _ = storage::write_atomic(&self.paths.data.join("runtime.json"), &bytes);
        }
    }

    fn ip_plan(&self) -> Plan {
        Plan {
            vpn_port: self.running.as_ref().map(|r| r.layout.check_vpn_port),
            direct_port: self.running.as_ref().map(|r| r.layout.check_direct_port),
            tun: self.running.as_ref().is_some_and(|r| r.capture == Capture::Tun),
            all_vpn: self.settings.routing == Routing::AllVpn,
        }
    }

    /// Предупреждения ядра о неудачных соединениях — в память для «Не открывается?».
    fn spawn_logs(&self, api: CoreApi) -> tokio::task::JoinHandle<()> {
        let failures = self.failures.clone();
        tokio::spawn(async move {
            loop {
                let ring = failures.clone();
                let _ = api
                    .stream("/logs?level=warning", move |v| {
                        let Some(payload) = v["payload"].as_str() else { return };
                        if let Some(f) = ipcheck::parse_failure(payload, unix_now()) {
                            let mut r = ring.lock().unwrap();
                            if r.len() == FAILURES_KEPT {
                                r.pop_front();
                            }
                            r.push_back(f);
                        }
                    })
                    .await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    }

    async fn set_preferences(&mut self, p: Preferences) -> Result<Value, ErrorInfo> {
        if let Some(lang) = &p.language {
            if lang != "ru" && lang != "en" {
                return Err(ErrorInfo::with("input.bad_language", json!({ "language": lang })));
            }
        }
        if p.appearance.as_ref().is_some_and(|a| !a.is_valid()) {
            return Err(ErrorInfo::new("input.bad_appearance"));
        }
        let russia_before = self.settings.russia_direct;
        let preset_before = self.settings.blocked_preset;
        if let Some(v) = p.blocked_preset {
            self.settings.blocked_preset = v;
        }
        if let Some(v) = p.ru_domains {
            self.settings.russia_direct.domains = v;
        }
        if let Some(v) = p.ru_ips {
            self.settings.russia_direct.ips = v;
        }
        if let Some(v) = p.on_server_down {
            self.settings.on_server_down = v;
        }
        if let Some(v) = p.restore_on_logon {
            self.settings.restore_on_logon = v;
        }
        if let Some(v) = p.auto_update {
            self.settings.auto_update = v;
        }
        if let Some(v) = p.notify {
            self.settings.notify = v;
        }
        if let Some(v) = p.on_exit {
            self.settings.on_exit = v;
        }
        if let Some(a) = p.appearance {
            self.settings.appearance = a;
        }
        if let Some(v) = p.language {
            self.settings.language = v;
        }
        self.save()?;
        let changed = match self.settings.routing {
            Routing::AllVpn => russia_before != self.settings.russia_direct,
            Routing::Selected => preset_before != self.settings.blocked_preset,
        };
        if changed {
            self.reload_rules().await;
        }
        serde_json::to_value(&self.settings).map_err(internal)
    }

    // ── О приложении и отчёт ───────────────────────────────────────────────

    async fn about(&mut self) -> AboutView {
        if self.core_version.is_none() {
            let exe = self.paths.core_exe.clone();
            self.core_version = tokio::task::spawn_blocking(move || core_version(&exe)).await.ok().flatten();
        }
        AboutView {
            version: env!("CARGO_PKG_VERSION").into(),
            core_version: self.core_version.clone(),
            mixed_port: self.profile.mixed_port,
            data_dir: self.paths.data.display().to_string(),
            os: sys::os_name(),
            dev: self.profile.dev,
        }
    }

    /// Текст для поддержки: версии, состояние, счётчики и журнал. Без ссылок, имён подключений и адресов сайтов.
    async fn report(&mut self) -> String {
        use std::fmt::Write as _;
        let about = self.about().await;
        let s = &self.settings;
        let yes = |b: bool| if b { "да" } else { "нет" };
        let kinds = |k: ConnectionKind| s.connections.iter().filter(|c| c.kind == k).count();
        let mut out = String::new();
        let _ = writeln!(out, "kl!ck {} · ядро {} · {}{}", about.version, about.core_version.as_deref().unwrap_or("?"), about.os, if about.dev { " · разработка" } else { "" });
        let _ = writeln!(out, "Состояние: {} · режим {} · положение {}", code_name(&self.vpn), code_name(&s.mode), code_name(&s.routing));
        if let Some(e) = &self.error {
            let _ = writeln!(out, "Ошибка: {e}");
        }
        let ks_on = s.kill_switch.programs.iter().filter(|p| p.enabled).count();
        let _ = writeln!(out, "Kill Switch: {} · программ {} (включено {})", yes(s.kill_switch.enabled), s.kill_switch.programs.len(), ks_on);
        let _ = writeln!(out, "Подключения: подписок {}, ссылок {}, файлов {}", kinds(ConnectionKind::Subscription), kinds(ConnectionKind::Link), kinds(ConnectionKind::File));
        let _ = writeln!(out, "Правила: «Всё через VPN» {}, «Только выбранное» {}", s.lists.all_vpn.len(), s.lists.selected.len());
        let _ = writeln!(out, "Заблокированное через VPN: {}", yes(s.blocked_preset));
        let _ = writeln!(out, "Россия напрямую: домены {}, IP {}", yes(s.russia_direct.domains), yes(s.russia_direct.ips));
        let _ = writeln!(
            out,
            "Если сервер недоступен: {} · восстанавливать подключение: {} · обновлять подписки: {}",
            code_name(&s.on_server_down),
            yes(s.restore_on_logon),
            yes(s.auto_update)
        );
        let neighbors: Vec<String> = neighbors::scan(&self.paths.core_exe).into_iter().map(|n| format!("{} ({})", n.name, n.kind)).collect();
        let _ = writeln!(out, "Соседи: {}", if neighbors.is_empty() { "нет".into() } else { neighbors.join(", ") });
        let _ = writeln!(out, "Неудачных соединений в памяти: {}", self.failures.lock().unwrap().len());
        let _ = writeln!(out, "\nЖурнал службы (время UTC):");
        let log = logring::recent();
        for l in log.iter().skip(log.len().saturating_sub(80)) {
            let secs = l.at.div_euclid(1000).rem_euclid(86_400);
            let _ = writeln!(out, "{:02}:{:02}:{:02} {:5} {}", secs / 3600, secs / 60 % 60, secs % 60, l.level.to_uppercase(), l.text);
        }
        out
    }

    // ── Состояние ──────────────────────────────────────────────────────────

    fn view(&self) -> StateView {
        let active = self.settings.active();
        StateView {
            vpn: self.vpn,
            mode: self.settings.mode,
            routing: self.settings.routing,
            kill_switch: self.settings.kill_switch.enabled,
            connection: active.map(|c| ConnectionView { id: c.id.clone(), name: c.name.clone(), info: c.info.clone() }),
            server: active.and_then(|c| c.selected_server.clone()),
            since: self.since,
            attempt: if self.vpn == VpnState::Reconnecting { self.guard.as_ref().and_then(Guard::attempt_shown) } else { None },
            error: self.error.clone(),
            system_proxy: self.proxy_applied.then(|| self.proxy_spec()),
        }
    }

    fn proxy_spec(&self) -> SystemProxy {
        #[cfg(windows)]
        let bypass = vec!["localhost".into(), "127.*".into(), "10.*".into(), "172.16.*".into(), "192.168.*".into(), "<local>".into()];
        #[cfg(unix)]
        let bypass = netconf::BYPASS.iter().map(|s| s.to_string()).collect();
        SystemProxy { host: "127.0.0.1".into(), port: self.profile.mixed_port, bypass }
    }

    fn view_json(&self) -> Value {
        serde_json::to_value(self.view()).unwrap_or(Value::Null)
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    fn emit_state(&self) {
        self.emit(Event::State { state: self.view() });
    }

    fn notice(&self, code: &str, params: Value) {
        tracing::info!("уведомление {code}");
        self.emit(Event::Notice { code: code.into(), params });
    }

    fn set_state(&mut self, vpn: VpnState, error: Option<String>) {
        self.vpn = vpn;
        self.error = error;
        self.emit_state();
    }

    fn save(&self) -> Result<(), ErrorInfo> {
        storage::save_settings(&self.paths, &self.settings).map_err(|e| {
            tracing::error!("настройки не сохранились: {e:#}");
            ErrorInfo::new("storage.write_failed")
        })?;
        self.emit(Event::Settings);
        Ok(())
    }

    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    fn guard_deadline(&self) -> Option<Instant> {
        let g = self.guard.as_ref()?;
        let running = self.running.as_ref()?;
        if self.probing == Some(running.generation) {
            return None;
        }
        Some(self.epoch + Duration::from_millis(g.next().0))
    }

    fn set_files(&self) -> SetFiles {
        SetFiles {
            blocked_domains: "sets/blocked-domains.txt".into(),
            blocked_ips: "sets/blocked-ips.txt".into(),
            ru_domains: "sets/ru-domains.txt".into(),
        }
    }

    fn capture(&self) -> Capture {
        match self.settings.mode {
            Mode::Tun if self.profile.tun_allowed => Capture::Tun,
            Mode::Tun => {
                tracing::warn!("режим VPN (TUN) запрошен, но в разработке он выключен: работаю только через порт");
                Capture::Proxy
            }
            Mode::SysProxy => Capture::Proxy,
        }
    }

    fn api(&self) -> Option<CoreApi> {
        self.running.as_ref().map(|r| r.process.api.clone())
    }

    /// Порт, через который служба качает подписки и проверяет IP «через VPN».
    fn vpn_port(&self) -> Option<u16> {
        self.running.as_ref().map(|r| r.layout.check_vpn_port)
    }

    // ── Подключение ────────────────────────────────────────────────────────

    async fn connect(&mut self) -> Result<(), ErrorInfo> {
        if self.running.is_some() {
            return Ok(());
        }
        let conn = self.settings.active().cloned().ok_or_else(|| ErrorInfo::new("vpn.no_connection"))?;
        if !self.paths.provider_file(&conn.id).exists() {
            self.download(&conn.id).await?;
        }
        self.set_state(VpnState::Connecting, None);
        let capture = self.capture();
        if let Err(e) = self.start_core(capture).await {
            self.set_state(VpnState::Error, Some(e.code.clone()));
            return Err(e);
        }
        self.after_start().await;
        let ok = self.probe().await;
        let now = self.now_ms();
        let mut guard = Guard::new(now);
        if !ok {
            guard.report(false, now);
        }
        self.guard = Some(guard);
        self.since = Some(unix_now());
        self.restarts = 0;
        self.set_state(if ok { VpnState::Connected } else { VpnState::Reconnecting }, None);
        self.save_runtime(true);
        if capture == Capture::Tun {
            let conflicts: Vec<String> = neighbors::scan(&self.paths.core_exe).into_iter().filter(|n| n.conflicts_with_tun).map(|n| n.name).collect();
            if !conflicts.is_empty() {
                self.notice("neighbors.conflict", json!({ "names": conflicts }));
            }
        }
        Ok(())
    }

    async fn disconnect(&mut self) {
        let t0 = Instant::now();
        self.clear_proxy().await;
        let t1 = Instant::now();
        self.stop_core().await;
        let t2 = Instant::now();
        self.guard = None;
        self.since = None;
        self.restarts = 0;
        // VPN выключен, как только ядро остановлено и настройки сети возвращены. Страж Kill Switch
        // запускается следом: пока он поднимается, программы из Kill Switch держат правила pf.
        self.set_state(VpnState::Off, None);
        self.save_runtime(false);
        self.apply_killswitch().await;
        tracing::info!(
            "отключено за {} мс: прокси {} мс, ядро и DNS {} мс, Kill Switch {} мс",
            t0.elapsed().as_millis(),
            (t1 - t0).as_millis(),
            (t2 - t1).as_millis(),
            t2.elapsed().as_millis()
        );
    }

    /// Новый режим или другое подключение: ядро перезапускается с новым конфигом.
    async fn reconnect(&mut self) -> Result<(), ErrorInfo> {
        self.clear_proxy().await;
        self.stop_core().await;
        self.guard = None;
        self.connect().await
    }

    async fn start_core(&mut self, capture: Capture) -> Result<(), ErrorInfo> {
        let result = self.launch_core(capture).await;
        // macOS: запуск не удался — адаптер сразу отдаём стражу Kill Switch, а не ждём сторожа:
        // пока TUN ничей, pf не выпускает в интернет ни одну программу пользователя.
        #[cfg(unix)]
        if result.is_err() {
            self.sync_ks_core().await;
        }
        result
    }

    async fn launch_core(&mut self, capture: Capture) -> Result<(), ErrorInfo> {
        let conn_id = self.settings.active_connection.clone().ok_or_else(|| ErrorInfo::new("vpn.no_connection"))?;
        // macOS: адаптер TUN на системе один — страж Kill Switch уступает его основному ядру.
        #[cfg(unix)]
        if capture == Capture::Tun {
            self.stop_ks_core().await;
        }
        let layout = CoreLayout {
            os: Os::CURRENT,
            controller: self.profile.core_pipe.clone(),
            secret: random_secret(),
            mixed_port: self.profile.mixed_port,
            check_vpn_port: sys::free_port().map_err(internal)?,
            check_direct_port: sys::free_port().map_err(internal)?,
            tun_device: TUN_DEVICE.into(),
            log_level: self.profile.core_log_level.clone(),
            core_exe: std::fs::canonicalize(&self.paths.core_exe).unwrap_or_else(|_| self.paths.core_exe.clone()).to_string_lossy().into_owned(),
        };
        #[cfg(unix)]
        if !self.dns_overridden {
            let found = tokio::task::spawn_blocking(sys::system_dns_servers).await.unwrap_or_default();
            if !found.is_empty() {
                self.server_dns = found;
            }
        }
        let config_path = self.write_config(&layout, capture, &conn_id)?;
        self.generation += 1;
        let generation = self.generation;
        let tx = self.tx.clone();
        let api = CoreApi::new(&layout.controller, &layout.secret);
        let process = CoreProcess::start(&self.paths.core_exe, &self.paths.core_home, &config_path, api.clone(), self.profile.dev, move |code| {
            let _ = tx.try_send(Msg::CoreExited { generation, code });
        })
        .await
        .map_err(|e| {
            tracing::error!("ядро не запустилось: {e:#}");
            ErrorInfo::new("core.start_failed")
        })?;
        tracing::info!("ядро запущено (pid {:?}, {:?})", process.pid, capture);
        let traffic = self.spawn_traffic(api.clone());
        let logs = self.spawn_logs(api);
        self.running = Some(Running { process, generation, capture, layout, traffic, logs, started: Instant::now() });
        // macOS: ядро работает и без адаптера, если его не удалось создать, — тогда трафик шёл бы мимо
        // VPN, а подменённый DNS вёл бы в никуда. Это неудачный запуск: переподключение попробует снова.
        #[cfg(unix)]
        if capture == Capture::Tun && !wait_tun(Duration::from_secs(10)).await {
            tracing::error!("адаптер {TUN_DEVICE} не появился за 10 с");
            self.stop_core().await;
            return Err(ErrorInfo::new("core.start_failed"));
        }
        Ok(())
    }

    fn write_config(&self, layout: &CoreLayout, capture: Capture, conn_id: &str) -> Result<PathBuf, ErrorInfo> {
        let provider = Paths::provider_rel(conn_id);
        let sets = self.set_files();
        let cfg = compile::compile(&CompileInput {
            settings: &self.settings,
            catalog: &self.catalog,
            layout,
            capture,
            provider_path: &provider,
            sets: &sets,
            server_dns: &self.server_dns,
        });
        let path = self.paths.core_home.join("config.yaml");
        let text = serde_json::to_vec_pretty(&cfg).map_err(internal)?;
        storage::write_atomic(&path, &text).map_err(internal)?;
        Ok(path)
    }

    fn spawn_traffic(&self, api: CoreApi) -> tokio::task::JoinHandle<()> {
        let events = self.events.clone();
        tokio::spawn(async move {
            loop {
                let ev = events.clone();
                let _ = api
                    .stream("/traffic", move |v| {
                        let _ = ev.send(Event::Traffic { up: v["up"].as_u64().unwrap_or(0), down: v["down"].as_u64().unwrap_or(0) });
                    })
                    .await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    }

    /// После запуска ядра: сервер, адаптер, Kill Switch, системный прокси.
    async fn after_start(&mut self) {
        self.apply_selected_server().await;
        let Some(capture) = self.running.as_ref().map(|r| r.capture) else { return };
        if capture == Capture::Tun {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !sys::tun_up() && Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if !sys::tun_up() {
                tracing::warn!("адаптер {TUN_DEVICE} не появился за 5 с");
            }
        }
        // В обоих режимах: после TUN в фильтрах осталось бы разрешение для пропавшего адаптера.
        // macOS: в режиме «Системный прокси» TUN свободен, и его забирает страж Kill Switch.
        self.apply_killswitch().await;
        #[cfg(unix)]
        if capture == Capture::Tun {
            self.set_dns_override(true).await;
        }
        if self.settings.mode == Mode::SysProxy {
            let spec = self.proxy_spec();
            self.emit(Event::ProxyApply { host: spec.host.clone(), port: spec.port, bypass: spec.bypass.clone() });
            self.proxy_applied = true;
            // macOS: настройки сети общие и меняются с правами администратора — прокси всегда ставит служба.
            #[cfg(unix)]
            self.netconf_proxy(Some(spec)).await;
            // Окна нет — прокси пользователю ставит служба.
            #[cfg(windows)]
            if self.events.receiver_count() == 0 && self.user_proxy_saved.is_none() {
                self.user_proxy_saved = userproxy::apply(&spec);
                if let Some(saved) = &self.user_proxy_saved {
                    tracing::info!("окна нет: системный прокси пользователю поставила служба");
                    if let Ok(bytes) = serde_json::to_vec(saved) {
                        let _ = storage::write_atomic(&self.paths.data.join("user-proxy.json"), &bytes);
                    }
                }
            }
        }
    }

    /// Снять прокси, оставшийся с прошлого запуска. Отметку стираем, только когда разобрались:
    /// сняли или прокси уже не наш. Никто ещё не вошёл в Windows — отметка остаётся до следующей попытки.
    #[cfg(windows)]
    fn clear_leftover_proxy(&mut self) {
        let marker = self.paths.data.join("user-proxy.json");
        // VPN уже включили заново — прокси снова наш и нужен.
        if self.vpn != VpnState::Off || self.proxy_applied {
            return;
        }
        let Some(saved) = std::fs::read(&marker).ok().and_then(|b| serde_json::from_slice::<userproxy::Saved>(&b).ok()) else { return };
        match userproxy::clear(self.profile.mixed_port, Some(saved)) {
            userproxy::Cleared::NoUser => {}
            result => {
                if result == userproxy::Cleared::Done {
                    tracing::info!("снят оставшийся системный прокси kl!ck");
                }
                let _ = std::fs::remove_file(&marker);
            }
        }
    }

    async fn clear_proxy(&mut self) {
        if self.proxy_applied {
            self.emit(Event::ProxyClear);
            self.proxy_applied = false;
            #[cfg(unix)]
            self.netconf_proxy(None).await;
            #[cfg(windows)]
            if let Some(saved) = self.user_proxy_saved.take() {
                match userproxy::clear(self.profile.mixed_port, Some(saved.clone())) {
                    // Пользователь уже вышел (выключение компьютера) — отметка остаётся,
                    // прокси снимем после следующего входа.
                    userproxy::Cleared::NoUser => {
                        if let Ok(bytes) = serde_json::to_vec(&saved) {
                            let _ = storage::write_atomic(&self.paths.data.join("user-proxy.json"), &bytes);
                        }
                        return;
                    }
                    userproxy::Cleared::Done => tracing::info!("системный прокси пользователя сняла служба"),
                    userproxy::Cleared::NotOurs => {}
                }
                let _ = std::fs::remove_file(self.paths.data.join("user-proxy.json"));
            }
        }
    }

    async fn stop_core(&mut self) {
        // macOS: DNS вернуть до остановки ядра — пока ядро живо, запросы к подменному адресу ещё отвечаются.
        #[cfg(unix)]
        self.set_dns_override(false).await;
        if let Some(r) = self.running.take() {
            r.abort_tasks();
            r.process.stop().await;
            tracing::info!("ядро остановлено");
        }
    }

    async fn restart_core(&mut self) -> bool {
        let Some(capture) = self.running.as_ref().map(|r| r.capture) else { return false };
        self.stop_core().await;
        match self.start_core(capture).await {
            Ok(()) => {
                self.after_start().await;
                true
            }
            Err(_) => false,
        }
    }

    async fn apply_selected_server(&mut self) {
        let Some(api) = self.api() else { return };
        // Сразу после запуска или перечитывания конфига серверы провайдера бывают ещё не загружены:
        // в группе тогда только заглушка ядра COMPATIBLE. Выбирать и запоминать можно лишь настоящие.
        let deadline = Instant::now() + Duration::from_secs(5);
        let names = loop {
            let names: Vec<String> = api.provider_proxies(PROVIDER).await.unwrap_or_default().into_iter().map(|(n, _)| n).collect();
            if !names.is_empty() || Instant::now() >= deadline {
                break names;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        if names.is_empty() {
            tracing::warn!("ядро не загрузило серверы подписки за 5 с, выбранный сервер не трогаю");
            return;
        }
        let wanted = self.settings.active().and_then(|c| c.selected_server.clone());
        if let Some(name) = &wanted {
            if names.contains(name) && api.select(VPN_GROUP, name).await.is_ok() {
                return;
            }
            tracing::warn!("сохранённого сервера больше нет в подписке, беру первый");
        }
        let now = api.selected(VPN_GROUP).await.ok().flatten().filter(|n| names.contains(n));
        if let Some(now) = now.or_else(|| names.first().cloned()) {
            if let Some(c) = self.settings.active_mut() {
                c.selected_server = Some(now);
            }
            let _ = self.save();
        }
    }

    async fn probe(&self) -> bool {
        match self.api() {
            Some(api) => matches!(api.proxy_delay(VPN_GROUP, PROBE_URL, PROBE_TIMEOUT_MS).await, Ok(Some(_))),
            None => false,
        }
    }

    /// Тумблер, списки, Kill Switch: ядро перечитывает конфиг без отключения.
    async fn reload_rules(&mut self) {
        let Some((layout, capture)) = self.running.as_ref().map(|r| (r.layout.clone(), r.capture)) else { return };
        let Some(conn_id) = self.settings.active_connection.clone() else { return };
        let Some(api) = self.api() else { return };
        let path = match self.write_config(&layout, capture, &conn_id) {
            Ok(p) => p,
            Err(_) => return,
        };
        match api.reload(&path).await {
            Ok(()) => self.apply_selected_server().await,
            Err(e) => {
                tracing::warn!("ядро не перечитало конфиг ({e:#}), перезапускаю");
                if !self.restart_core().await {
                    self.set_state(VpnState::Error, Some("core.start_failed".into()));
                }
            }
        }
    }

    // ── Страж ──────────────────────────────────────────────────────────────

    async fn on_guard(&mut self) {
        let Some((_, action)) = self.guard.as_ref().map(Guard::next) else { return };
        match action {
            Action::Probe => self.spawn_probe(false),
            Action::Retry { attempt, .. } => {
                self.set_state(VpnState::Reconnecting, None);
                match attempt {
                    1 => self.spawn_probe(true),
                    2 => {
                        self.reload_rules().await;
                        self.spawn_probe(true);
                    }
                    _ => {
                        if self.restart_core().await {
                            self.spawn_probe(true);
                        } else {
                            self.probed(true, false).await;
                        }
                    }
                }
            }
            Action::GiveUp => {
                let now = self.now_ms();
                let notice = self.guard.as_mut().map(|g| g.give_up(now));
                self.after_guard(notice);
            }
        }
    }

    /// Проверка сервера — в фоне: пока ядро отвечает (до нескольких секунд), служба выполняет команды окна.
    fn spawn_probe(&mut self, retry: bool) {
        let Some((api, generation)) = self.api().zip(self.running.as_ref().map(|r| r.generation)) else { return };
        self.probing = Some(generation);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let ok = matches!(api.proxy_delay(VPN_GROUP, PROBE_URL, PROBE_TIMEOUT_MS).await, Ok(Some(_)));
            let _ = tx.send(Msg::Probed { generation, retry, ok }).await;
        });
    }

    /// Итог проверки: обычной (`retry = false`) или попытки переподключения.
    async fn probed(&mut self, retry: bool, mut ok: bool) {
        if !retry {
            if !ok && self.settings.on_server_down != ServerDownPolicy::Reconnect && self.guard.as_ref().map(Guard::health) == Some(Health::Healthy) {
                ok = self.switch_to_alternative().await;
            }
            if ok && self.running.as_ref().is_some_and(|r| r.started.elapsed() > Duration::from_secs(60)) {
                self.restarts = 0;
            }
        }
        let now = self.now_ms();
        let notice = self.guard.as_mut().and_then(|g| g.report(ok, now));
        self.after_guard(notice);
    }

    fn after_guard(&mut self, notice: Option<Notice>) {
        let Some(health) = self.guard.as_ref().map(Guard::health) else { return };
        let vpn = match health {
            Health::Healthy => VpnState::Connected,
            Health::Retrying { .. } => VpnState::Reconnecting,
            Health::Down => VpnState::ServerDown,
        };
        if vpn != self.vpn || vpn == VpnState::Reconnecting {
            self.set_state(vpn, None);
        }
        let server = self.settings.active().and_then(|c| c.selected_server.clone());
        match notice {
            Some(Notice::Down) => self.notice("vpn.down", json!({ "server": server })),
            Some(Notice::Restored) => self.notice("vpn.restored", json!({ "server": server })),
            None => {}
        }
    }

    /// «Следующий рабочий» или «самый быстрый»: проверить всех и перейти на живой сервер.
    async fn switch_to_alternative(&mut self) -> bool {
        let Some(api) = self.api() else { return false };
        let Ok(list) = api.provider_proxies(PROVIDER).await else { return false };
        let delays = api.group_delay(VPN_GROUP, PROBE_URL, PROBE_TIMEOUT_MS).await.unwrap_or_default();
        let current = self.settings.active().and_then(|c| c.selected_server.clone());
        let pick = match self.settings.on_server_down {
            ServerDownPolicy::Fastest => delays.iter().filter(|(n, _)| Some(*n) != current.as_ref()).min_by_key(|(_, d)| **d).map(|(n, _)| n.clone()),
            _ => {
                let start = current.as_ref().and_then(|c| list.iter().position(|(n, _)| n == c)).map(|i| i + 1).unwrap_or(0);
                (0..list.len()).map(|k| &list[(start + k) % list.len()].0).find(|n| delays.contains_key(*n) && Some(*n) != current.as_ref()).cloned()
            }
        };
        let Some(name) = pick else { return false };
        if api.select(VPN_GROUP, &name).await.is_err() {
            return false;
        }
        if let Some(c) = self.settings.active_mut() {
            c.selected_server = Some(name.clone());
        }
        let _ = self.save();
        self.notice("server.switched", json!({ "server": name, "reason": "down" }));
        true
    }

    async fn on_core_exit(&mut self, generation: u64, code: Option<i32>) {
        if self.running.as_ref().map(|r| r.generation) != Some(generation) {
            return;
        }
        let tail = self.running.as_ref().map(|r| r.process.tail()).unwrap_or_default();
        tracing::warn!("ядро завершилось само (код {code:?}); последние строки: {}", tail.join(" | "));
        let capture = self.running.as_ref().map(|r| r.capture).unwrap_or(Capture::Proxy);
        if let Some(r) = self.running.take() {
            r.abort_tasks();
        }
        if self.restarts < 3 {
            self.restarts += 1;
            if self.start_core(capture).await.is_ok() {
                self.after_start().await;
                self.notice("core.restarted", json!({ "attempt": self.restarts }));
                return;
            }
        }
        self.clear_proxy().await;
        self.guard = None;
        self.since = None;
        #[cfg(unix)]
        self.set_dns_override(false).await;
        self.apply_killswitch().await;
        self.set_state(VpnState::Error, Some("core.crashed".into()));
        self.notice("vpn.core_crashed", Value::Null);
    }

    async fn shutdown(&mut self) {
        self.clear_proxy().await;
        self.stop_core().await;
        #[cfg(unix)]
        self.stop_ks_core().await;
        // Рабочая служба правила pf оставляет: остановка — это обновление, перезапуск или выключение Mac,
        // и в это время прямых соединений быть не должно. Снимает их выключение Kill Switch и удаление.
        // Служба для разработки за собой убирает, иначе у разработчика пропадёт интернет.
        #[cfg(unix)]
        if self.profile.dev && self.pf_on == Some(true) {
            let data = self.paths.data.clone();
            let _ = tokio::task::spawn_blocking(move || pf::clear(&data)).await;
        }
    }

    // ── Kill Switch ────────────────────────────────────────────────────────

    /// exe включённых программ Kill Switch, по папкам.
    fn ks_scan(&self) -> Vec<(String, Vec<PathBuf>)> {
        let ks = &self.settings.kill_switch;
        if !ks.enabled {
            return Vec::new();
        }
        ks.programs.iter().filter(|p| p.enabled).map(|p| (p.folder.clone(), killswitch::scan_folder(Path::new(&p.folder)))).collect()
    }

    async fn apply_killswitch(&mut self) {
        if !self.profile.wfp_allowed {
            return;
        }
        let exes: Vec<PathBuf> = self.ks_scan().into_iter().flat_map(|(_, e)| e).collect();
        self.ks_applied = exes.clone();
        #[cfg(unix)]
        self.sync_ks_core().await;
        #[cfg(windows)]
        self.apply_wfp(&exes);
    }

    #[cfg(windows)]
    fn apply_wfp(&mut self, exes: &[PathBuf]) {
        let tun = self.running.as_ref().filter(|r| r.capture == Capture::Tun).and_then(|_| crate::win::interface_luid(TUN_DEVICE));
        match Wfp::open().and_then(|w| w.apply(exes, tun)) {
            Ok(r) => {
                tracing::info!("Kill Switch: защищено exe {}, снято старых фильтров {}, адаптер {}", r.protected, r.removed, if tun.is_some() { "есть" } else { "нет" });
                if !r.failed.is_empty() {
                    self.notice("killswitch.partial", json!({ "failed": r.failed.len() }));
                }
            }
            Err(e) => {
                tracing::error!("Kill Switch не применился: {e:#}");
                self.notice("killswitch.failed", Value::Null);
            }
        }
    }

    /// Новые exe в папке программы (обновилась) — закрыть и их; папка пуста — предупредить один раз.
    async fn check_killswitch_programs(&mut self) {
        let scan = self.ks_scan();
        let mut missing = Vec::new();
        for (folder, exes) in &scan {
            if exes.is_empty() {
                if self.warned.insert((folder.clone(), "ks_missing")) {
                    missing.push(folder.clone());
                }
            } else {
                self.warned.remove(&(folder.clone(), "ks_missing"));
            }
        }
        for folder in missing {
            tracing::warn!("Kill Switch: в папке программы нет exe");
            self.notice("killswitch.missing", json!({ "folder": folder }));
        }
        let exes: Vec<PathBuf> = scan.into_iter().flat_map(|(_, e)| e).collect();
        if self.profile.wfp_allowed && exes != self.ks_applied {
            tracing::info!("Kill Switch: состав exe изменился, применяю заново");
            self.apply_killswitch().await;
        }
    }

    async fn killswitch_changed(&mut self) -> Result<Value, ErrorInfo> {
        self.save()?;
        self.apply_killswitch().await;
        self.reload_rules().await;
        self.emit_state();
        serde_json::to_value(&self.settings.kill_switch).map_err(internal)
    }

    // ── macOS: системный прокси, DNS и страж Kill Switch ───────────────────

    /// Трогать настройки сети можно рабочей службе и службе для разработки, запущенной через sudo.
    #[cfg(unix)]
    fn netconf_allowed(&self) -> bool {
        !self.profile.dev || sys::is_elevated()
    }

    /// Поставить (`Some`) или снять (`None`) системный прокси всем сетевым службам Mac.
    #[cfg(unix)]
    async fn netconf_proxy(&self, spec: Option<SystemProxy>) {
        if !self.netconf_allowed() {
            return;
        }
        let marker = self.paths.data.join(netconf::PROXY_MARKER);
        let on = spec.is_some();
        let ok = tokio::task::spawn_blocking(move || match spec {
            Some(s) => netconf::proxy_apply(&s, &marker),
            None => netconf::proxy_clear(&marker),
        })
        .await
        .unwrap_or(false);
        match (on, ok) {
            (true, true) => tracing::info!("системный прокси поставлен"),
            (true, false) => tracing::warn!("системный прокси не поставился: нет включённых сетевых служб или настройки не записались"),
            (false, true) => tracing::info!("системный прокси снят"),
            (false, false) => {}
        }
    }

    /// Режим VPN (TUN): DNS сетевых служб — на адрес внутри адаптера, иначе DNS из локальной сети
    /// (роутер) спрашивается мимо VPN. При отключении — вернуть прежний.
    #[cfg(unix)]
    async fn set_dns_override(&mut self, on: bool) {
        if on == self.dns_overridden || !self.netconf_allowed() {
            return;
        }
        self.dns_overridden = on;
        let marker = self.paths.data.join(netconf::DNS_MARKER);
        let ok = tokio::task::spawn_blocking(move || if on { netconf::dns_apply(&marker) } else { netconf::dns_restore(&marker) })
            .await
            .unwrap_or(false);
        match (on, ok) {
            (true, true) => tracing::info!("DNS на время VPN: {}", netconf::DNS_ADDR),
            (true, false) => tracing::warn!("DNS не подменился: нет включённых сетевых служб или настройки не записались"),
            (false, true) => tracing::info!("DNS возвращён"),
            (false, false) => {}
        }
    }

    /// Сеть изменилась: новым сетевым службам — тот же прокси и DNS.
    #[cfg(unix)]
    async fn refresh_netconf(&mut self) {
        if !self.netconf_allowed() {
            return;
        }
        if self.proxy_applied {
            self.netconf_proxy(Some(self.proxy_spec())).await;
        }
        if self.dns_overridden {
            let marker = self.paths.data.join(netconf::DNS_MARKER);
            let _ = tokio::task::spawn_blocking(move || netconf::dns_apply(&marker)).await;
        }
        // Прошлый раз вернуть настройки не удалось (настройки сети были заняты) — пробуем снова при смене сети.
        let proxy_left = self.paths.data.join(netconf::PROXY_MARKER);
        if !self.proxy_applied && netconf::has_leftover(&proxy_left) {
            let _ = tokio::task::spawn_blocking(move || netconf::proxy_clear(&proxy_left)).await;
        }
        let dns_left = self.paths.data.join(netconf::DNS_MARKER);
        if !self.dns_overridden && netconf::has_leftover(&dns_left) {
            let _ = tokio::task::spawn_blocking(move || netconf::dns_restore(&dns_left)).await;
        }
    }

    /// Какие программы закрывает страж: все включённые программы Kill Switch — и те, которых сейчас
    /// нет на диске. Программа на внешнем диске или посреди обновления пропадает ненадолго, и снять
    /// на это время защиту значило бы выпустить её напрямую, как только она появится снова.
    #[cfg(unix)]
    fn ks_folders(&self) -> Vec<String> {
        let ks = &self.settings.kill_switch;
        if !ks.enabled {
            return Vec::new();
        }
        ks.programs.iter().filter(|p| p.enabled).map(|p| p.folder.clone()).collect()
    }

    /// Правила Kill Switch в pf: стоят, пока в Kill Switch есть программы, при любом состоянии VPN.
    #[cfg(unix)]
    async fn sync_pf(&mut self, want: bool) {
        if self.pf_on == Some(want) {
            return;
        }
        let data = self.paths.data.clone();
        if want {
            match tokio::task::spawn_blocking(move || pf::apply(&data)).await.map_err(anyhow::Error::from).and_then(|r| r) {
                Ok(()) => {
                    tracing::info!("Kill Switch: правила pf стоят — мимо адаптера ядра программы в интернет не выходят");
                    self.pf_on = Some(true);
                }
                Err(e) => {
                    tracing::error!("Kill Switch: правила pf не поставились: {e:#}");
                    // Сторож попробует снова через 10 с; уведомление — один раз, а не на каждую попытку.
                    if self.pf_on != Some(false) {
                        self.notice("killswitch.failed", Value::Null);
                    }
                    self.pf_on = Some(false);
                }
            }
        } else {
            if tokio::task::spawn_blocking(move || pf::clear(&data)).await.unwrap_or(false) {
                tracing::info!("Kill Switch: правила pf сняты");
            }
            self.pf_on = Some(false);
        }
    }

    /// Раз в 10 с: правила pf на месте (их могла сбросить другая программа), страж работает.
    #[cfg(unix)]
    async fn ks_watchdog(&mut self) {
        if !self.profile.wfp_allowed {
            return;
        }
        if self.pf_on == Some(true) && !tokio::task::spawn_blocking(pf::active).await.unwrap_or(false) {
            tracing::warn!("Kill Switch: правила pf пропали, ставлю снова");
            self.pf_on = None;
        }
        if !self.ks_retry_pending {
            self.sync_ks_core().await;
        }
    }

    /// Страж нужен, когда есть что закрывать, а TUN не держит основное ядро: VPN выключен
    /// или включён в режиме «Системный прокси». Состав программ меняется без перезапуска стража.
    #[cfg(unix)]
    async fn sync_ks_core(&mut self) {
        let active = self.ks_folders();
        self.sync_pf(!active.is_empty()).await;
        let main_tun = self.running.as_ref().is_some_and(|r| r.capture == Capture::Tun);
        let folders = if main_tun { Vec::new() } else { active };
        if folders.is_empty() {
            self.stop_ks_core().await;
            return;
        }
        if let Some(k) = &self.ks_core {
            if k.folders == folders {
                return;
            }
            let (api, layout) = (k.process.api.clone(), k.layout.clone());
            if let Ok(path) = self.write_guard_config(&layout, &folders) {
                if api.reload(&path).await.is_ok() {
                    tracing::info!("Kill Switch: страж закрывает программ {}", folders.len());
                    if let Some(k) = self.ks_core.as_mut() {
                        k.folders = folders;
                    }
                    return;
                }
            }
            self.stop_ks_core().await;
        }
        match self.start_ks_core(folders).await {
            Ok(n) => tracing::info!("Kill Switch: страж запущен, программ {n}"),
            Err(e) => {
                tracing::error!("Kill Switch: страж не запустился: {e:#}");
                self.ks_failed();
            }
        }
    }

    /// Страж не запустился или упал. Пока его нет, прямые соединения закрывает pf, поэтому
    /// страж не бросаем: пробуем снова через 1, 2, 5, 10 с, дальше раз в 30 с. После третьей
    /// неудачи подряд — уведомление: без стража интернета нет у всех программ пользователя.
    #[cfg(unix)]
    fn ks_failed(&mut self) {
        const BACKOFF: [u64; 5] = [1, 2, 5, 10, 30];
        self.ks_failures = self.ks_failures.saturating_add(1);
        if self.ks_failures == 3 {
            self.notice("killswitch.failed", Value::Null);
        }
        if self.ks_retry_pending {
            return;
        }
        self.ks_retry_pending = true;
        let delay = BACKOFF[(self.ks_failures as usize - 1).min(BACKOFF.len() - 1)];
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(delay)).await;
            let _ = tx.send(Msg::KsRetry).await;
        });
    }

    #[cfg(unix)]
    fn write_guard_config(&self, layout: &CoreLayout, folders: &[String]) -> anyhow::Result<PathBuf> {
        let mut settings = self.settings.clone();
        settings.kill_switch.enabled = true;
        settings.kill_switch.programs = folders.iter().map(|f| KsProgram { folder: f.clone(), enabled: true }).collect();
        let cfg = compile::compile_guard(&settings, layout);
        let home = self.paths.core_home.join("guard");
        std::fs::create_dir_all(&home)?;
        let path = home.join("config.yaml");
        storage::write_atomic(&path, &serde_json::to_vec_pretty(&cfg)?)?;
        Ok(path)
    }

    #[cfg(unix)]
    async fn start_ks_core(&mut self, folders: Vec<String>) -> anyhow::Result<usize> {
        let layout = CoreLayout {
            os: Os::CURRENT,
            controller: self.profile.guard_pipe.clone(),
            secret: random_secret(),
            mixed_port: 0,
            check_vpn_port: 0,
            check_direct_port: 0,
            tun_device: TUN_DEVICE.into(),
            log_level: self.profile.core_log_level.clone(),
            // У стража нет своих соединений, которые надо было бы отпускать мимо: правило не нужно.
            core_exe: String::new(),
        };
        let path = self.write_guard_config(&layout, &folders)?;
        let home = self.paths.core_home.join("guard");
        self.ks_generation += 1;
        let generation = self.ks_generation;
        let tx = self.tx.clone();
        let api = CoreApi::new(&layout.controller, &layout.secret);
        let process = CoreProcess::start(&self.paths.core_exe, &home, &path, api, self.profile.dev, move |_| {
            let _ = tx.try_send(Msg::KsCoreExited { generation });
        })
        .await?;
        let n = folders.len();
        self.ks_core = Some(KsCore { process, generation, layout, folders, started: Instant::now() });
        Ok(n)
    }

    #[cfg(unix)]
    async fn stop_ks_core(&mut self) {
        if let Some(k) = self.ks_core.take() {
            k.process.stop().await;
            tracing::info!("Kill Switch: страж остановлен");
        }
    }

    /// Страж упал сам. Первый раз — сразу запустить снова, дальше — с паузами (`ks_failed`).
    #[cfg(unix)]
    async fn on_ks_core_exit(&mut self, generation: u64) {
        let Some(k) = self.ks_core.take_if(|k| k.generation == generation) else { return };
        tracing::warn!("Kill Switch: страж завершился сам; последние строки: {}", k.process.tail().join(" | "));
        if k.started.elapsed() > Duration::from_secs(60) {
            self.ks_failures = 0;
        }
        drop(k);
        if self.ks_failures == 0 {
            self.ks_failures = 1;
            self.sync_ks_core().await;
        } else {
            self.ks_failed();
        }
    }

    // ── Списки ─────────────────────────────────────────────────────────────

    fn validate_rule(&self, rule: Rule) -> Result<Rule, ErrorInfo> {
        let target = match rule.target {
            Target::Domain(d) => Target::Domain(normalize_domain(&d).map_err(|e| ErrorInfo::new(e.code()))?),
            Target::Ip(c) => Target::Ip(normalize_cidr(&c).map_err(|e| ErrorInfo::new(e.code()))?),
            Target::Program(f) => Target::Program(program_folder(&f).map_err(|e| ErrorInfo::new(e.code()))?),
            Target::Service(id) => {
                if self.catalog.find(&id).is_none() {
                    return Err(ErrorInfo::with("input.unknown_service", json!({ "id": id })));
                }
                Target::Service(id)
            }
        };
        Ok(Rule { target, route: rule.route, enabled: rule.enabled })
    }

    async fn lists_changed(&mut self, position: Routing) -> Result<Value, ErrorInfo> {
        self.save()?;
        if position == self.settings.routing {
            self.reload_rules().await;
        }
        serde_json::to_value(self.settings.lists.get(position)).map_err(internal)
    }

    // ── Подключения ────────────────────────────────────────────────────────

    fn find_connection(&self, source: &str) -> Option<&Connection> {
        added_by_source(&self.settings.connections, &self.secrets.0, source)
    }

    async fn add_connection(&mut self, source: String, name: Option<String>) -> Result<Value, ErrorInfo> {
        let source = source.trim().to_string();
        // Та же ссылка второй раз — не дубль, а «Уже добавлено»: окно откроет это подключение.
        if let Some(c) = self.find_connection(&source) {
            return Err(ErrorInfo::with("conn.exists", json!({ "id": c.id, "name": c.name })));
        }
        let kind = subparse::detect_source(&source).ok_or_else(|| ErrorInfo::new("input.unknown_format"))?;
        let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        let conn = match kind {
            SourceKind::Subscription => {
                let f = subs::fetch(&source, self.vpn_port()).await?;
                tracing::info!("подписка скачана: {:?}, срок и трафик {}", f.content, if f.info.is_some() { "есть" } else { "нет" });
                self.write_provider(&id, &f.body)?;
                let host = source.split_once("://").and_then(|(_, r)| r.split(['/', '?', ':']).next()).unwrap_or("").to_string();
                Connection {
                    id: id.clone(),
                    name: name.or(f.title).unwrap_or(host),
                    kind: ConnectionKind::Subscription,
                    info: f.info,
                    updated_at: Some(unix_now()),
                    update_interval_hours: f.interval_hours,
                    selected_server: None,
                }
            }
            SourceKind::Link => {
                self.write_provider(&id, &format!("{source}\n"))?;
                Connection {
                    id: id.clone(),
                    name: name.or_else(|| subparse::link_name(&source)).unwrap_or_else(|| "Сервер".into()),
                    kind: ConnectionKind::Link,
                    info: None,
                    updated_at: Some(unix_now()),
                    update_interval_hours: None,
                    selected_server: None,
                }
            }
        };
        self.secrets.0.insert(id.clone(), source);
        self.secrets.save(&self.paths).map_err(internal)?;
        self.push_connection(conn)
    }

    async fn import_file(&mut self, file_name: String, content: String) -> Result<Value, ErrorInfo> {
        match subs::check(&content)? {
            Content::ClashYaml | Content::Links(_) | Content::Base64Links(_) => {}
            _ => return Err(ErrorInfo::new("sub.unknown_format")),
        }
        let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        self.write_provider(&id, &content)?;
        let stem = Path::new(&file_name).file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty());
        let conn = Connection {
            id,
            name: stem.unwrap_or_else(|| "Файл".into()),
            kind: ConnectionKind::File,
            info: None,
            updated_at: Some(unix_now()),
            update_interval_hours: None,
            selected_server: None,
        };
        self.push_connection(conn)
    }

    fn push_connection(&mut self, conn: Connection) -> Result<Value, ErrorInfo> {
        let view = ConnectionView { id: conn.id.clone(), name: conn.name.clone(), info: conn.info.clone() };
        if self.settings.active_connection.is_none() {
            self.settings.active_connection = Some(conn.id.clone());
        }
        self.settings.connections.push(conn);
        self.save()?;
        self.emit_state();
        serde_json::to_value(view).map_err(internal)
    }

    fn write_provider(&self, id: &str, body: &str) -> Result<(), ErrorInfo> {
        storage::write_atomic(&self.paths.provider_file(id), body.as_bytes()).map_err(internal)
    }

    /// Скачать подписку заново. Ошибка панели не трогает рабочий список серверов.
    async fn download(&mut self, id: &str) -> Result<(), ErrorInfo> {
        let conn = self.settings.connections.iter().find(|c| c.id == id).cloned().ok_or_else(|| ErrorInfo::new("conn.not_found"))?;
        match conn.kind {
            ConnectionKind::Subscription => {
                let url = self.secrets.0.get(id).cloned().ok_or_else(|| ErrorInfo::new("conn.secret_missing"))?;
                let f = subs::fetch(&url, self.vpn_port()).await?;
                self.write_provider(id, &f.body)?;
                if let Some(c) = self.settings.connections.iter_mut().find(|c| c.id == id) {
                    c.info = f.info.or(c.info.take());
                    c.updated_at = Some(unix_now());
                    c.update_interval_hours = f.interval_hours.or(c.update_interval_hours);
                }
                self.servers.remove(id);
                self.save()
            }
            ConnectionKind::Link => {
                let link = self.secrets.0.get(id).cloned().ok_or_else(|| ErrorInfo::new("conn.secret_missing"))?;
                self.write_provider(id, &format!("{link}\n"))
            }
            ConnectionKind::File => {
                if self.paths.provider_file(id).exists() {
                    Ok(())
                } else {
                    Err(ErrorInfo::new("conn.file_missing"))
                }
            }
        }
    }

    async fn refresh(&mut self, id: &str) -> Result<Value, ErrorInfo> {
        self.download(id).await?;
        if self.settings.active_connection.as_deref() == Some(id) {
            if let Some(api) = self.api() {
                if let Err(e) = api.refresh_provider(PROVIDER).await {
                    tracing::warn!("ядро не перечитало серверы: {e:#}");
                }
                self.apply_selected_server().await;
            }
        }
        self.emit_state();
        let c = self.settings.connections.iter().find(|c| c.id == id).ok_or_else(|| ErrorInfo::new("conn.not_found"))?;
        serde_json::to_value(ConnectionView { id: c.id.clone(), name: c.name.clone(), info: c.info.clone() }).map_err(internal)
    }

    async fn remove(&mut self, id: &str) -> Result<Value, ErrorInfo> {
        if !self.settings.connections.iter().any(|c| c.id == id) {
            return Err(ErrorInfo::new("conn.not_found"));
        }
        let active = self.settings.active_connection.as_deref() == Some(id);
        if active && self.running.is_some() {
            self.disconnect().await;
        }
        self.settings.connections.retain(|c| c.id != id);
        if active {
            self.settings.active_connection = self.settings.connections.first().map(|c| c.id.clone());
        }
        self.secrets.0.remove(id);
        self.secrets.save(&self.paths).map_err(internal)?;
        let _ = std::fs::remove_file(self.paths.provider_file(id));
        self.servers.remove(id);
        self.save()?;
        self.emit_state();
        Ok(self.view_json())
    }

    async fn select_connection(&mut self, id: &str) -> Result<Value, ErrorInfo> {
        if !self.settings.connections.iter().any(|c| c.id == id) {
            return Err(ErrorInfo::new("conn.not_found"));
        }
        if self.settings.active_connection.as_deref() != Some(id) {
            self.settings.active_connection = Some(id.to_string());
            self.save()?;
            if self.running.is_some() {
                self.reconnect().await?;
            } else {
                self.emit_state();
            }
        }
        Ok(self.view_json())
    }

    // ── Серверы ────────────────────────────────────────────────────────────

    /// Список серверов активного подключения. `measure` — заодно проверить задержку.
    /// При выключенном VPN служба на пару секунд запускает проверочное ядро без перехвата.
    async fn list_servers(&mut self, measure: bool) -> Result<Value, ErrorInfo> {
        let conn = self.settings.active().cloned().ok_or_else(|| ErrorInfo::new("vpn.no_connection"))?;
        if !measure && self.running.is_none() {
            if let Some(cached) = self.servers.get(&conn.id) {
                return serde_json::to_value(cached).map_err(internal);
            }
        }
        if !self.paths.provider_file(&conn.id).exists() {
            self.download(&conn.id).await?;
        }
        let (list, delays, now) = match self.api() {
            Some(api) => {
                let list = api.provider_proxies(PROVIDER).await.map_err(internal)?;
                let delays = if measure { api.group_delay(VPN_GROUP, PROBE_URL, PROBE_TIMEOUT_MS).await.unwrap_or_default() } else { HashMap::new() };
                let now = api.selected(VPN_GROUP).await.ok().flatten();
                (list, delays, now)
            }
            None => {
                let (list, delays) = self.tester(&conn.id, measure).await?;
                (list, delays, None)
            }
        };
        let selected = now.or(conn.selected_server.clone());
        let old = self.servers.get(&conn.id).cloned().unwrap_or_default();
        let views: Vec<ServerView> = list
            .into_iter()
            .map(|(name, kind)| {
                let delay = if measure { delays.get(&name).copied() } else { old.iter().find(|s| s.name == name).and_then(|s| s.delay) };
                ServerView { selected: selected.as_deref() == Some(name.as_str()), name, kind, delay }
            })
            .collect();
        self.servers.insert(conn.id.clone(), views.clone());
        serde_json::to_value(views).map_err(internal)
    }

    async fn tester(&self, conn_id: &str, measure: bool) -> Result<(Vec<(String, String)>, HashMap<String, u32>), ErrorInfo> {
        let layout = CoreLayout {
            os: Os::CURRENT,
            controller: self.profile.tester_pipe.clone(),
            secret: random_secret(),
            mixed_port: 0,
            check_vpn_port: 0,
            check_direct_port: 0,
            tun_device: TUN_DEVICE.into(),
            log_level: "warning".into(),
            core_exe: String::new(),
        };
        let cfg = compile::compile_tester(&layout, &Paths::provider_rel(conn_id));
        let path = self.paths.core_home.join("tester.yaml");
        storage::write_atomic(&path, &serde_json::to_vec_pretty(&cfg).map_err(internal)?).map_err(internal)?;
        let api = CoreApi::new(&layout.controller, &layout.secret);
        let process = CoreProcess::start(&self.paths.core_exe, &self.paths.core_home, &path, api.clone(), false, |_| {}).await.map_err(|e| {
            tracing::error!("проверочное ядро не запустилось: {e:#}");
            ErrorInfo::new("core.start_failed")
        })?;
        // Ядро отвечает раньше, чем дочитает файл подписки: первые запросы бывают с пустым списком.
        let mut list = api.provider_proxies(PROVIDER).await;
        for _ in 0..30 {
            if matches!(&list, Ok(l) if !l.is_empty()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            list = api.provider_proxies(PROVIDER).await;
        }
        let delays = if measure { api.group_delay(VPN_GROUP, PROBE_URL, PROBE_TIMEOUT_MS).await.unwrap_or_default() } else { HashMap::new() };
        process.stop().await;
        let list = list.map_err(|e| {
            tracing::warn!("проверочное ядро не отдало серверы: {e:#}");
            ErrorInfo::new("sub.no_servers")
        })?;
        if list.is_empty() {
            return Err(ErrorInfo::new("sub.no_servers"));
        }
        Ok((list, delays))
    }

    async fn select_server(&mut self, name: String) -> Result<Value, ErrorInfo> {
        let conn_id = self.settings.active_connection.clone().ok_or_else(|| ErrorInfo::new("vpn.no_connection"))?;
        if let Some(api) = self.api() {
            api.select(VPN_GROUP, &name).await.map_err(|e| {
                tracing::warn!("сервер не выбран: {e:#}");
                ErrorInfo::new("server.not_found")
            })?;
        } else if let Some(list) = self.servers.get(&conn_id) {
            if !list.iter().any(|s| s.name == name) {
                return Err(ErrorInfo::new("server.not_found"));
            }
        }
        if let Some(c) = self.settings.active_mut() {
            c.selected_server = Some(name.clone());
        }
        if let Some(list) = self.servers.get_mut(&conn_id) {
            for s in list.iter_mut() {
                s.selected = s.name == name;
            }
        }
        self.save()?;
        if self.running.is_some() {
            self.notice("server.switched", json!({ "server": name, "reason": "user" }));
        }
        self.emit_state();
        Ok(self.view_json())
    }
}

/// Подключение, добавленное по этой же ссылке. Ссылки сравниваются как есть: токен в них точный.
fn added_by_source<'a>(connections: &'a [Connection], secrets: &HashMap<String, String>, source: &str) -> Option<&'a Connection> {
    let source = source.trim();
    connections.iter().find(|c| secrets.get(&c.id).is_some_and(|s| s.trim() == source))
}

#[cfg(test)]
mod tests {
    use super::{added_by_source, version_newer};
    use klick_core::{Connection, ConnectionKind};
    use std::collections::HashMap;

    #[test]
    fn same_link_is_found_not_added_twice() {
        let conn = |id: &str| Connection {
            id: id.into(),
            name: id.into(),
            kind: ConnectionKind::Subscription,
            info: None,
            updated_at: None,
            update_interval_hours: None,
            selected_server: None,
        };
        let connections = [conn("a"), conn("b")];
        let secrets = HashMap::from([
            ("a".to_string(), "https://sub.example.com/s/one".to_string()),
            ("b".to_string(), "https://sub.example.com/s/two\n".to_string()),
            // Ссылка удалённого подключения, если бы осталась, ничего не находит.
            ("gone".to_string(), "https://sub.example.com/s/gone".to_string()),
        ]);
        let found = |s: &str| added_by_source(&connections, &secrets, s).map(|c| c.id.as_str());
        assert_eq!(found("https://sub.example.com/s/one"), Some("a"));
        assert_eq!(found("  https://sub.example.com/s/two "), Some("b"));
        assert_eq!(found("https://sub.example.com/s/gone"), None);
        // Токен другой — это другая подписка.
        assert_eq!(found("https://sub.example.com/s/ONE"), None);
        assert_eq!(found("https://sub.example.com/s/on"), None);
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(version_newer("0.4.10", "0.4.9"));
        assert!(version_newer("1.0.0", "0.4.0"));
        assert!(!version_newer("0.3.0", "0.4.0"));
        assert!(!version_newer("0.4.0", "0.4.0"));
        assert!(version_newer("0.4.1-beta", "0.4.0"));
    }
}
