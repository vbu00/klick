//! Мост окна к службе: команды — отдельным подключением к каналу, события — постоянным.
//! Windows — именованный канал, macOS — Unix-сокет.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
#[cfg(windows)]
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

#[cfg(windows)]
const ERROR_PIPE_BUSY: i32 = 231;
/// Канал рабочей службы (как `klick_proto::PIPE`); служба для разработки — без проверки.
#[cfg(windows)]
const WORK_PIPE: &str = r"\\.\pipe\klick";

pub struct Bridge {
    pipe: String,
    up: AtomicBool,
    next_id: AtomicU64,
}

impl Bridge {
    pub fn new(pipe: String) -> Self {
        Self { pipe, up: AtomicBool::new(false), next_id: AtomicU64::new(1) }
    }
}

#[cfg(windows)]
async fn open(pipe: &str) -> std::io::Result<NamedPipeClient> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match ClientOptions::new().open(pipe) {
            // Рабочий канал держит служба от имени системы. Кто занял имя раньше неё, — чужой:
            // ему не отдаём ни ссылки подписок, ни команды.
            Ok(c) if pipe == WORK_PIPE && !served_by_system(&c) => {
                return Err(std::io::Error::other("канал kl!ck держит не служба kl!ck"));
            }
            Ok(c) => return Ok(c),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Канал создала служба kl!ck: его владелец — система или администраторы. Так выглядит канал,
/// который служба (LocalSystem) создаёт при запуске. Программа обычного пользователя, занявшая имя
/// раньше службы, владеет своим каналом сама и назначить владельцем систему не может.
///
/// Владельца видно любому, кто подключился к каналу: нужно только право READ_CONTROL, а оно входит
/// в обычный доступ на чтение. Процесс службы при этом не открываем — обычному пользователю Windows
/// его не откроет, и проверка по процессу отвергла бы настоящую службу.
#[cfg(windows)]
fn served_by_system(pipe: &NamedPipeClient) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
    use windows::Win32::Security::{IsWellKnownSid, WinBuiltinAdministratorsSid, WinLocalSystemSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID};
    unsafe {
        let mut owner = PSID::default();
        let mut sd = PSECURITY_DESCRIPTOR::default();
        let rc = GetSecurityInfo(HANDLE(pipe.as_raw_handle() as _), SE_KERNEL_OBJECT, OWNER_SECURITY_INFORMATION, Some(&mut owner), None, None, None, Some(&mut sd));
        if rc.is_err() || owner.is_invalid() {
            return false;
        }
        let ok = IsWellKnownSid(owner, WinLocalSystemSid).as_bool() || IsWellKnownSid(owner, WinBuiltinAdministratorsSid).as_bool();
        let _ = LocalFree(HLOCAL(sd.0));
        ok
    }
}

/// macOS: сокет службы лежит в /var/run, куда пишет только root, — занять его имя чужая программа не может.
#[cfg(unix)]
async fn open(socket: &str) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(socket).await
}

fn unreachable() -> Value {
    json!({ "code": "service.unreachable" })
}

/// Команда службе. `request` — `{ "cmd": "...", "args": {...} }`, как в klick-proto.
#[tauri::command]
pub async fn service_call(bridge: State<'_, Bridge>, request: Value) -> Result<Value, Value> {
    let id = bridge.next_id.fetch_add(1, Ordering::Relaxed);
    #[cfg(debug_assertions)]
    eprintln!("[bridge] {}", request["cmd"]);
    let mut req = request;
    req["id"] = json!(id);
    let client = open(&bridge.pipe).await.map_err(|_| unreachable())?;
    let (read, mut write) = tokio::io::split(client);
    write.write_all(format!("{req}\n").as_bytes()).await.map_err(|_| unreachable())?;
    let mut lines = BufReader::new(read).lines();
    loop {
        let line = tokio::time::timeout(Duration::from_secs(120), lines.next_line())
            .await
            .map_err(|_| json!({ "code": "service.timeout" }))?
            .map_err(|_| unreachable())?
            .ok_or_else(unreachable)?;
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        if msg["t"] == "res" && msg["id"] == json!(id) {
            return if msg["ok"] == json!(true) { Ok(msg["data"].clone()) } else { Err(msg["error"].clone()) };
        }
    }
}

#[tauri::command]
pub fn service_up(bridge: State<'_, Bridge>) -> bool {
    bridge.up.load(Ordering::Relaxed)
}

fn set_up(app: &AppHandle, up: bool) {
    let bridge = app.state::<Bridge>();
    if bridge.up.swap(up, Ordering::Relaxed) != up {
        let _ = app.emit("klick://service", json!({ "up": up }));
    }
}

/// Состояние службы — значку трея и системному прокси.
fn on_state(app: &AppHandle, state: &Value) {
    crate::tray::update(app, state);
    let desired = serde_json::from_value::<Option<crate::sysproxy::Spec>>(state["system_proxy"].clone()).ok().flatten();
    crate::sysproxy::reconcile(desired);
}

/// Постоянная подписка на события службы; если службы нет, пробуем снова каждые 2 секунды.
pub fn start_events(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let pipe = app.state::<Bridge>().pipe.clone();
            match open(&pipe).await {
                Ok(client) => {
                    let (read, mut write) = tokio::io::split(client);
                    if write.write_all(b"{\"id\":0,\"cmd\":\"subscribe\"}\n").await.is_ok() {
                        set_up(&app, true);
                        let mut lines = BufReader::new(read).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                            if msg["t"] == "res" && msg["ok"] == json!(true) {
                                on_state(&app, &msg["data"]);
                            } else if msg["t"] == "event" {
                                match msg["ev"].as_str() {
                                    Some("state") => on_state(&app, &msg["state"]),
                                    Some("proxy_clear") => crate::sysproxy::reconcile(None),
                                    _ => {}
                                }
                                let _ = app.emit("klick://event", &msg);
                            }
                        }
                    }
                    set_up(&app, false);
                    crate::sysproxy::service_lost();
                    crate::tray::service_down(&app);
                }
                Err(_) => {
                    set_up(&app, false);
                    crate::tray::service_down(&app);
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Живая служба kl!ck на этом компьютере — из обычного процесса, без прав администратора
    /// (так работает окно): `cargo test -p klick-ui live_service -- --ignored`.
    #[test]
    #[ignore]
    fn live_service_pipe_is_trusted() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let c = ClientOptions::new().open(WORK_PIPE).expect("служба kl!ck не запущена");
            assert!(served_by_system(&c), "канал службы должен пройти проверку");
        });
    }

    /// Канал, созданный этим (обычным) процессом, проверку не проходит.
    #[test]
    fn own_pipe_is_not_trusted() {
        use tokio::net::windows::named_pipe::ServerOptions;
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let name = format!(r"\\.\pipe\klick-owner-test-{}", std::process::id());
            let _server = ServerOptions::new().first_pipe_instance(true).create(&name).unwrap();
            let c = ClientOptions::new().open(&name).unwrap();
            let elevated = is_elevated();
            // От администратора с повышенными правами владелец и у своего канала — «Администраторы»:
            // такой процесс и так может всё. Проверка защищает от обычных программ пользователя.
            assert_eq!(served_by_system(&c), elevated);
        });
    }

    fn is_elevated() -> bool {
        std::process::Command::new("net").arg("session").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
    }
}
