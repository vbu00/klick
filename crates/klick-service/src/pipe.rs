//! Канал управления: строка JSON на сообщение, запросы по очереди, события — после `subscribe`.
//! Windows — именованный канал, macOS — Unix-сокет.

use crate::engine::EngineHandle;
use klick_proto::{Command, ErrorInfo, Request, ServerMsg};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};
#[cfg(windows)]
use {
    crate::win::PipeSecurity,
    tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions},
};

/// `secure` — рабочая служба: доступ только системе, администраторам и вошедшим пользователям.
/// Для разработки канал создаётся с правами по умолчанию (владелец — текущий пользователь).
#[cfg(windows)]
pub async fn serve(name: String, secure: bool, engine: EngineHandle) -> anyhow::Result<()> {
    let mut security = if secure { Some(PipeSecurity::interactive_users()?) } else { None };
    let mut server = create(&name, true, &mut security)?;
    tracing::info!("канал управления {name}");
    loop {
        server.connect().await?;
        let client = server;
        server = create(&name, false, &mut security)?;
        tokio::spawn(handle(client, engine.clone()));
    }
}

#[cfg(windows)]
fn create(name: &str, first: bool, security: &mut Option<Box<PipeSecurity>>) -> std::io::Result<NamedPipeServer> {
    let mut opts = ServerOptions::new();
    opts.first_pipe_instance(first).reject_remote_clients(true);
    match security {
        Some(s) => unsafe { opts.create_with_security_attributes_raw(name, s.as_ptr()) },
        None => opts.create(name),
    }
}

/// macOS: Unix-сокет. `secure` — рабочая служба: сокет root:staff 0660, то есть доступ у root
/// и у всех, кто входит в Mac (обычные пользователи состоят в группе staff). В `/var/run` файлы
/// создаёт только root, поэтому подменить службу другая программа не может.
/// Для разработки под sudo сокет открыт всем, иначе — только владельцу.
#[cfg(unix)]
pub async fn serve(name: String, secure: bool, engine: EngineHandle) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    let path = Path::new(&name);
    if path.exists() {
        if tokio::net::UnixStream::connect(path).await.is_ok() {
            anyhow::bail!("служба уже запущена: {name} отвечает");
        }
        std::fs::remove_file(path)?;
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    if secure {
        #[cfg(target_os = "macos")]
        const STAFF: u32 = 20;
        #[cfg(target_os = "macos")]
        {
            std::os::unix::fs::chown(path, Some(0), Some(STAFF))?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
        }
        #[cfg(not(target_os = "macos"))]
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
    } else if crate::sys::is_elevated() {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
    }
    tracing::info!("канал управления {name}");
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(handle(stream, engine.clone()));
    }
}

async fn handle<S: AsyncRead + AsyncWrite + Send + 'static>(pipe: S, engine: EngineHandle) {
    let (read, mut write) = tokio::io::split(pipe);
    let (out_tx, mut out_rx) = mpsc::channel::<String>(512);
    let writer = tokio::spawn(async move {
        while let Some(line) = out_rx.recv().await {
            if write.write_all(line.as_bytes()).await.is_err() || write.write_all(b"\n").await.is_err() {
                break;
            }
        }
    });

    let mut lines = BufReader::new(read).lines();
    let mut forward: Option<tokio::task::JoinHandle<()>> = None;
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let msg = match serde_json::from_str::<Request>(&line) {
            Ok(req) => {
                if req.cmd == Command::Subscribe && forward.is_none() {
                    let mut rx = engine.events.subscribe();
                    let tx = out_tx.clone();
                    forward = Some(tokio::spawn(async move {
                        loop {
                            match rx.recv().await {
                                Ok(ev) => {
                                    let Ok(text) = serde_json::to_string(&ServerMsg::Event(ev)) else { continue };
                                    if tx.send(text).await.is_err() {
                                        break;
                                    }
                                }
                                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                Err(broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    }));
                }
                let result = if req.cmd == Command::IpCheck { engine.ip_check().await } else { engine.call(req.cmd).await };
                match result {
                    Ok(data) => ServerMsg::ok(req.id, data),
                    Err(e) => ServerMsg::err(req.id, e),
                }
            }
            Err(_) => ServerMsg::err(0, ErrorInfo::new("proto.bad_request")),
        };
        let Ok(text) = serde_json::to_string(&msg) else { continue };
        if out_tx.send(text).await.is_err() {
            break;
        }
    }
    if let Some(f) = forward {
        f.abort();
    }
    drop(out_tx);
    let _ = writer.await;
}
