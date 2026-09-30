//! Канал управления: окно (или утилита) ↔ служба.
//!
//! Одно сообщение — одна строка JSON. Окно присылает намерения, служба отвечает и
//! шлёт события. Служба не присылает человеческих фраз: только коды и параметры,
//! переводит интерфейс.

use klick_core::{Appearance, ExitAction, Mode, Route, Routing, Rule, ServerDownPolicy, SubInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Канал рабочей службы.
pub const PIPE: &str = r"\\.\pipe\klick";
/// Канал службы, запущенной для разработки в консоли.
pub const PIPE_DEV: &str = r"\\.\pipe\klick-dev";
/// macOS: Unix-сокет рабочей службы. Создавать файлы в `/var/run` может только root,
/// поэтому подменить службу обычная программа не может.
pub const SOCKET: &str = "/var/run/klick.sock";
/// macOS: сокет службы, запущенной для разработки в консоли.
pub const SOCKET_DEV: &str = "/tmp/klick-dev.sock";

/// Канал управления рабочей службы на этой системе.
pub const CONTROL: &str = if cfg!(windows) { PIPE } else { SOCKET };
/// Канал управления службы для разработки на этой системе.
pub const CONTROL_DEV: &str = if cfg!(windows) { PIPE_DEV } else { SOCKET_DEV };

/// Сообщение от окна.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub cmd: Command,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]
pub enum Command {
    /// Снимок состояния.
    Status,
    /// Получать события по этому соединению.
    Subscribe,
    Connect,
    Disconnect,
    SetMode { mode: Mode },
    SetRouting { routing: Routing },
    /// Ссылка на подписку или одиночная ссылка.
    AddConnection { source: String, name: Option<String> },
    /// Содержимое файла конфигурации: окно читает файл само, служба пути от пользователя не открывает.
    ImportFile { file_name: String, content: String },
    RefreshConnection { id: String },
    RemoveConnection { id: String },
    SelectConnection { id: String },
    /// Серверы активного подключения.
    Servers,
    SelectServer { name: String },
    /// Проверить задержку всех серверов активного подключения.
    TestLatency,
    ListAdd { position: Routing, rule: Rule },
    ListRemove { position: Routing, index: usize },
    ListSetRoute { position: Routing, index: usize, route: Route },
    /// Тумблер правила: выключенное остаётся в списке, но не работает.
    ListSetEnabled { position: Routing, index: usize, enabled: bool },
    KillSwitchSet { enabled: bool },
    /// Папка программы или путь к её exe: служба сама найдёт папку установки.
    KillSwitchAdd { folder: String },
    KillSwitchRemove { folder: String },
    /// Свой переключатель у программы в списке.
    KillSwitchProgram { folder: String, enabled: bool },
    /// Программы Kill Switch и сколько exe в каждой папке нашлось: 0 — программа не найдена.
    KillSwitchStatus,
    /// Окно запустилось после входа в систему: вернуть VPN, если он был включён и включено «Восстанавливать подключение».
    Resume,
    /// Версии, порт, папка данных, система — для «О приложении» и «Продвинутых».
    About,
    /// Журнал службы: последние записи, без адресов сайтов.
    Log,
    LogClear,
    /// «Скопировать отчёт»: текст для поддержки, без ссылок и адресов сайтов.
    Report,
    /// Есть ли новая версия на GitHub.
    CheckUpdate,
    /// Настройки поведения: меняются только переданные поля.
    SetPreferences { prefs: Preferences },
    /// «Как вас видят сайты»: IP, страна, провайдер через VPN и напрямую, проверка утечек.
    IpCheck,
    /// «Сейчас в сети»: открытые соединения.
    Connections,
    /// «Не открывается?»: недавние неудачные соединения.
    Failures,
    /// Соседи на компьютере: zapret, GoodbyeDPI, другие VPN.
    Neighbors,
    /// Все настройки целиком (без секретов).
    Settings,
    /// Каталог сервисов для списков: имена, домены, подсети.
    Catalog,
    /// Программы с открытыми соединениями — для выбора в список и в Kill Switch.
    Programs,
    /// Понятные названия программ по папкам правил: описание главного exe.
    ProgramNames { folders: Vec<String> },
}

/// Программа Kill Switch с тем, что нашлось на диске.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KsProgramView {
    pub folder: String,
    pub enabled: bool,
    /// Сколько exe в папке; 0 — папки нет или программа удалена.
    pub exes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AboutView {
    pub version: String,
    /// Версия ядра, например `v1.19.31`; `None`, если ядро не ответило.
    pub core_version: Option<String>,
    pub mixed_port: u16,
    pub data_dir: String,
    /// «Windows 11 · 24H2 · x64», «macOS 15.1 Sequoia · Apple Silicon».
    pub os: String,
    /// Служба для разработки.
    pub dev: bool,
}

/// Запись журнала службы.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    /// Миллисекунды Unix.
    pub at: i64,
    /// `error`, `warn`, `info`, `debug`.
    pub level: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateView {
    pub current: String,
    /// Последняя версия на GitHub; `None`, если выпусков нет.
    pub latest: Option<String>,
    pub url: Option<String>,
    pub newer: bool,
}

/// Программа из «Запущено сейчас».
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramView {
    /// Описание из exe, иначе имя файла.
    pub name: String,
    pub path: String,
    /// Папка, которую займёт правило; `None`, если программа лежит в общей папке вроде «Загрузок».
    pub folder: Option<String>,
    /// Сколько соединений у программы открыто сейчас.
    pub connections: u32,
}

/// Одна колонка блока «Как вас видят сайты».
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IpColumn {
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    /// Примерный: базы геолокации ошибаются.
    pub city: Option<String>,
    pub provider: Option<String>,
    pub asn: Option<u32>,
    /// Обратное DNS-имя адреса.
    pub reverse_dns: Option<String>,
    /// «Сайты могут понять, что это VPN»: по базе одного сервиса, у других может отличаться.
    pub vpn_detected: Option<bool>,
    pub datacenter: Option<bool>,
    /// Код ошибки, если колонку проверить не удалось.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IpReport {
    /// `None`, когда VPN выключен.
    pub via_vpn: Option<IpColumn>,
    pub direct: IpColumn,
    /// Уходит ли IPv6 мимо туннеля. Проверяется только в режиме VPN (TUN) при «Всё через VPN».
    pub ipv6_leak: Option<bool>,
    /// Отвечает ли на DNS-запросы программ сам kl!ck. Проверяется только в режиме VPN (TUN).
    pub dns_protected: Option<bool>,
    pub checked_at: i64,
}

/// Соединение из «Сейчас в сети».
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnView {
    pub host: String,
    pub process: Option<String>,
    /// Полный путь к исполняемому файлу: из него окно делает правило для программы.
    #[serde(default)]
    pub process_path: Option<String>,
    /// `vpn`, `direct` или `block`.
    pub route: String,
    /// Какое правило сработало: `Match`, `RuleSet`, `ProcessPathRegex`…
    pub rule: String,
    pub network: String,
    pub upload: u64,
    pub download: u64,
}

/// Сосед на компьютере, который перехватывает тот же трафик.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeighborView {
    /// `dpi_bypass`, `vpn`, `proxy_core`, `vpn_adapter`, `windivert`.
    pub kind: String,
    /// `zapret`, `GoodbyeDPI`, `WireGuard`, имя адаптера…
    pub name: String,
    /// Мешает ли режиму VPN (TUN).
    pub conflicts_with_tun: bool,
}

/// Неудачное соединение из «Не открывается?».
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FailureView {
    pub host: String,
    pub route: String,
    pub error: String,
    pub at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    /// «Заблокированное в РФ — через VPN» в положении «Только выбранное».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_preset: Option<bool>,
    /// «.ru и .рф напрямую» в положении «Всё через VPN».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ru_domains: Option<bool>,
    /// «Российские IP напрямую» в положении «Всё через VPN».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ru_ips: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_server_down: Option<ServerDownPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore_on_logon: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_exit: Option<ExitAction>,
    /// `ru` или `en`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Сообщение от службы.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    Res {
        id: u64,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<ErrorInfo>,
    },
    Event(Event),
}

impl ServerMsg {
    pub fn ok(id: u64, data: Value) -> Self {
        ServerMsg::Res { id, ok: true, data: Some(data), error: None }
    }

    pub fn err(id: u64, error: ErrorInfo) -> Self {
        ServerMsg::Res { id, ok: false, data: None, error: Some(error) }
    }
}

/// Ошибка кодом: `sub.download_failed`, `core.start_failed`, `input.bad_domain`…
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

impl ErrorInfo {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into(), params: Value::Null }
    }

    pub fn with(code: impl Into<String>, params: Value) -> Self {
        Self { code: code.into(), params }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum Event {
    State { state: StateView },
    /// Скорость, байт в секунду.
    Traffic { up: u64, down: u64 },
    /// Уведомление кодом: `vpn.down`, `vpn.restored`, `server.switched`…
    Notice {
        code: String,
        #[serde(default, skip_serializing_if = "Value::is_null")]
        params: Value,
    },
    /// Окну: включить системный прокси для текущего пользователя.
    ProxyApply { host: String, port: u16, bypass: Vec<String> },
    /// Окну: снять системный прокси.
    ProxyClear,
    /// Настройки изменились — в любом окне или из консоли: окна перечитывают их, иначе главное
    /// окно и окно трея показывают разное.
    Settings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnState {
    #[default]
    Off,
    Connecting,
    Connected,
    Reconnecting,
    ServerDown,
    Error,
}

/// Всё, что нужно окну и трею, чтобы нарисовать состояние.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateView {
    pub vpn: VpnState,
    pub mode: Mode,
    pub routing: Routing,
    pub kill_switch: bool,
    pub connection: Option<ConnectionView>,
    pub server: Option<String>,
    /// Когда подключились, секунды Unix.
    pub since: Option<i64>,
    /// «Переподключаюсь… 2 из 3».
    pub attempt: Option<(u8, u8)>,
    /// Код ошибки, если `vpn = error`.
    pub error: Option<String>,
    /// Каким должен быть системный прокси пользователя сейчас; `None` — снят.
    /// Окно сверяет с ним реестр при каждом состоянии: так пропущенное событие ничего не ломает.
    #[serde(default)]
    pub system_proxy: Option<SystemProxy>,
}

/// Системный прокси, который ставит kl!ck: на Windows — окно (у каждого пользователя свой),
/// на macOS — служба (настройки сети там общие и требуют прав администратора).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxy {
    pub host: String,
    pub port: u16,
    /// Куда ходить мимо прокси: `localhost`, `192.168.*`, `<local>`.
    pub bypass: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionView {
    pub id: String,
    pub name: String,
    pub info: Option<SubInfo>,
}

/// Сервер из списка.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerView {
    pub name: String,
    /// Протокол: vless, hysteria2, trojan…
    pub kind: String,
    /// Задержка через сервер в миллисекундах; `None` — не проверяли или нет ответа.
    pub delay: Option<u32>,
    pub selected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use klick_core::Target;

    #[test]
    fn request_wire_format() {
        let r = Request { id: 7, cmd: Command::SetMode { mode: Mode::SysProxy } };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"id":7,"cmd":"set_mode","args":{"mode":"sys_proxy"}}"#);
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), r);
        let status: Request = serde_json::from_str(r#"{"id":1,"cmd":"status"}"#).unwrap();
        assert_eq!(status.cmd, Command::Status);
        let programs: Request = serde_json::from_str(r#"{"id":2,"cmd":"programs"}"#).unwrap();
        assert_eq!(programs.cmd, Command::Programs);
    }

    #[test]
    fn rule_in_request() {
        let r = Request {
            id: 2,
            cmd: Command::ListAdd {
                position: Routing::Selected,
                rule: Rule { target: Target::Domain("claude.ai".into()), route: Route::Vpn, enabled: true },
            },
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains(r#""target":{"kind":"domain","value":"claude.ai"}"#), "{s}");
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), r);
    }

    #[test]
    fn server_messages() {
        let e = ServerMsg::Event(Event::Traffic { up: 1, down: 2 });
        assert_eq!(serde_json::to_string(&e).unwrap(), r#"{"t":"event","ev":"traffic","up":1,"down":2}"#);
        let err = ServerMsg::err(3, ErrorInfo::new("core.start_failed"));
        assert_eq!(serde_json::to_string(&err).unwrap(), r#"{"t":"res","id":3,"ok":false,"error":{"code":"core.start_failed"}}"#);
    }
}
