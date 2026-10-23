//! kl!ck для iOS: то же окно (`ui/`), движок службы — в процессе приложения (`klick-service`), ядро VPN —
//! в расширении Network Extension (`gen/apple/KlickTunnel`), ядро mihomo — библиотекой (`core/`).
//! Как устроено и почему — docs/ios-port.md.

#[path = "../../klick-ui/src/deeplink.rs"]
mod deeplink;
mod host;

use klick_proto::{Command, ServerMsg};
use klick_service::engine::{self, EngineHandle};
use klick_service::paths::{Paths, Profile};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

/// Движок службы. Поднимается в фоне: до этого окно видит «служба не отвечает» и ждёт `klick://service`.
static ENGINE: OnceLock<EngineHandle> = OnceLock::new();

fn unreachable() -> Value {
    json!({ "code": "service.unreachable" })
}

/// Команда движку. `request` — `{ "cmd": "...", "args": {...} }`, как в klick-proto.
#[tauri::command]
async fn service_call(request: Value) -> Result<Value, Value> {
    let engine = ENGINE.get().ok_or_else(unreachable)?;
    let cmd: Command = serde_json::from_value(request).map_err(|_| json!({ "code": "proto.bad_request" }))?;
    let result = if cmd == Command::IpCheck { engine.ip_check().await } else { engine.call(cmd).await };
    result.map_err(|e| serde_json::to_value(e).unwrap_or_else(|_| unreachable()))
}

#[tauri::command]
fn service_up() -> bool {
    ENGINE.get().is_some()
}

#[tauri::command]
fn clipboard_text() -> String {
    host::clipboard()
}

/// Для `deeplink.rs`: на iOS главное окно и так на экране.
fn show_main(_app: &AppHandle) {}

/// Настройки, ключи и журнал — в контейнере приложения; конфиг и серверы для ядра — в общей папке
/// с расширением VPN. Нет общей папки (подпись без App Group) — окно работает, но туннель не поднимется:
/// Swift скажет почему.
fn paths() -> Result<Paths, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or("нет папки приложения (HOME)")?;
    let data = home.join("Library/Application Support/klick");
    let group = host::group_dir().unwrap_or_else(|| home.join("Library/Application Support/klick-group"));
    let bundle = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)).ok_or("нет пакета приложения")?;
    Ok(Paths::ios(data, group, bundle.join("klick-resources")))
}

fn start_engine(app: AppHandle) -> Result<(), String> {
    let paths = paths()?;
    paths.ensure_dirs().map_err(|e| format!("{e:#}"))?;
    let _ = klick_service::init_file_log(&paths);
    let (engine, _task) = engine::spawn(paths, Profile::ios()).map_err(|e| format!("{e:#}"))?;
    let mut events = engine.events.subscribe();
    let _ = ENGINE.set(engine);
    let forward = app.clone();
    tauri::async_runtime::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match events.recv().await {
                Ok(ev) => {
                    let _ = forward.emit("klick://event", ServerMsg::Event(ev));
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
    let _ = app.emit("klick://service", json!({ "up": true }));
    if let Some(sub) = std::env::var("KLICK_SELFTEST_SUB").ok().filter(|s| !s.is_empty()) {
        tauri::async_runtime::spawn(self_test(sub));
    }
    Ok(())
}

/// Самопроверка в симуляторе (scripts/ios/sim-test.sh): подписка из окружения запуска, список серверов
/// и задержка через каждый — это ядро mihomo в самом приложении (туннеля в симуляторе нет). Итог — строкой
/// «самопроверка: …» в журнал; ссылку подписки в журнал не пишем.
async fn self_test(sub: String) {
    let Some(engine) = ENGINE.get() else { return };
    let result = async {
        let id = match engine.call(Command::AddConnection { source: sub, name: Some("Проверка".into()) }).await {
            Ok(view) => view["id"].as_str().unwrap_or_default().to_string(),
            Err(e) if e.code == "conn.exists" => e.params["id"].as_str().unwrap_or_default().to_string(),
            Err(e) => return Err(e),
        };
        engine.call(Command::SelectConnection { id }).await?;
        engine.call(Command::TestLatency).await
    }
    .await;
    match result {
        Ok(Value::Array(servers)) => {
            let mut kinds = std::collections::BTreeMap::<String, (u32, u32)>::new();
            for s in &servers {
                let entry = kinds.entry(s["kind"].as_str().unwrap_or("?").to_string()).or_default();
                entry.0 += 1;
                entry.1 += u32::from(s["delay"].is_u64());
            }
            let summary: Vec<String> = kinds.iter().map(|(kind, (all, alive))| format!("{kind} {alive}/{all}")).collect();
            tracing::info!("самопроверка: серверов {}, отвечают: {}", servers.len(), summary.join(", "));
        }
        Ok(other) => tracing::warn!("самопроверка: неожиданный ответ {other}"),
        Err(e) => tracing::warn!("самопроверка: ошибка {}", e.code),
    }
}

/// Swift: туннель подключился или остановился.
fn tunnel_changed(connected: bool, by_user: bool) {
    if !connected {
        klick_service::ios::tunnel_stopped(by_user);
        return;
    }
    // VPN включили в Настройках (или он поднялся сам по требованию), пока приложение открыто: движок
    // подключается к туннелю. Если туннель поднял сам движок, команда ничего не делает.
    if let Some(engine) = ENGINE.get().cloned() {
        tauri::async_runtime::spawn(async move {
            let _ = engine.call(Command::Connect).await;
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(deeplink::Pending::default())
        .invoke_handler(tauri::generate_handler![service_call, service_up, clipboard_text, deeplink::take_pending_link])
        // Окно на весь экран — когда страница загрузилась и WKWebView точно есть.
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.with_webview(|w| host::setup_webview(w.inner()));
            }
        })
        .setup(|app| {
            klick_service::ios::init(host::host());
            // Не на главном потоке: Swift ждёт ответов NetworkExtension, а они приходят через главный поток.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = start_engine(handle) {
                    eprintln!("движок kl!ck не запустился: {e}");
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("kl!ck не запустился")
        .run(|app, event| {
            // Ссылка klick://add со страницы подписки.
            if let tauri::RunEvent::Opened { urls } = event {
                for url in urls {
                    deeplink::open(app, url.as_str());
                }
            }
        });
}
