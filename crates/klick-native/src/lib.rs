//! kl!ck для iOS с родным интерфейсом (SwiftUI, `crates/klick-ios/native`). Движок службы — в процессе
//! приложения, как в `crates/klick-ios`, но без Tauri и веб-окна: Swift зовёт те же команды движка, что
//! и окно (JSON klick-proto), через функции C ниже и получает события. Туннель, ядро mihomo, QR-код и ID
//! устройства — те же функции Swift и Go, что у веб-версии (`host.rs` общий).

#[path = "../../klick-ios/src/host.rs"]
mod host;

use klick_proto::Command;
use klick_service::engine::{self, EngineHandle};
use klick_service::paths::{Paths, Profile};
use serde_json::{json, Value};
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::OnceLock;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static ENGINE: OnceLock<EngineHandle> = OnceLock::new();

/// Ответ или событие — Swift: `ctx` — его контекст, `json` — живёт только до возврата из функции.
type Callback = extern "C" fn(ctx: *mut c_void, json: *const c_char, ok: bool);

/// Контекст Swift уходит в фоновую задачу и возвращается ему же: сам Rust его не трогает.
struct Ctx(*mut c_void);
unsafe impl Send for Ctx {}

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("klick-engine")
            .build()
            .expect("tokio на iOS не запустился")
    })
}

fn unreachable() -> Value {
    json!({ "code": "service.unreachable" })
}

/// Как у веб-версии: настройки, ключи и журнал — в контейнере приложения, конфиг и серверы ядра —
/// в общей папке с расширением VPN.
fn paths() -> Result<Paths, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or("нет папки приложения (HOME)")?;
    let data = home.join("Library/Application Support/klick");
    let group = host::group_dir().unwrap_or_else(|| home.join("Library/Application Support/klick-group"));
    let bundle = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)).ok_or("нет пакета приложения")?;
    Ok(Paths::ios(data, group, bundle.join("klick-resources")))
}

fn start() -> Result<(), String> {
    klick_service::ios::init(host::host());
    let paths = paths()?;
    paths.ensure_dirs().map_err(|e| format!("{e:#}"))?;
    let _ = klick_service::init_file_log(&paths);
    let _guard = runtime().enter();
    let (engine, _task) = engine::spawn(paths, Profile::ios()).map_err(|e| format!("{e:#}"))?;
    let _ = ENGINE.set(engine);
    Ok(())
}

fn to_c(text: String) -> CString {
    CString::new(text).unwrap_or_else(|_| CString::new("null").unwrap())
}

/// Поднять движок. NULL — поднялся (или уже работает), иначе текст ошибки — освободить
/// `klick_native_free`. Звать не с главного потока: Swift ждёт ответов NetworkExtension через него.
#[no_mangle]
pub extern "C" fn klick_native_start() -> *mut c_char {
    if ENGINE.get().is_some() {
        return std::ptr::null_mut();
    }
    match start() {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => to_c(e).into_raw(),
    }
}

async fn call(text: String) -> Result<Value, Value> {
    let engine = ENGINE.get().ok_or_else(unreachable)?;
    let cmd: Command = serde_json::from_str(&text).map_err(|_| json!({ "code": "proto.bad_request" }))?;
    let result = if cmd == Command::IpCheck { engine.ip_check().await } else { engine.call(cmd).await };
    result.map_err(|e| serde_json::to_value(e).unwrap_or_else(|_| unreachable()))
}

/// Команда движку: `request` — `{"cmd": "...", "args": {...}}`, как в klick-proto. Ответ — в `cb` с
/// другого потока: `ok` — JSON результата, иначе JSON ошибки `{"code": ..., "params": ...}`.
///
/// # Safety
/// `request` — строка C (UTF-8) или NULL.
#[no_mangle]
pub unsafe extern "C" fn klick_native_call(request: *const c_char, ctx: *mut c_void, cb: Callback) {
    let text = if request.is_null() { String::new() } else { CStr::from_ptr(request).to_string_lossy().into_owned() };
    let ctx = Ctx(ctx);
    runtime().spawn(async move {
        let ctx = ctx;
        let (body, ok) = match call(text).await {
            Ok(v) => (v.to_string(), true),
            Err(e) => (e.to_string(), false),
        };
        let body = to_c(body);
        cb(ctx.0, body.as_ptr(), ok);
    });
}

/// События движка (состояние, скорость, уведомления, «настройки изменились») — в `cb` по одному JSON
/// klick-proto `Event`. `false` — движок ещё не поднят.
#[no_mangle]
pub extern "C" fn klick_native_subscribe(ctx: *mut c_void, cb: Callback) -> bool {
    let Some(engine) = ENGINE.get() else { return false };
    let mut events = engine.events.subscribe();
    let ctx = Ctx(ctx);
    runtime().spawn(async move {
        let ctx = ctx;
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match events.recv().await {
                Ok(ev) => {
                    if let Ok(text) = serde_json::to_string(&ev) {
                        let text = to_c(text);
                        cb(ctx.0, text.as_ptr(), true);
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
    true
}

/// Строка, которую отдал Rust (`klick_native_start`).
///
/// # Safety
/// `p` — указатель из `klick_native_start` или NULL, освобождается один раз.
#[no_mangle]
pub unsafe extern "C" fn klick_native_free(p: *mut c_char) {
    if !p.is_null() {
        drop(CString::from_raw(p));
    }
}

/// Swift (`Host.swift`): туннель подключился или остановился.
fn tunnel_changed(connected: bool, by_user: bool) {
    if !connected {
        klick_service::ios::tunnel_stopped(by_user);
        return;
    }
    // VPN включили в Настройках (или он поднялся сам по требованию), пока приложение открыто.
    if let Some(engine) = ENGINE.get().cloned() {
        runtime().spawn(async move {
            let _ = engine.call(Command::Connect).await;
        });
    }
}
