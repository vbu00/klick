//! Системные мелочи установщика: реестр, ярлыки, процессы, запуск, права на папку, место на диске.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, GlobalFree, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SUCCESS, HGLOBAL};
use windows::Win32::Networking::WinInet::{
    InternetQueryOptionW, InternetSetOptionW, INTERNET_OPTION_PER_CONNECTION_OPTION, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED, INTERNET_PER_CONN,
    INTERNET_PER_CONN_AUTOCONFIG_URL, INTERNET_PER_CONN_FLAGS, INTERNET_PER_CONN_FLAGS_UI, INTERNET_PER_CONN_OPTIONW, INTERNET_PER_CONN_OPTIONW_0,
    INTERNET_PER_CONN_OPTION_LISTW, INTERNET_PER_CONN_PROXY_BYPASS, INTERNET_PER_CONN_PROXY_SERVER,
};
pub use windows::Win32::Networking::WinInet::{PROXY_TYPE_DIRECT, PROXY_TYPE_PROXY};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
use windows::core::PROPVARIANT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegEnumKeyExW, RegOpenKeyExW, RegQueryInfoKeyW, RegQueryValueExW, RegSetValueExW, HKEY,
    KEY_READ, KEY_WRITE, REG_BINARY, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::System::Services::{
    CloseServiceHandle, ControlService, DeleteService, OpenSCManagerW, OpenServiceW, QueryServiceStatus, StartServiceW, SC_HANDLE, SC_MANAGER_CONNECT,
    SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS, SERVICE_STATUS_CURRENT_STATE, SERVICE_STOP, SERVICE_STOPPED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    PROCESS_SYNCHRONIZE,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    IShellLinkW, SHChangeNotify, SHGetKnownFolderPath, ShellLink, FOLDERID_Desktop, KF_FLAG_DEFAULT, SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
};

pub use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ── Реестр ─────────────────────────────────────────────────────────────

pub struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

impl Key {
    pub fn open(root: HKEY, path: &str) -> Option<Key> {
        let p = wide(path);
        let mut key = HKEY::default();
        let rc = unsafe { RegOpenKeyExW(root, PCWSTR(p.as_ptr()), 0, KEY_READ | KEY_WRITE, &mut key) };
        (rc == ERROR_SUCCESS).then_some(Key(key))
    }

    pub fn create(root: HKEY, path: &str) -> Option<Key> {
        let p = wide(path);
        let mut key = HKEY::default();
        let rc = unsafe { RegCreateKeyExW(root, PCWSTR(p.as_ptr()), 0, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_READ | KEY_WRITE, None, &mut key, None) };
        (rc == ERROR_SUCCESS).then_some(Key(key))
    }

    pub fn string(&self, name: &str) -> Option<String> {
        let n = wide(name);
        let mut ty = REG_VALUE_TYPE::default();
        let mut buf = vec![0u16; 4096];
        let mut size = (buf.len() * 2) as u32;
        let rc = unsafe { RegQueryValueExW(self.0, PCWSTR(n.as_ptr()), None, Some(&mut ty), Some(buf.as_mut_ptr() as *mut u8), Some(&mut size)) };
        if rc != ERROR_SUCCESS || ty != REG_SZ {
            return None;
        }
        let len = (size as usize / 2).min(buf.len());
        let end = buf[..len].iter().position(|c| *c == 0).unwrap_or(len);
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    pub fn dword(&self, name: &str) -> Option<u32> {
        let n = wide(name);
        let mut ty = REG_VALUE_TYPE::default();
        let mut v = 0u32;
        let mut size = 4u32;
        let rc = unsafe { RegQueryValueExW(self.0, PCWSTR(n.as_ptr()), None, Some(&mut ty), Some(&mut v as *mut u32 as *mut u8), Some(&mut size)) };
        (rc == ERROR_SUCCESS && ty == REG_DWORD).then_some(v)
    }

    pub fn set_string(&self, name: &str, v: &str) {
        let n = wide(name);
        let data: Vec<u8> = wide(v).iter().flat_map(|c| c.to_le_bytes()).collect();
        unsafe {
            let _ = RegSetValueExW(self.0, PCWSTR(n.as_ptr()), 0, REG_SZ, Some(&data));
        }
    }

    pub fn set_dword(&self, name: &str, v: u32) {
        let n = wide(name);
        unsafe {
            let _ = RegSetValueExW(self.0, PCWSTR(n.as_ptr()), 0, REG_DWORD, Some(&v.to_le_bytes()));
        }
    }

    pub fn set_binary(&self, name: &str, v: &[u8]) {
        let n = wide(name);
        unsafe {
            let _ = RegSetValueExW(self.0, PCWSTR(n.as_ptr()), 0, REG_BINARY, Some(v));
        }
    }

    pub fn delete_value(&self, name: &str) {
        let n = wide(name);
        unsafe {
            let _ = RegDeleteValueW(self.0, PCWSTR(n.as_ptr()));
        }
    }

    pub fn subkeys(&self) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0.. {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            let rc = unsafe { RegEnumKeyExW(self.0, i, PWSTR(buf.as_mut_ptr()), &mut len, None, PWSTR::null(), None, None) };
            if rc != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        out
    }

    /// Ни вложенных разделов, ни значений.
    pub fn is_empty(&self) -> bool {
        let (mut keys, mut values) = (0u32, 0u32);
        let rc = unsafe { RegQueryInfoKeyW(self.0, PWSTR::null(), None, None, Some(&mut keys), None, None, Some(&mut values), None, None, None, None) };
        rc == ERROR_SUCCESS && keys == 0 && values == 0
    }
}

/// Удалить раздел, только если он пуст: общий раздел издателя делят разные программы.
pub fn delete_key_if_empty(root: HKEY, path: &str) {
    if Key::open(root, path).is_some_and(|k| k.is_empty()) {
        let p = wide(path);
        unsafe {
            let _ = windows::Win32::System::Registry::RegDeleteKeyW(root, PCWSTR(p.as_ptr()));
        }
    }
}

/// Удалить раздел реестра вместе с вложенными.
pub fn delete_key(root: HKEY, path: &str) {
    let p = wide(path);
    unsafe {
        let _ = RegDeleteTreeW(root, PCWSTR(p.as_ptr()));
    }
    let (parent, name) = path.rsplit_once('\\').unwrap_or(("", path));
    if let Some(k) = Key::open(root, parent) {
        let n = wide(name);
        unsafe {
            let _ = windows::Win32::System::Registry::RegDeleteKeyW(k.0, PCWSTR(n.as_ptr()));
        }
    }
}

// ── Процессы ───────────────────────────────────────────────────────────

pub fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn under(path: &str, dirs: &[PathBuf]) -> bool {
    let p = path.to_lowercase();
    dirs.iter().any(|d| p.starts_with(&format!("{}\\", d.to_string_lossy().trim_end_matches('\\').to_lowercase())))
}

/// Путь лежит внутри папки (без учёта регистра, как в Windows).
pub fn inside(path: &Path, dir: &Path) -> bool {
    under(&path.to_string_lossy(), &[dir.to_path_buf()])
}

/// Идёт ли хоть один процесс из этих папок.
pub fn running_under(dirs: &[PathBuf]) -> bool {
    let me = std::process::id();
    let mut found = false;
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok && !found {
            found = stoppable(entry.th32ProcessID, me, dirs);
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    found
}

/// Процесс из этих папок, который надо остановить. Копии установщика не трогаем: из копии
/// в папке программы запускают удаление, и она ждёт свою временную копию.
fn stoppable(pid: u32, me: u32, dirs: &[PathBuf]) -> bool {
    pid != me
        && pid > 4
        && process_path(pid).is_some_and(|p| under(&p, dirs) && !Path::new(&p).file_name().is_some_and(|n| n.eq_ignore_ascii_case("klick-setup.exe")))
}

/// Завершить все процессы, запущенные из этих папок (окно kl!ck, ядро), кроме установщика.
pub fn kill_under(dirs: &[PathBuf]) -> usize {
    let me = std::process::id();
    let mut killed = 0;
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return 0 };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let pid = entry.th32ProcessID;
            if stoppable(pid, me, dirs) {
                if let Ok(h) = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid) {
                    if TerminateProcess(h, 1).is_ok() {
                        killed += 1;
                        let _ = WaitForSingleObject(h, 3000);
                    }
                    let _ = CloseHandle(h);
                }
            }
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    killed
}

/// Запустить программу без окна и дождаться; по истечении времени — завершить. Код выхода.
pub fn run(exe: &Path, args: &[&str], timeout: Duration) -> Result<i32, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    wait(cmd, exe, timeout)
}

/// То же, но строка аргументов уходит как есть, без кавычек: так их ждёт `_?=` у NSIS.
pub fn run_raw(exe: &Path, raw: &str, timeout: Duration) -> Result<i32, String> {
    let mut cmd = Command::new(exe);
    cmd.raw_arg(raw);
    wait(cmd, exe, timeout)
}

/// Запустить без окна и забрать вывод (текстом; нули UTF-16 выбрасываются).
pub fn run_capture(exe: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", exe.display()))?;
    let mut out = child.stdout.take().ok_or("нет вывода")?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut out, &mut buf);
        buf
    });
    let deadline = Instant::now() + timeout;
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(format!("{} не завершился за {} с", exe.display(), timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let bytes: Vec<u8> = reader.join().unwrap_or_default().into_iter().filter(|b| *b != 0).collect();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn wait(mut cmd: Command, exe: &Path, timeout: Duration) -> Result<i32, String> {
    let mut child = cmd
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", exe.display()))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status.code().unwrap_or(-1));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(format!("{} не завершился за {} с", exe.display(), timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// PowerShell одной командой, без окна.
pub fn powershell(script: &str, timeout: Duration) -> Result<i32, String> {
    let ps = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    run(&ps, &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script], timeout)
}

/// PowerShell в фоне, без окна и без ожидания: для уборки после выхода установщика.
pub fn spawn_powershell(script: &str) {
    let ps = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let _ = Command::new(ps)
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Строка для PowerShell в одинарных кавычках.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn system32(exe: &str) -> PathBuf {
    std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("System32").join(exe)
}

/// Запустить окно kl!ck от имени пользователя, а не администратора: через проводник.
pub fn launch_as_user(exe: &Path) -> Result<(), String> {
    let explorer = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("explorer.exe");
    Command::new(explorer).arg(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ── Ярлыки ─────────────────────────────────────────────────────────────

pub fn start_menu() -> PathBuf {
    std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| "C:\\ProgramData".into()).join(r"Microsoft\Windows\Start Menu\Programs")
}

pub fn public_desktop() -> PathBuf {
    std::env::var_os("PUBLIC").map(PathBuf::from).unwrap_or_else(|| "C:\\Users\\Public".into()).join("Desktop")
}

/// «Пуск» и рабочий стол текущего пользователя: сюда ярлыки кладут установщики «только для меня».
pub fn user_start_menu() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs"))
}

pub fn user_desktop() -> Option<PathBuf> {
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s.map(PathBuf::from)
    }
}

/// Ярлык с AppUserModelID: по нему Windows показывает уведомления от имени kl!ck.
pub fn shortcut(lnk: &Path, target: &Path, args: &str, aumid: &str) -> Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        link.SetPath(&HSTRING::from(target.as_os_str())).map_err(|e| e.to_string())?;
        link.SetArguments(&HSTRING::from(args)).map_err(|e| e.to_string())?;
        if let Some(dir) = target.parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str())).map_err(|e| e.to_string())?;
        }
        link.SetIconLocation(&HSTRING::from(target.as_os_str()), 0).map_err(|e| e.to_string())?;
        link.SetDescription(&HSTRING::from("kl!ck — VPN-клиент")).map_err(|e| e.to_string())?;
        let store: IPropertyStore = link.cast().map_err(|e| e.to_string())?;
        let value = PROPVARIANT::from(aumid);
        store.SetValue(&PKEY_AppUserModel_ID, &value).map_err(|e| e.to_string())?;
        store.Commit().map_err(|e| e.to_string())?;
        let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        file.Save(&HSTRING::from(lnk.as_os_str()), true).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Куда ведёт ярлык.
pub fn shortcut_target(lnk: &Path) -> Option<PathBuf> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link.cast().ok()?;
        file.Load(&HSTRING::from(lnk.as_os_str()), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        (end > 0).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..end])))
    }
}

/// Сказать оболочке, что значки поменялись: иначе «Пуск» и панель задач показывают старую картинку.
pub fn refresh_icons() {
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

// ── Служба ─────────────────────────────────────────────────────────────

struct Sc(SC_HANDLE);

impl Drop for Sc {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

fn open_service(name: &str, access: u32) -> Option<(Sc, Sc)> {
    unsafe {
        let manager = Sc(OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT).ok()?);
        let n = wide(name);
        let service = Sc(OpenServiceW(manager.0, PCWSTR(n.as_ptr()), access).ok()?);
        Some((manager, service))
    }
}

fn service_status(s: &Sc) -> Option<SERVICE_STATUS_CURRENT_STATE> {
    let mut st = SERVICE_STATUS::default();
    unsafe { QueryServiceStatus(s.0, &mut st).ok()? };
    Some(st.dwCurrentState)
}

/// Работает ли служба; `None` — такой службы нет.
pub fn service_running(name: &str) -> Option<bool> {
    let (_m, s) = open_service(name, SERVICE_QUERY_STATUS)?;
    Some(service_status(&s).is_some_and(|st| st != SERVICE_STOPPED))
}

/// Остановить и дождаться. `true` — остановлена или её нет.
pub fn stop_service(name: &str, timeout: Duration) -> bool {
    let Some((_m, s)) = open_service(name, SERVICE_QUERY_STATUS | SERVICE_STOP) else { return true };
    if service_status(&s) == Some(SERVICE_STOPPED) {
        return true;
    }
    let mut st = SERVICE_STATUS::default();
    unsafe {
        let _ = ControlService(s.0, SERVICE_CONTROL_STOP, &mut st);
    }
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if service_status(&s) == Some(SERVICE_STOPPED) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// Запустить и дождаться, пока заработает.
pub fn start_service(name: &str, timeout: Duration) -> Result<(), String> {
    let (_m, s) = open_service(name, SERVICE_QUERY_STATUS | SERVICE_START).ok_or_else(|| format!("служба {name} не найдена"))?;
    if let Err(e) = unsafe { StartServiceW(s.0, None) } {
        if e.code() != ERROR_SERVICE_ALREADY_RUNNING.to_hresult() {
            return Err(e.message());
        }
    }
    let deadline = Instant::now() + timeout;
    loop {
        match service_status(&s) {
            Some(SERVICE_RUNNING) => return Ok(()),
            Some(SERVICE_STOPPED) => return Err("служба остановилась сразу после запуска".into()),
            _ if Instant::now() >= deadline => return Err(format!("служба не запустилась за {} с", timeout.as_secs())),
            _ => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Убрать службу из системы. Windows удаляет её, когда закроются все ссылки на неё.
pub fn delete_service(name: &str) -> bool {
    const DELETE: u32 = 0x0001_0000;
    let Some((_m, s)) = open_service(name, DELETE) else { return true };
    unsafe { DeleteService(s.0).is_ok() }
}

// ── Файлы ──────────────────────────────────────────────────────────────

/// Удалить при следующей перезагрузке: файл занят и сейчас не удаляется.
pub fn delete_on_reboot(path: &Path) {
    let w = wide(&path.to_string_lossy());
    unsafe {
        let _ = MoveFileExW(PCWSTR(w.as_ptr()), PCWSTR::null(), MOVEFILE_DELAY_UNTIL_REBOOT);
    }
}

/// Удалить папку целиком. Файл бывает занят ещё секунду после завершения процесса —
/// поэтому несколько попыток; что так и не удалилось, Windows уберёт при перезагрузке.
/// `true` — папки больше нет.
pub fn remove_tree(dir: &Path) -> bool {
    for _ in 0..10 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    schedule_tree(dir);
    false
}

/// Сначала файлы, потом папки: при перезагрузке Windows удаляет в том же порядке.
fn schedule_tree(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                schedule_tree(&p);
            } else {
                delete_on_reboot(&p);
            }
        }
    }
    delete_on_reboot(dir);
}

/// Переименовать папку; сразу после остановки процессов файлы бывают ещё заняты.
pub fn rename_dir(from: &Path, to: &Path) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..20 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(last)
}

/// Путь из настроек Windows: вместо Program Files там бывает код известной папки.
pub fn expand_known_folder(path: &str) -> String {
    const KNOWN: [(&str, &str); 3] = [
        ("{6D809377-6AF0-444B-8957-A3773F02200E}", "ProgramW6432"),
        ("{905E63B6-C1BF-494E-B29C-65B732D3D21A}", "ProgramFiles"),
        ("{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}", "ProgramFiles(x86)"),
    ];
    for (guid, var) in KNOWN {
        if path.len() >= guid.len() && path[..guid.len()].eq_ignore_ascii_case(guid) {
            if let Ok(base) = std::env::var(var) {
                return format!("{base}{}", &path[guid.len()..]);
            }
        }
    }
    path.to_string()
}

// ── Драйвер Wintun ─────────────────────────────────────────────────────

/// Пакеты Wintun в хранилище драйверов (`oemN.inf`). Его ставит ядро mihomo, когда впервые
/// включает TUN, и Windows хранит его и после того, как адаптер пропал.
pub fn wintun_packages() -> Vec<String> {
    let dir = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("INF");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if !(name.starts_with("oem") && name.ends_with(".inf")) {
            continue;
        }
        let Ok(bytes) = std::fs::read(e.path()) else { continue };
        let text = if bytes.starts_with(&[0xFF, 0xFE]) {
            String::from_utf16_lossy(&bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        if text.to_lowercase().contains("wintun.sys") {
            out.push(name);
        }
    }
    out
}

/// Отключённые адаптеры Wintun (`SWD\WINTUN\{…}`) из вывода `pnputil /enum-devices /disconnected`.
/// Адаптер TUN пропадает вместе с ядром, но Windows помнит его как отключённое устройство,
/// и пока оно числится, драйвер из хранилища не удаляется.
pub fn wintun_ghosts(pnputil_output: &str) -> Vec<String> {
    const MARK: &str = "SWD\\WINTUN\\";
    // Только ASCII: длина строки в байтах не меняется, и позиции совпадают с исходной.
    let upper = pnputil_output.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = upper[from..].find(MARK) {
        let start = from + i;
        let end = upper[start..].find(|c: char| c.is_whitespace()).map_or(upper.len(), |j| start + j);
        let id = pnputil_output[start..end].to_string();
        if !out.contains(&id) {
            out.push(id);
        }
        from = end;
    }
    out
}

// ── Диск и права ───────────────────────────────────────────────────────

/// Свободно на диске папки (папки может ещё не быть — берём ближайшую существующую).
pub fn free_bytes(path: &Path) -> Option<u64> {
    let mut p = path.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    let w = wide(&p.to_string_lossy());
    let mut free = 0u64;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), None, None).ok()? };
    Some(free)
}

/// Папка вне Program Files: права как у Program Files — писать могут только администраторы и система.
/// Иначе обычная программа подменила бы ядро или службу и получила бы права системы.
pub fn lock_folder(dir: &Path) -> Result<(), String> {
    let icacls = system32("icacls.exe");
    let d = dir.to_string_lossy().to_string();
    run(&icacls, &[&d, "/setowner", "*S-1-5-32-544", "/T", "/C", "/Q"], Duration::from_secs(60))?;
    let code = run(
        &icacls,
        &[&d, "/inheritance:r", "/grant:r", "*S-1-5-32-544:(OI)(CI)F", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-545:(OI)(CI)RX", "/C", "/Q"],
        Duration::from_secs(60),
    )?;
    run(&icacls, &[&format!("{d}\\*"), "/reset", "/T", "/C", "/Q"], Duration::from_secs(60))?;
    if code == 0 {
        Ok(())
    } else {
        Err(format!("icacls вернул {code}"))
    }
}

pub fn under_program_files(dir: &Path) -> bool {
    let d = dir.to_string_lossy().to_lowercase();
    ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .any(|pf| d.starts_with(&format!("{}\\", pf.trim_end_matches('\\').to_lowercase())))
}

// ── Прокси пользователя ────────────────────────────────────────────────

/// Настройки прокси подключения «по локальной сети» (Wi-Fi, Ethernet), как их видит WinINet.
/// Читаем и пишем через WinINet одной операцией — так же, как окно kl!ck: тогда сразу обновляется
/// запись настроек, которую читают браузеры, а не только старые значения в реестре.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Proxy {
    pub flags: u32,
    pub server: Option<String>,
    pub bypass: Option<String>,
    pub pac: Option<String>,
}

impl Proxy {
    pub fn on(&self) -> bool {
        self.flags & PROXY_TYPE_PROXY != 0
    }
}

pub fn proxy_query() -> Option<Proxy> {
    for flags_option in [INTERNET_PER_CONN_FLAGS_UI, INTERNET_PER_CONN_FLAGS.0] {
        let mut opts = [per_conn(flags_option), per_conn(INTERNET_PER_CONN_PROXY_SERVER.0), per_conn(INTERNET_PER_CONN_PROXY_BYPASS.0), per_conn(INTERNET_PER_CONN_AUTOCONFIG_URL.0)];
        let mut list = INTERNET_PER_CONN_OPTION_LISTW {
            dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            pszConnection: PWSTR::null(),
            dwOptionCount: opts.len() as u32,
            dwOptionError: 0,
            pOptions: opts.as_mut_ptr(),
        };
        let mut size = list.dwSize;
        let ok = unsafe { InternetQueryOptionW(None, INTERNET_OPTION_PER_CONNECTION_OPTION, Some(&mut list as *mut _ as *mut std::ffi::c_void), &mut size).is_ok() };
        if ok {
            return Some(unsafe {
                Proxy { flags: opts[0].Value.dwValue, server: take_wininet(opts[1].Value.pszValue), bypass: take_wininet(opts[2].Value.pszValue), pac: take_wininet(opts[3].Value.pszValue) }
            });
        }
    }
    None
}

/// Записать одной операцией и сообщить программам, что прокси поменялся.
pub fn proxy_set(p: &Proxy) {
    let mut server = wide(p.server.as_deref().unwrap_or(""));
    let mut bypass = wide(p.bypass.as_deref().unwrap_or(""));
    let mut pac = wide(p.pac.as_deref().unwrap_or(""));
    let mut opts = [
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_FLAGS, Value: INTERNET_PER_CONN_OPTIONW_0 { dwValue: p.flags } },
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_PROXY_SERVER, Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(server.as_mut_ptr()) } },
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_PROXY_BYPASS, Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(bypass.as_mut_ptr()) } },
        INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_AUTOCONFIG_URL, Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(pac.as_mut_ptr()) } },
    ];
    let list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        pszConnection: PWSTR::null(),
        dwOptionCount: opts.len() as u32,
        dwOptionError: 0,
        pOptions: opts.as_mut_ptr(),
    };
    unsafe {
        let _ = InternetSetOptionW(None, INTERNET_OPTION_PER_CONNECTION_OPTION, Some(&list as *const _ as *const std::ffi::c_void), list.dwSize);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
    }
}

fn per_conn(id: u32) -> INTERNET_PER_CONN_OPTIONW {
    INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN(id), ..Default::default() }
}

/// Строка от WinINet: забрать и освободить.
unsafe fn take_wininet(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let s = p.to_string().ok();
    let _ = GlobalFree(HGLOBAL(p.0 as *mut std::ffi::c_void));
    s.filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wintun_ghosts_are_found_in_any_language() {
        let out = concat!(
            "Instance ID:                SWD\\WINTUN\\{0AFD3ED9-02C8-0811-124D-1682118B6574}\r\n",
            "Device Description:         klick\r\n\r\n",
            "Идентификатор экземпляра:  SWD\\Wintun\\{11111111-2222-3333-4444-555555555555}\r\n",
            "Instance ID:                USB\\VID_1\r\n",
        );
        assert_eq!(
            wintun_ghosts(out),
            vec![r"SWD\WINTUN\{0AFD3ED9-02C8-0811-124D-1682118B6574}".to_string(), r"SWD\Wintun\{11111111-2222-3333-4444-555555555555}".to_string()]
        );
        assert!(wintun_ghosts(r"Instance ID: USB\VID_1").is_empty());
    }

    #[test]
    fn tray_paths_with_known_folders_expand() {
        let pf = std::env::var("ProgramW6432").unwrap();
        assert_eq!(expand_known_folder(r"{6D809377-6AF0-444B-8957-A3773F02200E}\kl!ck\klick.exe"), format!(r"{pf}\kl!ck\klick.exe"));
        assert_eq!(expand_known_folder(r"C:\Apps\klick\klick.exe"), r"C:\Apps\klick\klick.exe");
        assert!(inside(Path::new(&format!(r"{pf}\kl!ck\klick.exe")), Path::new(&format!(r"{pf}\KL!CK"))));
        assert!(!inside(Path::new(&format!(r"{pf}\klickx\klick.exe")), Path::new(&format!(r"{pf}\klick"))));
    }
}
