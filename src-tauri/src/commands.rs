//! Команды окна. Всё, что может задуматься (mihomo, PowerShell, сеть),
//! объявлено async — идёт не в главном потоке.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::{now_secs, AppState, Kind, Mode, Profile, Settings, SubInfo};
use crate::{autostart, core, killswitch, links, procs, sub};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    name: String,
    proto: String,
    host: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    id: String,
    kind: Kind,
    name: String,
    has_url: bool,
    servers: Vec<ServerView>,
    active: Option<String>,
    info: Option<SubInfo>,
    updated_at: Option<u64>,
}

fn host_of(p: &Value) -> String {
    let s = p.get("server").and_then(Value::as_str).unwrap_or("");
    match p.get("port") {
        Some(port) => format!("{s}:{}", port.as_u64().map(|n| n.to_string()).unwrap_or_else(|| port.to_string())),
        None => s.to_string(),
    }
}

fn view(p: &Profile) -> ProfileView {
    ProfileView {
        id: p.id.clone(),
        kind: p.kind,
        name: p.name.clone(),
        has_url: p.url.is_some(),
        servers: p
            .proxies
            .iter()
            .map(|v| ServerView {
                name: v.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                proto: links::proto_label(v),
                host: host_of(v),
            })
            .collect(),
        active: p.active_proxy().and_then(|v| v.get("name")).and_then(Value::as_str).map(String::from),
        info: p.info.clone(),
        updated_at: p.updated_at,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    status: core::Status,
    traffic: core::Traffic,
    settings: Settings,
    profiles: Vec<ProfileView>,
    active_profile: Option<String>,
    autostart: bool,
    app_version: String,
    mihomo_version: Option<String>,
    kill_switch_issue: Option<String>,
}

static MIHOMO_VERSION: once_cell::sync::OnceCell<Option<String>> = once_cell::sync::OnceCell::new();

/// Версия ядра — один раз за запуск: `mihomo -v` — это отдельный процесс.
pub fn mihomo_version(app: &AppHandle) -> Option<String> {
    MIHOMO_VERSION.get_or_init(|| core::mihomo_version(app)).clone()
}

#[tauri::command(async)]
pub fn get_overview(app: AppHandle, state: State<AppState>) -> Overview {
    let settings = state.settings.lock().unwrap().clone();
    let profiles: Vec<ProfileView> = state.vault.lock().unwrap().profiles.iter().map(view).collect();
    Overview {
        status: core::status(),
        traffic: core::traffic(),
        active_profile: state.active_profile().map(|p| p.id),
        settings,
        profiles,
        autostart: autostart::is_enabled(),
        app_version: app.package_info().version.to_string(),
        mihomo_version: MIHOMO_VERSION.get_or_init(|| core::mihomo_version(&app)).clone(),
        kill_switch_issue: killswitch::issue(),
    }
}

// ─────────── Подключение ───────────

#[tauri::command(async)]
pub fn connect(app: AppHandle) -> Result<(), String> {
    core::connect(&app)
}

#[tauri::command(async)]
pub fn disconnect(app: AppHandle) {
    core::disconnect(&app);
}

#[tauri::command(async)]
pub fn select_profile(app: AppHandle, id: String) -> Result<(), String> {
    core::select_profile(&app, &id)
}

#[tauri::command(async)]
pub fn select_server(app: AppHandle, profile_id: String, name: String) -> Result<(), String> {
    core::select_server(&app, &profile_id, &name)
}

#[tauri::command(async)]
pub fn ping(app: AppHandle, profile_id: String) -> Result<HashMap<String, Option<u32>>, String> {
    let r = core::ping_profile(&app, &profile_id)?;
    let ok = r.values().filter(|v| v.is_some()).count();
    core::note("INFO", &format!("Проверка задержки: ответили {ok} из {}", r.len()));
    Ok(r)
}

// ─────────── Подключения (профили) ───────────

fn after_profiles_changed(app: &AppHandle, state: &AppState) {
    {
        let ids: Vec<String> = state.vault.lock().unwrap().profiles.iter().map(|p| p.id.clone()).collect();
        let mut s = state.settings.lock().unwrap();
        if !s.active_profile.as_ref().map(|a| ids.contains(a)).unwrap_or(false) {
            s.active_profile = ids.first().cloned();
        }
    }
    state.save_settings();
    let _ = app.emit("profiles-changed", ());
    crate::tray::refresh(app);
}

fn host_name(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    rest.split(['/', '?', '#', ':']).next().unwrap_or(rest).to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    id: String,
    kind: Kind,
    servers: usize,
}

/// Ссылка на подписку, одна или несколько ссылок на серверы, конфиг.
#[tauri::command(async)]
pub fn add_link(app: AppHandle, state: State<AppState>, input: String, name: String) -> Result<Added, String> {
    let text = input.trim();
    let name = name.trim();
    let id = crate::sys::random_hex(6);
    let profile = if (text.starts_with("https://") || text.starts_with("http://")) && !text.contains(char::is_whitespace) {
        let f = sub::fetch(text)?;
        let first = f.proxies.first().and_then(|p| p.get("name")).and_then(Value::as_str).map(String::from);
        Profile {
            id: id.clone(),
            kind: Kind::Sub,
            name: if !name.is_empty() { name.to_string() } else { f.title.unwrap_or_else(|| format!("Подписка {}", host_name(text))) },
            url: Some(text.to_string()),
            proxies: f.proxies,
            active: first,
            info: f.info,
            updated_at: Some(now_secs()),
            update_hours: f.update_hours,
        }
    } else {
        let proxies = links::parse_any(text)?;
        let single = proxies.len() == 1 && links::is_link(text);
        let first = proxies[0].get("name").and_then(Value::as_str).unwrap_or("сервер").to_string();
        Profile {
            id: id.clone(),
            kind: if single { Kind::Single } else { Kind::File },
            name: if !name.is_empty() { name.to_string() } else if single { first.clone() } else { format!("Импорт · {} серв.", proxies.len()) },
            url: single.then(|| text.to_string()),
            active: Some(first),
            proxies,
            info: None,
            updated_at: Some(now_secs()),
            update_hours: None,
        }
    };
    let added = Added { id: id.clone(), kind: profile.kind, servers: profile.proxies.len() };
    core::note("INFO", &format!("Добавлено подключение: {} ({} серв.)", profile.name, added.servers));
    state.vault.lock().unwrap().profiles.push(profile);
    state.save_vault()?;
    state.settings.lock().unwrap().active_profile = Some(id.clone());
    after_profiles_changed(&app, &state);
    Ok(added)
}

#[tauri::command(async)]
pub fn add_file(app: AppHandle, state: State<AppState>, path: String) -> Result<Added, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("файл не читается: {e}"))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let file = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
    let proxies = links::parse_any(&text)?;
    let id = crate::sys::random_hex(6);
    let first = proxies[0].get("name").and_then(Value::as_str).map(String::from);
    let n = proxies.len();
    core::note("INFO", &format!("Импортирован {file}: {n} серв."));
    state.vault.lock().unwrap().profiles.push(Profile {
        id: id.clone(),
        kind: Kind::File,
        name: file,
        url: None,
        proxies,
        active: first,
        info: None,
        updated_at: Some(now_secs()),
        update_hours: None,
    });
    state.save_vault()?;
    state.settings.lock().unwrap().active_profile = Some(id.clone());
    after_profiles_changed(&app, &state);
    Ok(Added { id, kind: Kind::File, servers: n })
}

/// Обновить подписку. Выбранный сервер сохраняется, если он ещё есть.
pub fn refresh_inner(app: &AppHandle, id: &str) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let url = state
        .vault
        .lock()
        .unwrap()
        .profiles
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| if p.kind == Kind::Sub { p.url.clone() } else { None })
        .ok_or("Это не подписка.")?;
    let f = match sub::fetch(&url) {
        Ok(f) => f,
        Err(e) => {
            core::note("WARN", &format!("Подписка не обновилась: {e}"));
            return Err(e);
        }
    };
    let n = f.proxies.len();
    {
        let mut v = state.vault.lock().unwrap();
        if let Some(p) = v.profiles.iter_mut().find(|p| p.id == id) {
            let keep = p.active.clone().filter(|a| f.proxies.iter().any(|x| x.get("name").and_then(Value::as_str) == Some(a)));
            p.active = keep.or_else(|| f.proxies.first().and_then(|x| x.get("name")).and_then(Value::as_str).map(String::from));
            p.proxies = f.proxies;
            if f.info.is_some() {
                p.info = f.info;
            }
            if f.update_hours.is_some() {
                p.update_hours = f.update_hours;
            }
            p.updated_at = Some(now_secs());
        }
    }
    state.save_vault()?;
    core::note("INFO", &format!("Подписка обновлена: {n} серв."));
    // Работает на этой подписке — подхватить новый список без переподключения.
    if core::is_on() && core::status().profile_id.as_deref() == Some(id) {
        if let Err(e) = core::apply_config(app) {
            core::note("WARN", &e);
        }
    }
    after_profiles_changed(app, &state);
    Ok(n)
}

#[tauri::command(async)]
pub fn refresh_sub(app: AppHandle, id: String) -> Result<usize, String> {
    refresh_inner(&app, &id)
}

/// Подписки, которым пора обновиться: по profile-update-interval или раз в 12 часов.
pub fn refresh_due(app: &AppHandle) {
    if !app.state::<AppState>().settings.lock().unwrap().auto_update {
        return;
    }
    let due: Vec<String> = app
        .state::<AppState>()
        .vault
        .lock()
        .unwrap()
        .profiles
        .iter()
        .filter(|p| p.kind == Kind::Sub)
        .filter(|p| {
            let every = p.update_hours.unwrap_or(12).clamp(1, 24 * 7) as u64 * 3600;
            p.updated_at.map(|t| now_secs().saturating_sub(t) >= every).unwrap_or(true)
        })
        .map(|p| p.id.clone())
        .collect();
    for id in due {
        let _ = refresh_inner(app, &id);
    }
}

#[derive(Serialize)]
pub struct Removed {
    profile: Profile,
    index: usize,
    was_live: bool,
}

#[tauri::command(async)]
pub fn remove_profile(app: AppHandle, state: State<AppState>, id: String) -> Result<Removed, String> {
    let was_live = core::is_on() && core::status().profile_id.as_deref() == Some(id.as_str());
    if was_live {
        core::disconnect(&app);
    }
    let (profile, index) = {
        let mut v = state.vault.lock().unwrap();
        let i = v.profiles.iter().position(|p| p.id == id).ok_or("Такого подключения нет.")?;
        (v.profiles.remove(i), i)
    };
    state.save_vault()?;
    core::note("INFO", &format!("Подключение удалено: {}", profile.name));
    after_profiles_changed(&app, &state);
    Ok(Removed { profile, index, was_live })
}

#[tauri::command(async)]
pub fn restore_profile(app: AppHandle, state: State<AppState>, profile: Profile, index: usize, make_active: bool) -> Result<(), String> {
    let id = profile.id.clone();
    {
        let mut v = state.vault.lock().unwrap();
        if v.profiles.iter().any(|p| p.id == id) {
            return Ok(());
        }
        let i = index.min(v.profiles.len());
        core::note("INFO", &format!("Подключение восстановлено: {}", profile.name));
        v.profiles.insert(i, profile);
    }
    state.save_vault()?;
    if make_active {
        state.settings.lock().unwrap().active_profile = Some(id);
    }
    after_profiles_changed(&app, &state);
    Ok(())
}

/// Ссылка подписки или одиночной конфигурации — для «Копировать».
#[tauri::command]
pub fn profile_link(state: State<AppState>, id: String) -> Result<String, String> {
    let v = state.vault.lock().unwrap();
    let p = v.profiles.iter().find(|p| p.id == id).ok_or("Такого подключения нет.")?;
    match &p.url {
        Some(u) => Ok(u.clone()),
        // Из файла: отдаём прокси в YAML — их можно вставить в другой клиент.
        None => serde_yaml::to_string(&serde_json::json!({ "proxies": p.proxies })).map_err(|e| e.to_string()),
    }
}

// ─────────── Настройки ───────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResult {
    settings: Settings,
    warning: Option<String>,
}

/// Частичное обновление. Каждое изменение сразу вступает в силу: режим
/// маршрутизации и правила — на лету, режим подключения и порт — с
/// переподключением.
#[tauri::command(async)]
pub fn update_settings(app: AppHandle, state: State<AppState>, patch: Value) -> Result<SettingsResult, String> {
    let before = state.settings.lock().unwrap().clone();
    let mut merged = serde_json::to_value(&before).map_err(|e| e.to_string())?;
    let Value::Object(p) = &patch else { return Err("ожидался объект".into()) };
    for (k, v) in p {
        merged[k] = v.clone();
    }
    let mut next: Settings = serde_json::from_value(merged).map_err(|e| format!("неверное значение: {e}"))?;
    if next.proxy_port < 1024 {
        return Err("Порт — от 1024 до 65535.".into());
    }
    for s in &mut next.sites {
        s.pattern = links::normalize_site(&s.pattern);
    }
    next.sites.retain(|s| !s.pattern.is_empty());
    let mut seen = std::collections::HashSet::new();
    next.ks_sites = next
        .ks_sites
        .into_iter()
        .filter_map(|s| killswitch::normalize_domain(&s.pattern).map(|pattern| crate::state::KsSite { pattern, on: s.on }))
        .filter(|s| seen.insert(s.pattern.clone()))
        .collect();
    *state.settings.lock().unwrap() = next.clone();
    state.save_settings();

    // Проблемы Kill Switch окно узнаёт событием «killswitch» — здесь не дублируем.
    let warning = None;
    let on = core::is_on();
    let ks_changed = before.kill_switch != next.kill_switch || before.ks_apps != next.ks_apps || before.ks_sites != next.ks_sites;
    if ks_changed || (on && before.route_mode != next.route_mode) {
        core::ks_apply(&app, core::ks_engaged(&next, on), &next.ks_apps, &next.ks_sites);
    }
    if on {
        if before.mode != next.mode || (before.proxy_port != next.proxy_port && next.mode != Mode::Tun) {
            core::note("INFO", "Режим подключения изменён — переподключаюсь");
            core::reconnect(&app)?;
        } else {
            if before.route_mode != next.route_mode {
                core::set_route_mode(next.route_mode.as_str())?;
                core::note("INFO", &format!("Режим маршрутизации: {}", next.route_mode.as_str()));
            }
            // Защищённое Kill Switch тоже попадает в правила: только через VPN.
            if before.presets != next.presets || before.sites != next.sites || before.apps != next.apps || before.default_route != next.default_route || ks_changed {
                core::apply_config(&app)?;
            }
        }
    }
    crate::tray::refresh(&app);
    Ok(SettingsResult { settings: next, warning })
}

/// «Повторить» на экране Kill Switch.
#[tauri::command(async)]
pub fn retry_kill_switch(app: AppHandle, state: State<AppState>) -> Option<String> {
    let s = state.settings.lock().unwrap().clone();
    let before = killswitch::issue();
    let w = killswitch::retry(core::ks_engaged(&s, core::is_on()), &s.ks_apps, &s.ks_sites);
    if w != before {
        let _ = app.emit("killswitch", w.clone());
    }
    w
}

#[tauri::command(async)]
pub fn set_autostart(state: State<AppState>, enabled: bool) -> Result<bool, String> {
    autostart::set_enabled(enabled)?;
    // Запуск с Windows без подключения смысла не имеет.
    state.settings.lock().unwrap().connect_on_launch = enabled;
    state.save_settings();
    Ok(autostart::is_enabled())
}

#[derive(Serialize)]
pub struct RunningApp {
    #[serde(flatten)]
    proc: procs::Proc,
    /// Скачивает сейчас, байт/с — только при подключённом VPN.
    activity: Option<u64>,
}

#[tauri::command(async)]
pub fn running_apps() -> Vec<RunningApp> {
    let activity = core::process_activity();
    let live = core::is_on();
    let mut list: Vec<RunningApp> = procs::running()
        .into_iter()
        .map(|p| {
            let a = activity.get(&p.exe.to_lowercase()).copied();
            RunningApp { proc: p, activity: if live { Some(a.unwrap_or(0)) } else { None } }
        })
        .collect();
    list.sort_by(|a, b| b.activity.unwrap_or(0).cmp(&a.activity.unwrap_or(0)));
    list
}

#[tauri::command(async)]
pub fn app_from_file(path: String) -> procs::Proc {
    procs::from_path(&path)
}

// ─────────── Логи и прочее ───────────

#[tauri::command]
pub fn get_logs() -> Vec<core::LogEntry> {
    core::logs()
}

#[tauri::command]
pub fn clear_logs() {
    core::clear_logs();
}

#[tauri::command]
pub fn restore_logs(list: Vec<RestoreLog>) {
    core::restore_logs(list.into_iter().map(|l| core::LogEntry { time: l.time, level: level(&l.level), text: l.text }).collect());
}

#[derive(serde::Deserialize)]
pub struct RestoreLog {
    time: String,
    level: String,
    text: String,
}

fn level(s: &str) -> &'static str {
    match s {
        "WARN" => "WARN",
        "ERR" => "ERR",
        "DEBUG" => "DEBUG",
        _ => "INFO",
    }
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_data_dir(state: State<AppState>) {
    let _ = std::process::Command::new(crate::sys::system_exe("explorer.exe")).arg(&state.dir).spawn();
}

#[tauri::command]
pub fn window_minimize(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

/// Крестик прячет в трей: VPN живёт без окна.
#[tauri::command]
pub fn window_close(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

#[tauri::command(async)]
pub fn quit_app(app: AppHandle) {
    core::shutdown(&app);
    app.exit(0);
}

// ─────────── Иконки, «О приложении», GeoIP ───────────

/// Иконка сайта как data:-URI (см. favicon.rs). None — у сайта её нет.
#[tauri::command(async)]
pub fn favicon(app: AppHandle, host: String) -> Option<String> {
    crate::favicon::get(&app, &host).ok().flatten()
}

pub const REPO: &str = "https://github.com/vbu00/klick";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    build: &'static str,
    mihomo: Option<String>,
    system: String,
    data_dir: String,
    geo: crate::geo::GeoInfo,
    repo: &'static str,
}

/// «Windows 11 · x64»: номер сборки из реестра — версию 10/11 по нему и
/// отличают (GetVersionEx без манифеста совместимости врёт).
fn system_name() -> String {
    let out = crate::sys::run("reg.exe", &["query", r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "/v", "CurrentBuildNumber"]);
    let build: Option<u32> = out.split_whitespace().last().and_then(|v| v.parse().ok());
    let name = match build {
        Some(b) if b >= 22000 => "Windows 11",
        Some(_) => "Windows 10",
        None => "Windows",
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "ARM64",
        a => a,
    };
    format!("{name} · {arch}")
}

#[tauri::command(async)]
pub fn app_info(app: AppHandle, state: State<AppState>) -> AppInfo {
    let dir = state.dir.display().to_string();
    let data_dir = match std::env::var("LOCALAPPDATA") {
        Ok(base) if !base.is_empty() && dir.to_lowercase().starts_with(&base.to_lowercase()) => format!("%LOCALAPPDATA%{}", &dir[base.len()..]),
        _ => dir,
    };
    AppInfo {
        version: app.package_info().version.to_string(),
        build: env!("KLICK_BUILD_DATE"),
        mihomo: MIHOMO_VERSION.get_or_init(|| core::mihomo_version(&app)).clone(),
        system: system_name(),
        data_dir,
        geo: crate::geo::info(&app),
        repo: REPO,
    }
}

#[tauri::command(async)]
pub fn update_geo(app: AppHandle) -> Result<crate::geo::GeoInfo, String> {
    crate::geo::update(&app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current: String,
    latest: String,
    newer: bool,
    url: String,
}

fn semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim().trim_start_matches('v').split(['.', '-']).map(|x| x.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// Последний релиз на GitHub. Ничего не скачивает и не ставит — только
/// сравнивает версии; установщик человек берёт со страницы релиза сам.
#[tauri::command(async)]
pub fn check_update(app: AppHandle) -> Result<UpdateCheck, String> {
    let out = crate::sys::command("curl.exe")
        .args(["-sS", "-L", "--fail", "--max-time", "15", "--proto", "=https", "-A", "klick", "-H", "Accept: application/vnd.github+json"])
        .arg("https://api.github.com/repos/vbu00/klick/releases/latest")
        .output()
        .map_err(|e| format!("не удалось запустить curl: {e}"))?;
    if !out.status.success() {
        let err = crate::sys::decode_console(&out.stderr);
        return Err(format!("GitHub не ответил: {}", err.trim().trim_start_matches("curl: ")));
    }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|_| "GitHub ответил непонятно.".to_string())?;
    let latest = v.get("tag_name").and_then(Value::as_str).ok_or("На GitHub пока нет релизов.")?.to_string();
    let current = app.package_info().version.to_string();
    let url = v.get("html_url").and_then(Value::as_str).filter(|u| u.starts_with(REPO)).unwrap_or(REPO).to_string();
    Ok(UpdateCheck { newer: semver(&latest) > semver(&current), latest: latest.trim_start_matches('v').to_string(), current, url })
}

/// Открыть страницу проекта в браузере. Только адреса самого репозитория:
/// окно не должно уметь открывать что угодно. Через explorer.exe — он
/// передаёт адрес уже запущенной оболочке, и браузер стартует с обычными
/// правами, а не с нашими администраторскими.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let ok = url == REPO || url.starts_with(&format!("{REPO}/"));
    if !ok || url.chars().any(|c| c.is_whitespace() || c == '"') {
        return Err("Эту ссылку открыть нельзя.".into());
    }
    std::process::Command::new(crate::sys::system_exe("explorer.exe")).arg(&url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Тексты лицензий — вшиты в программу, показываются без сети.
#[tauri::command]
pub fn licenses() -> String {
    format!(
        "{}\n\n────────────────────────────────────────\n\n{}",
        include_str!("../../LICENSE").trim(),
        include_str!("../../THIRD_PARTY_NOTICES.md").trim()
    )
}
