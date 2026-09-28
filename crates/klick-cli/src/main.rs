//! Утилита для проверки службы без окна: `klick-cli status`, `klick-cli connect`, `klick-cli watch`…

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use klick_core::{Mode, Route, Routing, Rule, ServerDownPolicy, Target};
use klick_proto::{Command, Preferences, Request, ServerMsg};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

#[derive(Parser)]
#[command(name = "klick-cli", about = "Управление службой kl!ck из консоли")]
struct Cli {
    /// Говорить с рабочей службой, а не со службой для разработки.
    #[arg(long, global = true)]
    prod: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Состояние VPN.
    Status,
    /// Показывать события, пока не нажат Ctrl+C.
    Watch,
    /// Все настройки (без ссылок подписок).
    Settings,
    /// Добавить подписку или одиночную ссылку.
    Add {
        source: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Импортировать файл конфигурации mihomo/Clash.
    Import { file: std::path::PathBuf },
    /// Обновить подписку.
    Refresh { id: String },
    /// Удалить подключение.
    Remove { id: String },
    /// Сделать подключение активным.
    Use { id: String },
    /// Серверы активного подключения.
    Servers,
    /// Проверить задержку серверов.
    Latency,
    /// Выбрать сервер по имени.
    Server { name: String },
    Connect,
    Disconnect,
    /// «Как вас видят сайты».
    Ip,
    /// «Сейчас в сети».
    Conns,
    /// «Не открывается?».
    Failures,
    /// Соседи: zapret, другие VPN.
    Neighbors,
    /// Каталог сервисов.
    Catalog,
    /// Программы с открытыми соединениями.
    Programs,
    /// Названия программ по папкам.
    Names { folders: Vec<String> },
    /// Вернуть VPN после перезагрузки, если он был включён (так делает окно после входа).
    Resume,
    /// Версии, порт, папка данных.
    About,
    /// Журнал службы.
    Log,
    /// Очистить журнал в памяти.
    LogClear,
    /// Отчёт для поддержки.
    Report,
    /// Есть ли новая версия.
    Update,
    /// Режим: tun или proxy.
    Mode { mode: ModeArg },
    /// Тумблер: all (всё через VPN) или selected (только выбранное).
    Routing { routing: RoutingArg },
    /// Списки тумблера.
    List {
        #[command(subcommand)]
        op: ListOp,
    },
    /// Kill Switch.
    Ks {
        #[command(subcommand)]
        op: KsOp,
    },
    /// Настройки поведения: переключатели России, поведение при недоступном сервере, язык.
    Prefs {
        /// «Заблокированное в РФ — через VPN».
        #[arg(long)]
        blocked: Option<Toggle>,
        #[arg(long)]
        ru_domains: Option<Toggle>,
        #[arg(long)]
        ru_ips: Option<Toggle>,
        #[arg(long)]
        server_down: Option<DownArg>,
        #[arg(long)]
        restore: Option<Toggle>,
        #[arg(long)]
        auto_update: Option<Toggle>,
        #[arg(long)]
        notify: Option<Toggle>,
        #[arg(long)]
        lang: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Toggle {
    On,
    Off,
}

#[derive(Clone, Copy, ValueEnum)]
enum DownArg {
    Reconnect,
    Next,
    Fastest,
}

#[derive(Subcommand)]
enum ListOp {
    Add { position: RoutingArg, kind: KindArg, value: String, #[arg(default_value = "vpn")] route: RouteArg },
    Rm { position: RoutingArg, index: usize },
    Route { position: RoutingArg, index: usize, route: RouteArg },
    /// Тумблер правила: выключенное остаётся в списке.
    Toggle { position: RoutingArg, index: usize, state: Toggle },
}

#[derive(Subcommand)]
enum KsOp {
    On,
    Off,
    Add { folder: String },
    Rm { folder: String },
    /// Свой переключатель программы.
    Program { folder: String, state: Toggle },
    /// Программы и сколько exe нашлось в их папках.
    Status,
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    Tun,
    Proxy,
}

#[derive(Clone, Copy, ValueEnum)]
enum RoutingArg {
    All,
    Selected,
}

#[derive(Clone, Copy, ValueEnum)]
enum KindArg {
    Service,
    Program,
    Domain,
    Ip,
}

#[derive(Clone, Copy, ValueEnum)]
enum RouteArg {
    Vpn,
    Direct,
    Block,
}

impl From<RoutingArg> for Routing {
    fn from(r: RoutingArg) -> Self {
        match r {
            RoutingArg::All => Routing::AllVpn,
            RoutingArg::Selected => Routing::Selected,
        }
    }
}

impl From<RouteArg> for Route {
    fn from(r: RouteArg) -> Self {
        match r {
            RouteArg::Vpn => Route::Vpn,
            RouteArg::Direct => Route::Direct,
            RouteArg::Block => Route::Block,
        }
    }
}

fn command(cmd: Cmd) -> Result<Command> {
    Ok(match cmd {
        Cmd::Status | Cmd::Watch => Command::Status,
        Cmd::Settings => Command::Settings,
        Cmd::Add { source, name } => Command::AddConnection { source, name },
        Cmd::Import { file } => Command::ImportFile {
            file_name: file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            content: std::fs::read_to_string(&file).with_context(|| format!("не прочитать {}", file.display()))?,
        },
        Cmd::Refresh { id } => Command::RefreshConnection { id },
        Cmd::Remove { id } => Command::RemoveConnection { id },
        Cmd::Use { id } => Command::SelectConnection { id },
        Cmd::Servers => Command::Servers,
        Cmd::Latency => Command::TestLatency,
        Cmd::Server { name } => Command::SelectServer { name },
        Cmd::Connect => Command::Connect,
        Cmd::Disconnect => Command::Disconnect,
        Cmd::Ip => Command::IpCheck,
        Cmd::Conns => Command::Connections,
        Cmd::Failures => Command::Failures,
        Cmd::Neighbors => Command::Neighbors,
        Cmd::Catalog => Command::Catalog,
        Cmd::Programs => Command::Programs,
        Cmd::Names { folders } => Command::ProgramNames { folders },
        Cmd::Resume => Command::Resume,
        Cmd::About => Command::About,
        Cmd::Log => Command::Log,
        Cmd::LogClear => Command::LogClear,
        Cmd::Report => Command::Report,
        Cmd::Update => Command::CheckUpdate,
        Cmd::Mode { mode } => Command::SetMode { mode: match mode { ModeArg::Tun => Mode::Tun, ModeArg::Proxy => Mode::SysProxy } },
        Cmd::Routing { routing } => Command::SetRouting { routing: routing.into() },
        Cmd::List { op } => match op {
            ListOp::Add { position, kind, value, route } => {
                let target = match kind {
                    KindArg::Service => Target::Service(value),
                    KindArg::Program => Target::Program(value),
                    KindArg::Domain => Target::Domain(value),
                    KindArg::Ip => Target::Ip(value),
                };
                Command::ListAdd { position: position.into(), rule: Rule { target, route: route.into(), enabled: true } }
            }
            ListOp::Rm { position, index } => Command::ListRemove { position: position.into(), index },
            ListOp::Route { position, index, route } => Command::ListSetRoute { position: position.into(), index, route: route.into() },
            ListOp::Toggle { position, index, state } => Command::ListSetEnabled { position: position.into(), index, enabled: matches!(state, Toggle::On) },
        },
        Cmd::Ks { op } => match op {
            KsOp::On => Command::KillSwitchSet { enabled: true },
            KsOp::Off => Command::KillSwitchSet { enabled: false },
            KsOp::Add { folder } => Command::KillSwitchAdd { folder },
            KsOp::Rm { folder } => Command::KillSwitchRemove { folder },
            KsOp::Program { folder, state } => Command::KillSwitchProgram { folder, enabled: matches!(state, Toggle::On) },
            KsOp::Status => Command::KillSwitchStatus,
        },
        Cmd::Prefs { blocked, ru_domains, ru_ips, server_down, restore, auto_update, notify, lang } => {
            let on = |t: Option<Toggle>| t.map(|t| matches!(t, Toggle::On));
            Command::SetPreferences {
                prefs: Preferences {
                    blocked_preset: on(blocked),
                    ru_domains: on(ru_domains),
                    ru_ips: on(ru_ips),
                    on_server_down: server_down.map(|d| match d {
                        DownArg::Reconnect => ServerDownPolicy::Reconnect,
                        DownArg::Next => ServerDownPolicy::NextWorking,
                        DownArg::Fastest => ServerDownPolicy::Fastest,
                    }),
                    restore_on_logon: on(restore),
                    auto_update: on(auto_update),
                    notify: on(notify),
                    appearance: None,
                    on_exit: None,
                    language: lang,
                },
            }
        }
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let pipe = if cli.prod { klick_proto::PIPE } else { klick_proto::PIPE_DEV };
    let watch = matches!(cli.cmd, Cmd::Watch);
    let cmd = if watch { Command::Subscribe } else { command(cli.cmd)? };

    let client = ClientOptions::new().open(pipe).with_context(|| format!("служба не отвечает на {pipe}"))?;
    let (read, mut write) = tokio::io::split(client);
    let mut lines = BufReader::new(read).lines();
    let req = Request { id: 1, cmd };
    write.write_all(format!("{}\n", serde_json::to_string(&req)?).as_bytes()).await?;

    let timeout = if watch { Duration::from_secs(u64::MAX / 4) } else { Duration::from_secs(120) };
    loop {
        let line = tokio::time::timeout(timeout, lines.next_line()).await.context("служба не ответила")??;
        let Some(line) = line else { bail!("служба закрыла канал") };
        match serde_json::from_str::<ServerMsg>(&line)? {
            ServerMsg::Res { ok, data, error, .. } => {
                if ok {
                    match data.unwrap_or_default() {
                        serde_json::Value::String(text) => println!("{text}"),
                        other => println!("{}", serde_json::to_string_pretty(&other)?),
                    }
                } else {
                    let e = error.map(|e| serde_json::to_string(&e).unwrap_or_default()).unwrap_or_default();
                    eprintln!("ошибка: {e}");
                    if !watch {
                        std::process::exit(1);
                    }
                }
                if !watch {
                    return Ok(());
                }
            }
            ServerMsg::Event(ev) => println!("{}", serde_json::to_string(&ev)?),
        }
    }
}
