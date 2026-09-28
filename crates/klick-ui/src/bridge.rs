//! Мост окна к службе: команды — отдельным подключением к каналу, события — постоянным.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

const ERROR_PIPE_BUSY: i32 = 231;

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

async fn open(pipe: &str) -> std::io::Result<NamedPipeClient> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match ClientOptions::new().open(pipe) {
            Ok(c) => return Ok(c),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            Err(e) => return Err(e),
        }
    }
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
