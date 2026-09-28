//! Установка, обновление, переустановка и удаление. Каждое действие — список задач, их видно
//! на экране «Установка»; прогресс идёт событиями.
//!
//! Пока установщик трогал только папку программы, «Прервать» и любая ошибка возвращают всё как
//! было: прежние файлы лежат рядом в `klick.old`, служба запускается снова. Удаление прежней
//! kl!ck, регистрация службы и записи в реестре — необратимые шаги: с них отмена недоступна.

use crate::migrate;
use crate::payload;
use crate::plan::{self, Info, Old};
use crate::win::{self, Key, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Имя службы Windows — как в klick-service.
const SERVICE: &str = "klick";
/// По этому коду Windows показывает уведомления от имени kl!ck; он же записан в ярлык «Пуска».
const AUMID: &str = "app.klick.desktop";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// Имя значения автозапуска — то же, под которым его пишет переключатель в настройках окна.
const RUN_NAME: &str = "kl!ck";
/// «Включено» для «Автозагрузки» в Диспетчере задач.
const APPROVED_ON: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const LNK: &str = "kl!ck.lnk";
const NOTIFY_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings";
/// Здесь Windows помнит значки трея: показывать ли на панели задач.
const TRAY_SETTINGS: &str = r"Control Panel\NotifyIconSettings";
const PROXY_PORT: u16 = 7890;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Install,
    Update,
    Reinstall,
    Uninstall,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Request {
    pub kind: Kind,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub desktop: bool,
    #[serde(default)]
    pub autostart: bool,
    /// Удаление: стереть подписки и настройки. Обновление и переустановка: начать с чистого листа.
    #[serde(default)]
    pub wipe: bool,
    /// Прежняя kl!ck 0.2–0.4 найдена: перенести её подписки и ссылки (правила не переносятся).
    #[serde(default = "yes")]
    pub keep_old: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    Stop,
    Files,
    Core,
    Old,
    Service,
    Shortcuts,
    StopService,
    Unhook,
    Driver,
    Remove,
    Data,
    Migrate,
}

impl Task {
    /// Подпись для тихого режима; окно подписывает задачи само.
    pub fn label(self) -> String {
        match self {
            Task::Stop => "Остановка kl!ck".into(),
            Task::Files => "Распаковка файлов".into(),
            Task::Core => format!("Ядро mihomo {}", plan::CORE_VERSION).trim_end().to_string(),
            Task::Old => "Удаление прежней kl!ck".into(),
            Task::Service => "Служба kl!ck".into(),
            Task::Shortcuts => "Ярлыки и автозапуск".into(),
            Task::StopService => "Остановка службы kl!ck".into(),
            Task::Unhook => "Снятие Kill Switch и прокси".into(),
            Task::Driver => "Удаление драйвера Wintun".into(),
            Task::Remove => "Удаление файлов".into(),
            Task::Data => "Профили и настройки".into(),
            Task::Migrate => "Перенос подписок".into(),
        }
    }

    /// Доля в полосе прогресса — примерно по времени.
    fn weight(self) -> f64 {
        match self {
            Task::Stop => 8.0,
            Task::Files => 25.0,
            Task::Core => 30.0,
            Task::Old => 20.0,
            Task::Service => 12.0,
            Task::Shortcuts => 5.0,
            Task::StopService => 20.0,
            Task::Unhook => 15.0,
            Task::Driver => 15.0,
            Task::Remove => 25.0,
            Task::Data => 10.0,
            Task::Migrate => 15.0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub tasks: Vec<Task>,
    /// Какая задача идёт; `tasks.len()` — всё сделано.
    pub active: usize,
    /// Сделано, 0–100, и сколько будет, когда закончится текущая задача: между ними окно
    /// плавно ведёт полосу, пока задача без собственного прогресса.
    pub pct: f64,
    pub ceil: f64,
    pub cancellable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    pub ok: bool,
    pub cancelled: bool,
    /// Код ошибки (`path.*`, `folder.*`, `service.*`, …) и подробность от Windows.
    pub error: Option<String>,
    pub detail: Option<String>,
    /// Ошибка случилась до необратимых шагов, и всё вернули как было.
    pub rolled_back: bool,
    /// Не помешало, но стоит сказать: `note.reboot` — часть файлов удалится после перезагрузки.
    pub notes: Vec<String>,
    pub path: String,
    /// Перенос из прежней kl!ck: сколько подключений перенеслось и какие — нет.
    #[serde(default)]
    pub migrated: usize,
    #[serde(default)]
    pub not_migrated: Vec<String>,
}

enum Fail {
    Cancelled,
    Error { code: &'static str, detail: String, rolled_back: bool },
}

fn fail(code: &'static str, detail: impl Into<String>) -> Fail {
    Fail::Error { code, detail: detail.into(), rolled_back: false }
}

pub fn tasks(req: &Request, info: &Info) -> Vec<Task> {
    match req.kind {
        Kind::Uninstall => {
            let mut t = vec![Task::StopService, Task::Unhook, Task::Driver, Task::Remove];
            if req.wipe {
                t.push(Task::Data);
            }
            t
        }
        kind => {
            let mut t = Vec::new();
            if kind != Kind::Install {
                t.push(Task::Stop);
            }
            t.extend([Task::Files, Task::Core]);
            if info.old.is_some() {
                t.push(Task::Old);
            }
            if kind != Kind::Install && req.wipe {
                t.push(Task::Data);
            }
            t.push(Task::Service);
            if migrating(req, info) {
                t.push(Task::Migrate);
            }
            t.push(Task::Shortcuts);
            t
        }
    }
}

/// Есть что переносить из прежней kl!ck, и человек не отказался.
fn migrating(req: &Request, info: &Info) -> bool {
    req.kind != Kind::Uninstall && req.keep_old && info.old.as_ref().is_some_and(|o| o.data.is_some())
}

struct Run<'a> {
    tasks: Vec<Task>,
    at: usize,
    cancellable: bool,
    cancel: &'a AtomicBool,
    emit: &'a mut dyn FnMut(&Progress),
}

impl Run<'_> {
    fn edge(&self, i: usize) -> f64 {
        let total: f64 = self.tasks.iter().map(|t| t.weight()).sum();
        self.tasks[..i.min(self.tasks.len())].iter().map(|t| t.weight()).sum::<f64>() / total * 100.0
    }

    fn send(&mut self, frac: f64) {
        let (start, end) = (self.edge(self.at), self.edge(self.at + 1));
        let p = Progress { tasks: self.tasks.clone(), active: self.at, pct: start + (end - start) * frac.clamp(0.0, 1.0), ceil: end, cancellable: self.cancellable };
        (self.emit)(&p);
    }

    fn begin(&mut self, t: Task) {
        self.at = self.tasks.iter().position(|x| *x == t).unwrap_or(self.at);
        self.send(0.0);
    }

    /// Дальше отменить нельзя. Если «Прервать» нажали только что — ещё успеваем.
    fn point_of_no_return(&mut self) -> bool {
        if self.cancel.load(Ordering::SeqCst) {
            return false;
        }
        self.cancellable = false;
        self.send(1.0);
        true
    }

    fn cancelled(&self) -> bool {
        self.cancellable && self.cancel.load(Ordering::SeqCst)
    }

    fn finish(&mut self) {
        self.at = self.tasks.len();
        let p = Progress { tasks: self.tasks.clone(), active: self.at, pct: 100.0, ceil: 100.0, cancellable: false };
        (self.emit)(&p);
    }
}

pub fn run(req: &Request, info: &Info, cancel: &AtomicBool, emit: &mut dyn FnMut(&Progress)) -> Outcome {
    let mut r = Run { tasks: tasks(req, info), at: 0, cancellable: true, cancel, emit };
    let path = match req.kind {
        Kind::Uninstall => info.installed.as_ref().map(|i| i.path.clone()).unwrap_or_else(|| req.path.clone()),
        _ => plan::check_path(&req.path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| req.path.clone()),
    };
    let result = match req.kind {
        Kind::Uninstall => uninstall(req, Path::new(&path), &mut r),
        _ => install(req, info, &mut r),
    };
    let base = Outcome { ok: false, cancelled: false, error: None, detail: None, rolled_back: false, notes: Vec::new(), path, migrated: 0, not_migrated: Vec::new() };
    match result {
        Ok(notes) => {
            let (migrated, not_migrated) = LAST_MIGRATION.with(|m| std::mem::take(&mut *m.borrow_mut()));
            Outcome { ok: true, notes, migrated, not_migrated, ..base }
        }
        Err(Fail::Cancelled) => Outcome { cancelled: true, rolled_back: true, ..base },
        Err(Fail::Error { code, detail, rolled_back }) => Outcome { error: Some(code.into()), detail: (!detail.is_empty()).then_some(detail), rolled_back, ..base },
    }
}

// ── Установка, обновление, переустановка ──────────────────────────────

/// Как вернуть всё назад, пока не начались необратимые шаги.
struct Undo {
    dir: PathBuf,
    created: bool,
    backup: Option<PathBuf>,
    restart_service: bool,
}

impl Undo {
    fn revert(&mut self) -> bool {
        win::kill_under(&[self.dir.clone()]);
        let mut ok = true;
        if self.created {
            ok &= win::remove_tree(&self.dir);
        }
        if let Some(b) = self.backup.take() {
            ok &= win::rename_dir(&b, &self.dir).is_ok();
        }
        if self.restart_service {
            let _ = win::start_service(SERVICE, Duration::from_secs(30));
        }
        ok
    }

    fn fail(&mut self, code: &'static str, detail: impl Into<String>) -> Fail {
        let rolled_back = self.revert();
        Fail::Error { code, detail: detail.into(), rolled_back }
    }

    fn cancel(&mut self) -> Fail {
        self.revert();
        Fail::Cancelled
    }
}

/// Рядом с папкой программы: сюда уходят прежние файлы на время обновления.
fn backup_path(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".old");
    dir.with_file_name(name)
}

fn is_empty_dir(dir: &Path) -> bool {
    std::fs::read_dir(dir).map(|mut d| d.next().is_none()).unwrap_or(true)
}

fn install(req: &Request, info: &Info, r: &mut Run) -> Result<Vec<String>, Fail> {
    let dir = plan::check_path(&req.path).map_err(|code| Fail::Error { code, detail: String::new(), rolled_back: true })?;
    let upgrading = req.kind != Kind::Install;
    let mut notes = Vec::new();
    let mut undo = Undo { dir: dir.clone(), created: false, backup: None, restart_service: false };

    if upgrading {
        r.begin(Task::Stop);
        undo.restart_service = win::service_running(SERVICE) == Some(true);
        stop_klick(&dir);
        if r.cancelled() {
            return Err(undo.cancel());
        }
    }

    // Прежние файлы — в сторону: при отмене и ошибке они вернутся на место.
    if dir.exists() && !is_empty_dir(&dir) {
        if !upgrading {
            stop_klick(&dir);
        }
        let backup = backup_path(&dir);
        if backup.exists() && !win::remove_tree(&backup) {
            return Err(undo.fail("folder.busy", backup.display().to_string()));
        }
        win::rename_dir(&dir, &backup).map_err(|e| undo.fail("folder.busy", e))?;
        undo.backup = Some(backup);
    }
    std::fs::create_dir_all(&dir).map_err(|e| undo.fail("folder.create", e.to_string()))?;
    undo.created = true;
    if !win::under_program_files(&dir) {
        // Права как у Program Files — ставим до первого файла, чтобы всё внутри их унаследовало.
        win::lock_folder(&dir).map_err(|e| undo.fail("folder.lock", e))?;
    }

    r.begin(Task::Files);
    extract(&dir, &|n| !n.starts_with("resources/"), r).map_err(|e| if e == "cancelled" { undo.cancel() } else { undo.fail("files.write", e) })?;
    copy_self(&dir).map_err(|e| undo.fail("files.write", e))?;

    r.begin(Task::Core);
    extract(&dir, &|n| n.starts_with("resources/"), r).map_err(|e| if e == "cancelled" { undo.cancel() } else { undo.fail("files.write", e) })?;

    if !r.point_of_no_return() {
        return Err(undo.cancel());
    }

    // Подписки прежней kl!ck — прочитать, пока её данные ещё на месте.
    let carry: Vec<migrate::Item> = if migrating(req, info) {
        info.old.as_ref().and_then(|o| o.data.as_deref()).map(|d| migrate::read_old(Path::new(d))).unwrap_or_default()
    } else {
        vec![]
    };

    if let Some(old) = &info.old {
        r.begin(Task::Old);
        notes.extend(remove_old(old));
    }

    if upgrading && req.wipe {
        // Начать с чистого листа: служба остановлена на шаге «Остановка». Прокси окна — вернуть
        // до того, как исчезнет копия прежних настроек.
        r.begin(Task::Data);
        restore_proxy();
        for d in data_dirs() {
            if d.exists() && !win::remove_tree(&d) {
                notes.push("note.reboot".into());
            }
        }
    }

    r.begin(Task::Service);
    write_uninstall_entry(&dir);
    if upgrading && win::service_running(SERVICE).is_some() {
        // Служба уже зарегистрирована на эту папку — только запустить.
        win::start_service(SERVICE, Duration::from_secs(30)).map_err(|e| fail("service.start", e))?;
    } else {
        register_service(&dir.join("klick-service.exe"))?;
    }

    let (mut migrated, mut not_migrated) = (0, vec![]);
    if migrating(req, info) {
        r.begin(Task::Migrate);
        (migrated, not_migrated) = push_with_timeout(carry, Duration::from_secs(180));
    }

    r.begin(Task::Shortcuts);
    shortcuts(&dir, req, upgrading);
    win::refresh_icons();
    if let Some(b) = undo.backup.take() {
        if !win::remove_tree(&b) {
            notes.push("note.reboot".into());
        }
    }
    r.finish();
    notes.dedup();
    LAST_MIGRATION.with(|m| *m.borrow_mut() = (migrated, not_migrated));
    Ok(notes)
}

thread_local! {
    /// Итог переноса для `run`: `install` возвращает только заметки.
    static LAST_MIGRATION: std::cell::RefCell<(usize, Vec<String>)> = const { std::cell::RefCell::new((0, Vec::new())) };
}

/// Передать подключения новой службе, но не ждать дольше `limit`: подписка, панель которой
/// не отвечает, не должна повесить установщик.
fn push_with_timeout(items: Vec<migrate::Item>, limit: Duration) -> (usize, Vec<String>) {
    if items.is_empty() {
        return (0, vec![]);
    }
    let names: Vec<String> = items.iter().map(|i| i.name().to_string()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(migrate::push(r"\\.\pipe\klick", &items));
    });
    rx.recv_timeout(limit).unwrap_or((0, names))
}

fn extract(dir: &Path, filter: &dyn Fn(&str) -> bool, r: &mut Run) -> Result<(), String> {
    let total = payload::sum(filter).max(1) as f64;
    let cancel = r.cancel;
    let mut last = 0.0;
    payload::extract(dir, filter, cancel, &mut |done| {
        let frac = done as f64 / total;
        if frac - last >= 0.01 || frac >= 1.0 {
            last = frac;
            r.send(frac);
        }
    })
}

/// Копия установщика в папке программы: из неё «Установленные приложения» удаляют kl!ck.
fn copy_self(dir: &Path) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::copy(&me, dir.join("klick-setup.exe")).map(|_| ()).map_err(|e| format!("klick-setup.exe: {e}"))
}

/// Остановить kl!ck из этой папки: службу, окно, ядро.
fn stop_klick(dir: &Path) {
    let dirs = [dir.to_path_buf()];
    let window = win::running_under(&dirs);
    win::stop_service(SERVICE, Duration::from_secs(20));
    // Окно замечает, что служба пропала, и само снимает свой системный прокси — дать ему миг.
    if window {
        std::thread::sleep(Duration::from_millis(1500));
    }
    win::kill_under(&dirs);
}

fn wait_service_gone(timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while win::service_running(SERVICE).is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn register_service(exe: &Path) -> Result<(), Fail> {
    // Осталась служба от прежней установки — она смотрит на другую папку.
    if win::service_running(SERVICE).is_some() {
        win::stop_service(SERVICE, Duration::from_secs(20));
        win::delete_service(SERVICE);
        wait_service_gone(Duration::from_secs(10));
    }
    let mut last = String::new();
    for _ in 0..5 {
        match win::run(exe, &["install"], Duration::from_secs(60)) {
            Ok(0) => break,
            Ok(code) => last = format!("klick-service install: код {code}"),
            Err(e) => last = e,
        }
        if win::service_running(SERVICE).is_some() {
            break;
        }
        // Windows ещё не убрала прежнюю запись о службе.
        std::thread::sleep(Duration::from_secs(1));
    }
    if win::service_running(SERVICE).is_none() {
        return Err(fail("service.register", last));
    }
    win::start_service(SERVICE, Duration::from_secs(30)).map_err(|e| fail("service.start", e))
}

fn write_uninstall_entry(dir: &Path) {
    let Some(k) = Key::create(HKEY_LOCAL_MACHINE, plan::UNINSTALL_KEY) else { return };
    let setup = dir.join("klick-setup.exe");
    k.set_string("DisplayName", "kl!ck");
    k.set_string("DisplayVersion", plan::VERSION);
    k.set_string("Publisher", "vbu00");
    k.set_string("InstallLocation", &dir.to_string_lossy());
    k.set_string("DisplayIcon", &dir.join("klick.exe").to_string_lossy());
    k.set_string("UninstallString", &format!("\"{}\" --uninstall", setup.display()));
    k.set_string("QuietUninstallString", &format!("\"{}\" --uninstall --silent", setup.display()));
    k.set_dword("EstimatedSize", (dir_size(dir) / 1024) as u32);
    k.set_dword("NoModify", 1);
    k.set_dword("NoRepair", 1);
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

/// Ярлык в «Пуске» — всегда: по нему Windows показывает уведомления от имени kl!ck.
/// На рабочем столе и автозапуск — как выбрали; при обновлении — как было.
fn shortcuts(dir: &Path, req: &Request, upgrading: bool) {
    let exe = dir.join("klick.exe");
    let _ = win::shortcut(&win::start_menu().join(LNK), &exe, "", AUMID);

    let desktops: Vec<PathBuf> = [Some(win::public_desktop()), win::user_desktop()].into_iter().flatten().map(|d| d.join(LNK)).collect();
    if upgrading {
        for lnk in desktops.iter().filter(|l| points_into(l, dir)) {
            let _ = win::shortcut(lnk, &exe, "", AUMID);
        }
    } else if req.desktop {
        let _ = win::shortcut(&win::public_desktop().join(LNK), &exe, "", AUMID);
    }

    let ours = autostart_points_into(dir);
    if (!upgrading && req.autostart) || (upgrading && ours) {
        set_autostart(&exe);
    } else if !upgrading && !req.autostart && ours {
        remove_autostart(dir);
    }
}

fn points_into(lnk: &Path, dir: &Path) -> bool {
    lnk.exists() && win::shortcut_target(lnk).is_some_and(|t| win::inside(&t, dir))
}

fn set_autostart(exe: &Path) {
    if let Some(k) = Key::create(HKEY_CURRENT_USER, RUN_KEY) {
        k.set_string(RUN_NAME, &format!("\"{}\" --hidden", exe.display()));
    }
    if let Some(k) = Key::create(HKEY_CURRENT_USER, APPROVED_KEY) {
        k.set_binary(RUN_NAME, &APPROVED_ON);
    }
}

/// Значение автозапуска ведёт в эту папку. Окно пишет путь без кавычек, установщик — в кавычках.
fn autostart_points_into(dir: &Path) -> bool {
    Key::open(HKEY_CURRENT_USER, RUN_KEY)
        .and_then(|k| k.string(RUN_NAME))
        .is_some_and(|v| win::inside(Path::new(v.trim_start_matches('"').split('"').next().unwrap_or_default().trim_end_matches(" --hidden")), dir))
}

fn remove_autostart(dir: &Path) {
    if !autostart_points_into(dir) {
        return;
    }
    if let Some(k) = Key::open(HKEY_CURRENT_USER, RUN_KEY) {
        k.delete_value(RUN_NAME);
    }
    if let Some(k) = Key::open(HKEY_CURRENT_USER, APPROVED_KEY) {
        k.delete_value(RUN_NAME);
    }
}

// ── Прежняя kl!ck ──────────────────────────────────────────────────────

/// Убрать прежнюю kl!ck со всеми следами. Сначала — её собственным деинсталлятором (он же
/// снимает правила Kill Switch, возвращает системный прокси и удаляет задачу автозапуска),
/// потом — всё, что могло остаться: деинсталлятора нет, он упал или папку удаляли руками.
fn remove_old(old: &Old) -> Vec<String> {
    let mut notes = Vec::new();
    let dirs: Vec<PathBuf> = old.dirs.iter().map(PathBuf::from).collect();
    win::kill_under(&dirs);

    if let Some(u) = old.uninstaller.as_deref().map(PathBuf::from) {
        if let Some(parent) = u.parent() {
            // `_?=` — ждать, пока закончит, а не копировать себя во временную папку; без кавычек.
            let _ = win::run_raw(&u, &format!("/S _?={}", parent.display()), Duration::from_secs(120));
        }
    }

    let group = plan::OLD_FIREWALL_GROUP;
    let _ = win::powershell(
        &format!("$r = Get-NetFirewallRule -Group '{group}' -ErrorAction SilentlyContinue; if ($r) {{ $r | Remove-NetFirewallRule }}"),
        Duration::from_secs(60),
    );
    let schtasks = win::system32("schtasks.exe");
    if win::run(&schtasks, &["/Query", "/TN", plan::OLD_TASK], Duration::from_secs(10)).is_ok_and(|c| c == 0) {
        let _ = win::run(&schtasks, &["/Delete", "/TN", plan::OLD_TASK, "/F"], Duration::from_secs(10));
    }
    if let Some(data) = plan::old_data() {
        restore_old_proxy(&data);
    }

    remove_links_into(&dirs);
    forget_tray_icons(&dirs);
    win::delete_key(HKEY_CURRENT_USER, &format!(r"{NOTIFY_SETTINGS}\com.vbu00.klick"));

    for d in &dirs {
        if !win::remove_tree(d) {
            notes.push("note.reboot".into());
        }
    }
    if let Some(data) = plan::old_data().filter(|d| d.exists()) {
        // Окно прежней kl!ck только что закрыли — WebView2 держит свои файлы ещё пару секунд.
        if !win::remove_tree(&data) {
            notes.push("note.reboot".into());
        }
    }
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        win::delete_key(root, plan::OLD_UNINSTALL_KEY);
        // Раздел издателя общий: там же живёт, например, Klutz — его не трогаем.
        win::delete_key(root, r"SOFTWARE\vbu00\kl!ck");
        win::delete_key_if_empty(root, r"SOFTWARE\vbu00");
    }
    notes
}

/// Системный прокси прежней kl!ck: вернуть то, что было до неё, если он всё ещё стоит.
fn restore_old_proxy(data: &Path) {
    #[derive(Deserialize)]
    struct Saved {
        enable: u32,
        server: Option<String>,
        overrides: Option<String>,
    }
    #[derive(Deserialize, Default)]
    #[serde(default, rename_all = "camelCase")]
    struct Settings {
        proxy_port: Option<u16>,
        mode: Option<String>,
    }
    let settings: Settings = std::fs::read(data.join("settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let ours = format!("127.0.0.1:{}", settings.proxy_port.unwrap_or(PROXY_PORT));
    let Some(now) = win::proxy_query() else { return };
    if !now.on() || now.server.as_deref() != Some(ours.as_str()) {
        return;
    }
    let backup: Option<Saved> = std::fs::read(data.join("sysproxy-backup.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let restored = match backup {
        Some(b) if b.server.as_deref() != Some(ours.as_str()) => win::Proxy { flags: flags_of(b.enable), server: b.server, bypass: b.overrides, pac: now.pac.clone() },
        // Прежний сервер — её же (двойное включение), или бэкапа нет, но режим «Системный прокси»:
        // этот прокси ставила она — просто выключить.
        Some(_) => proxy_off(&now),
        None if settings.mode.as_deref() == Some("sysproxy") => proxy_off(&now),
        // Не доказано, что это её прокси: 127.0.0.1:7890 бывает и у Clash.
        None => return,
    };
    win::proxy_set(&restored);
}

/// Флаги WinINet по старому признаку «прокси включён».
fn flags_of(enable: u32) -> u32 {
    if enable == 1 {
        win::PROXY_TYPE_DIRECT | win::PROXY_TYPE_PROXY
    } else {
        win::PROXY_TYPE_DIRECT
    }
}

/// Выключить прокси, оставив остальное (автообнаружение, сценарий настройки) как есть.
fn proxy_off(now: &win::Proxy) -> win::Proxy {
    win::Proxy { flags: (now.flags & !win::PROXY_TYPE_PROXY) | win::PROXY_TYPE_DIRECT, ..now.clone() }
}

fn link_candidates() -> Vec<PathBuf> {
    [Some(win::start_menu()), win::user_start_menu(), Some(win::public_desktop()), win::user_desktop()].into_iter().flatten().map(|d| d.join(LNK)).collect()
}

/// Убрать ярлыки kl!ck, которые ведут в эти папки: чужие ярлыки с тем же именем не трогаем.
fn remove_links_into(dirs: &[PathBuf]) {
    for lnk in link_candidates() {
        if lnk.exists() && win::shortcut_target(&lnk).is_some_and(|t| dirs.iter().any(|d| win::inside(&t, d))) {
            let _ = std::fs::remove_file(&lnk);
        }
    }
}

fn forget_tray_icons(dirs: &[PathBuf]) {
    let Some(root) = Key::open(HKEY_CURRENT_USER, TRAY_SETTINGS) else { return };
    for sub in root.subkeys() {
        let path = format!(r"{TRAY_SETTINGS}\{sub}");
        let exe = Key::open(HKEY_CURRENT_USER, &path).and_then(|k| k.string("ExecutablePath"));
        if exe.is_some_and(|e| dirs.iter().any(|d| win::inside(Path::new(&win::expand_known_folder(&e)), d))) {
            win::delete_key(HKEY_CURRENT_USER, &path);
        }
    }
}

// ── Удаление ───────────────────────────────────────────────────────────

fn uninstall(req: &Request, dir: &Path, r: &mut Run) -> Result<Vec<String>, Fail> {
    let mut notes = Vec::new();
    let dirs = [dir.to_path_buf()];

    r.begin(Task::StopService);
    let was_running = win::service_running(SERVICE) == Some(true);
    stop_klick(dir);
    if !r.point_of_no_return() {
        if was_running {
            let _ = win::start_service(SERVICE, Duration::from_secs(30));
        }
        return Err(Fail::Cancelled);
    }
    win::delete_service(SERVICE);
    wait_service_gone(Duration::from_secs(10));

    r.begin(Task::Unhook);
    // Фильтры Kill Switch живут и без службы — иначе защищённые программы остались бы без сети.
    let svc = dir.join("klick-service.exe");
    if svc.exists() {
        let _ = win::run(&svc, &["cleanup-wfp"], Duration::from_secs(30));
    }
    restore_proxy();

    r.begin(Task::Driver);
    let pnputil = win::system32("pnputil.exe");
    let packages = win::wintun_packages();
    if !packages.is_empty() {
        // Сначала — отключённые адаптеры Wintun, которые Windows помнит после TUN. Работающие
        // (у другого VPN) не трогаем: они в этот список не попадают.
        if let Ok(out) = win::run_capture(&pnputil, &["/enum-devices", "/disconnected"], Duration::from_secs(60)) {
            for id in win::wintun_ghosts(&out) {
                let _ = win::run(&pnputil, &["/remove-device", &id], Duration::from_secs(60));
            }
        }
        // Без /force: пока драйвером пользуется чей-то адаптер, Windows его не отдаст.
        for inf in packages {
            let _ = win::run(&pnputil, &["/delete-driver", &inf], Duration::from_secs(60));
        }
    }

    r.begin(Task::Remove);
    remove_links_into(&dirs);
    remove_autostart(dir);
    forget_tray_icons(&dirs);
    win::delete_key(HKEY_CURRENT_USER, &format!(r"{NOTIFY_SETTINGS}\{AUMID}"));
    if !remove_program(dir) {
        notes.push("note.reboot".into());
    }
    let backup = backup_path(dir);
    if backup.exists() && !win::remove_tree(&backup) {
        notes.push("note.reboot".into());
    }
    win::delete_key(HKEY_LOCAL_MACHINE, plan::UNINSTALL_KEY);

    if req.wipe {
        r.begin(Task::Data);
        for d in data_dirs() {
            if d.exists() && !win::remove_tree(&d) {
                notes.push("note.reboot".into());
            }
        }
    }
    r.finish();
    notes.dedup();
    Ok(notes)
}

/// Системный прокси этой kl!ck, если окно или служба не успели его снять. Трогаем настройки,
/// только если есть отметка, что прокси ставила kl!ck: 127.0.0.1:7890 бывает и у Clash.
fn restore_proxy() {
    /// Копия окна (с флагами и сценарием настройки) или отметка службы (без них).
    #[derive(Deserialize)]
    struct Saved {
        enable: u32,
        server: Option<String>,
        bypass: Option<String>,
        #[serde(default)]
        flags: Option<u32>,
        #[serde(default)]
        pac: Option<String>,
    }
    let window = std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("klick").join("proxy-backup.json"));
    let service = plan::data_dir().join("user-proxy.json");
    let marks: Vec<PathBuf> = [window, Some(service)].into_iter().flatten().filter(|p| p.exists()).collect();
    if marks.is_empty() {
        return;
    }
    let saved: Option<Saved> = marks.iter().find_map(|p| std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()));
    if let Some(now) = win::proxy_query() {
        if now.on() && now.server.as_deref() == Some(format!("127.0.0.1:{PROXY_PORT}").as_str()) {
            let restored = match saved {
                Some(s) if s.server.is_some() || s.flags.is_some() => {
                    win::Proxy { flags: s.flags.unwrap_or_else(|| flags_of(s.enable)), server: s.server, bypass: s.bypass, pac: s.pac.or_else(|| now.pac.clone()) }
                }
                _ => proxy_off(&now),
            };
            win::proxy_set(&restored);
        }
    }
    for m in marks {
        let _ = std::fs::remove_file(m);
    }
}

/// Папку программы — целиком, если это точно она; иначе только свои файлы. `false` — что-то
/// удалится только после перезагрузки.
fn remove_program(dir: &Path) -> bool {
    if !dir.exists() {
        return true;
    }
    let whole = safe_to_remove(dir);
    // Копия установщика бывает занята: удаление запустили из неё, и она ждёт, чем кончится.
    // Её и опустевшую папку уберём, когда она завершится.
    let setup = dir.join("klick-setup.exe");
    let _ = std::fs::remove_file(&setup);
    let setup_busy = setup.exists();
    let mut gone = true;
    let entries: Vec<PathBuf> = if whole {
        std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default()
    } else {
        ["klick.exe", "klick-service.exe", "resources"].iter().map(|n| dir.join(n)).collect()
    };
    for p in entries.iter().filter(|p| **p != setup && p.exists()) {
        if p.is_dir() {
            gone &= win::remove_tree(p);
        } else if !remove_file_retry(p) {
            win::delete_on_reboot(p);
            gone = false;
        }
    }
    if setup_busy {
        remove_after_exit(&setup, dir);
    } else {
        let _ = std::fs::remove_dir(dir);
    }
    gone
}

fn remove_file_retry(p: &Path) -> bool {
    for _ in 0..10 {
        if std::fs::remove_file(p).is_ok() || !p.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

/// Удалить файл, как только его отпустит процесс, и затем папку, если она опустела.
fn remove_after_exit(file: &Path, dir: &Path) {
    let script = format!(
        "$f = {}; $d = {}; for ($i = 0; $i -lt 120; $i++) {{ try {{ [IO.File]::Delete($f) }} catch {{}}; if (-not (Test-Path -LiteralPath $f)) {{ break }}; Start-Sleep -Milliseconds 500 }}; try {{ [IO.Directory]::Delete($d) }} catch {{}}",
        win::ps_quote(&file.to_string_lossy()),
        win::ps_quote(&dir.to_string_lossy())
    );
    win::spawn_powershell(&script);
}

/// Путь из реестра мог испортиться: никогда не удалять целиком системные и общие папки.
fn safe_to_remove(dir: &Path) -> bool {
    let ours = dir.join("klick-service.exe").exists() || dir.join("klick-setup.exe").exists();
    let depth = dir.components().count();
    let shared = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)", "ProgramData", "SystemRoot", "USERPROFILE", "PUBLIC", "LOCALAPPDATA", "APPDATA"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .any(|p| p.to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(dir.to_string_lossy().trim_end_matches('\\')));
    ours && depth >= 3 && !shared
}

/// Профили, подписки, ключи и настройки: данные службы, окна и его WebView2.
fn data_dirs() -> Vec<PathBuf> {
    let mut v = vec![plan::data_dir()];
    if let Some(l) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        v.push(l.join("klick"));
        v.push(l.join(AUMID));
    }
    if let Some(a) = std::env::var_os("APPDATA").map(PathBuf::from) {
        v.push(a.join(AUMID));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(old: bool) -> Info {
        Info {
            version: "0.4.0".into(),
            core_version: String::new(),
            default_path: String::new(),
            size: 0,
            installed: None,
            old: old.then(|| Old { version: None, dirs: vec![], uninstaller: None, data: None, task: false }),
            data_path: String::new(),
        }
    }

    fn req(kind: Kind, wipe: bool) -> Request {
        Request { kind, path: String::new(), desktop: true, autostart: true, wipe, keep_old: true }
    }

    #[test]
    fn migration_and_clean_start_are_tasks() {
        let mut with_data = info(true);
        with_data.old.as_mut().unwrap().data = Some(r"C:\Users\x\AppData\Local\com.vbu00.klick".into());
        // Есть что переносить — шаг «Перенос подписок» после службы: ей и передаём.
        assert_eq!(tasks(&req(Kind::Install, false), &with_data), vec![Task::Files, Task::Core, Task::Old, Task::Service, Task::Migrate, Task::Shortcuts]);
        let no = Request { keep_old: false, ..req(Kind::Install, false) };
        assert!(!tasks(&no, &with_data).contains(&Task::Migrate));
        // Переустановка «с чистого листа» — данные стираются до запуска службы.
        let t = tasks(&req(Kind::Reinstall, true), &info(false));
        assert!(t.iter().position(|x| *x == Task::Data) < t.iter().position(|x| *x == Task::Service));
        // Старые запросы окна (без keep_old) — переносить по умолчанию.
        let r: Request = serde_json::from_str(r#"{"kind":"install","path":"C:\\x"}"#).unwrap();
        assert!(r.keep_old && !r.wipe);
    }

    #[test]
    fn task_lists_follow_the_action() {
        assert_eq!(tasks(&req(Kind::Install, false), &info(false)), vec![Task::Files, Task::Core, Task::Service, Task::Shortcuts]);
        assert_eq!(tasks(&req(Kind::Install, false), &info(true)), vec![Task::Files, Task::Core, Task::Old, Task::Service, Task::Shortcuts]);
        assert_eq!(tasks(&req(Kind::Update, false), &info(false))[0], Task::Stop);
        assert_eq!(tasks(&req(Kind::Uninstall, false), &info(false)).len(), 4);
        assert_eq!(tasks(&req(Kind::Uninstall, true), &info(false)).last(), Some(&Task::Data));
    }

    #[test]
    fn backup_sits_next_to_the_folder() {
        assert_eq!(backup_path(Path::new(r"C:\Program Files\klick")), PathBuf::from(r"C:\Program Files\klick.old"));
    }

    #[test]
    fn shared_folders_are_never_removed_whole() {
        assert!(!safe_to_remove(Path::new(r"C:\")));
        assert!(!safe_to_remove(Path::new(&std::env::var("ProgramW6432").unwrap())));
        assert!(!safe_to_remove(Path::new(r"C:\Program Files\klick-test-nonexistent")));
    }

    #[test]
    fn progress_goes_from_zero_to_hundred() {
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let mut emit = |p: &Progress| seen.push((p.active, p.pct, p.ceil));
        let mut r = Run { tasks: vec![Task::Files, Task::Core], at: 0, cancellable: true, cancel: &cancel, emit: &mut emit };
        r.begin(Task::Files);
        r.send(0.5);
        r.begin(Task::Core);
        r.finish();
        let total = 25.0 + 30.0;
        assert_eq!(seen[0], (0, 0.0, 25.0 / total * 100.0));
        assert!((seen[1].1 - 12.5 / total * 100.0).abs() < 1e-9);
        assert_eq!(seen.last().unwrap(), &(2, 100.0, 100.0));
    }
}
