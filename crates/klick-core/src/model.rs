//! Модель настроек. Это то, что пользователь меняет в интерфейсе, и то, что служба хранит на диске.

use crate::os::Os;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Как трафик попадает в ядро.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Виртуальный адаптер: все программы, включая игры и UDP.
    #[default]
    Tun,
    /// Системный прокси Windows: только программы, которые его слушаются.
    SysProxy,
}

/// Положение тумблера «Всё через VPN / Только выбранное».
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Routing {
    AllVpn,
    #[default]
    Selected,
}

/// Куда отправить совпавший трафик.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Vpn,
    Direct,
    Block,
}

/// Что именно попадает под правило.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Target {
    /// Сервис из каталога, например `youtube`.
    Service(String),
    /// Программа: папка установки целиком, со всеми exe внутри.
    Program(String),
    /// Домен со всеми поддоменами; `ru` — вся зона.
    Domain(String),
    /// Адрес или подсеть в записи CIDR.
    Ip(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub target: Target,
    pub route: Route,
    /// Выключенное правило остаётся в списке, но не работает.
    #[serde(default = "yes")]
    pub enabled: bool,
}

/// У каждого положения тумблера свой список. Второй сохраняется, пока включено первое.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lists {
    /// Для «Всё через VPN»: обычно то, что идёт напрямую.
    pub all_vpn: Vec<Rule>,
    /// Для «Только выбранное»: обычно то, что идёт через VPN.
    pub selected: Vec<Rule>,
}

impl Lists {
    pub fn get(&self, routing: Routing) -> &[Rule] {
        match routing {
            Routing::AllVpn => &self.all_vpn,
            Routing::Selected => &self.selected,
        }
    }

    pub fn get_mut(&mut self, routing: Routing) -> &mut Vec<Rule> {
        match routing {
            Routing::AllVpn => &mut self.all_vpn,
            Routing::Selected => &mut self.selected,
        }
    }
}

/// Kill Switch по программам.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KillSwitch {
    pub enabled: bool,
    pub programs: Vec<KsProgram>,
}

/// Программа в Kill Switch: папка установки и свой переключатель.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "KsProgramRepr")]
pub struct KsProgram {
    pub folder: String,
    pub enabled: bool,
}

impl From<&str> for KsProgram {
    fn from(folder: &str) -> Self {
        KsProgram { folder: folder.into(), enabled: true }
    }
}

/// Раньше программа хранилась просто строкой-папкой: такие настройки тоже читаются.
#[derive(Deserialize)]
#[serde(untagged)]
enum KsProgramRepr {
    Folder(String),
    Full {
        folder: String,
        #[serde(default = "yes")]
        enabled: bool,
    },
}

impl From<KsProgramRepr> for KsProgram {
    fn from(r: KsProgramRepr) -> Self {
        match r {
            KsProgramRepr::Folder(folder) => KsProgram { folder, enabled: true },
            KsProgramRepr::Full { folder, enabled } => KsProgram { folder, enabled },
        }
    }
}

fn yes() -> bool {
    true
}

/// Оформление окна. Настройки у ПК одни, поэтому и тема живёт в службе.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// `system`, `light`, `dark` или `custom`.
    pub theme: String,
    /// Основа своей темы: `graphite`, `midnight`, `oled`, `light`.
    pub base: String,
    /// Цвет акцента своей темы, `#rrggbb`.
    pub accent: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { theme: "system".into(), base: "graphite".into(), accent: "#30d158".into() }
    }
}

impl Appearance {
    pub fn is_valid(&self) -> bool {
        let hex = self.accent.len() == 7 && self.accent.starts_with('#') && self.accent[1..].chars().all(|c| c.is_ascii_hexdigit());
        ["system", "light", "dark", "custom"].contains(&self.theme.as_str()) && ["graphite", "midnight", "oled", "light"].contains(&self.base.as_str()) && hex
    }
}

impl Default for KillSwitch {
    fn default() -> Self {
        Self { enabled: true, programs: Vec::new() }
    }
}

/// Быстрые исключения положения «Всё через VPN»: что из России идёт напрямую.
/// Локальная сеть идёт напрямую всегда, без переключателя.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RussiaDirect {
    /// Домены .ru, .рф и российские сервисы на других доменах.
    pub domains: bool,
    /// Российские IP по базе стран.
    pub ips: bool,
}

impl Default for RussiaDirect {
    fn default() -> Self {
        Self { domains: true, ips: true }
    }
}

/// Что делать, когда сервер перестал отвечать.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerDownPolicy {
    /// Три попытки за 15 секунд, потом уведомление и тихая проверка раз в минуту.
    #[default]
    Reconnect,
    /// Переключиться на следующий рабочий сервер.
    NextWorking,
    /// Всегда держаться самого быстрого сервера.
    Fastest,
}

/// «Выход» из трея: спросить или сделать запомненное.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitAction {
    #[default]
    Ask,
    /// Отключить VPN и выйти.
    Disconnect,
    /// Выйти, VPN продолжает работать.
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    /// Ссылка на подписку: список серверов, срок, трафик.
    Subscription,
    /// Одиночная ссылка вида `vless://`.
    Link,
    /// Файл конфигурации mihomo/Clash.
    File,
}

/// Сведения о подписке из заголовка `subscription-userinfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubInfo {
    pub upload: u64,
    pub download: u64,
    /// Лимит в байтах; 0 — без лимита.
    pub total: u64,
    /// Окончание в секундах Unix.
    pub expire: Option<i64>,
}

/// Подключение: подписка, ссылка или файл. Сама ссылка — секрет и хранится отдельно.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub kind: ConnectionKind,
    #[serde(default)]
    pub info: Option<SubInfo>,
    /// Когда список серверов обновлялся в последний раз, секунды Unix.
    #[serde(default)]
    pub updated_at: Option<i64>,
    /// Как часто обновлять, если панель попросила; иначе раз в 12 часов.
    #[serde(default)]
    pub update_interval_hours: Option<u32>,
    #[serde(default)]
    pub selected_server: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub mode: Mode,
    pub routing: Routing,
    pub lists: Lists,
    /// «Заблокированное в РФ — через VPN»: готовый набор в положении «Только выбранное».
    pub blocked_preset: bool,
    pub russia_direct: RussiaDirect,
    pub kill_switch: KillSwitch,
    pub connections: Vec<Connection>,
    pub active_connection: Option<String>,
    pub on_server_down: ServerDownPolicy,
    /// «Восстанавливать подключение»: после перезагрузки VPN включится сам, если был включён.
    pub restore_on_logon: bool,
    /// «Обновлять подписки» по расписанию.
    pub auto_update: bool,
    /// «Уведомлять об обрывах»: уведомления Windows показывает окно.
    pub notify: bool,
    pub appearance: Appearance,
    pub on_exit: ExitAction,
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            mode: Mode::Tun,
            routing: Routing::Selected,
            lists: Lists::default(),
            blocked_preset: true,
            russia_direct: RussiaDirect::default(),
            kill_switch: KillSwitch::default(),
            connections: Vec::new(),
            active_connection: None,
            on_server_down: ServerDownPolicy::Reconnect,
            restore_on_logon: false,
            auto_update: true,
            notify: true,
            appearance: Appearance::default(),
            on_exit: ExitAction::Ask,
            language: "ru".into(),
        }
    }
}

impl Settings {
    /// Работающие правила текущего положения: выключенные в списке пропускаются.
    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.lists.get(self.routing).iter().filter(|r| r.enabled)
    }

    pub fn active(&self) -> Option<&Connection> {
        let id = self.active_connection.as_deref()?;
        self.connections.iter().find(|c| c.id == id)
    }

    pub fn active_mut(&mut self) -> Option<&mut Connection> {
        let id = self.active_connection.clone()?;
        self.connections.iter_mut().find(|c| c.id == id)
    }
}

/// Сервис из каталога: набор доменов и подсетей под одним понятным именем.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub cidrs: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub services: Vec<Service>,
}

impl Catalog {
    pub fn find(&self, id: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.id == id)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InputError {
    #[error("empty")]
    Empty,
    #[error("bad_domain")]
    BadDomain,
    #[error("bad_ip")]
    BadIp,
    #[error("bad_path")]
    BadPath,
    #[error("folder_too_broad")]
    TooBroad,
}

impl InputError {
    /// Код для интерфейса: служба говорит кодами, окно переводит.
    pub fn code(&self) -> &'static str {
        match self {
            InputError::Empty => "input.empty",
            InputError::BadDomain => "input.bad_domain",
            InputError::BadIp => "input.bad_ip",
            InputError::BadPath => "input.bad_path",
            InputError::TooBroad => "input.folder_too_broad",
        }
    }
}

/// Приводит домен к виду, который понимает ядро: без схемы, пути и точки в начале,
/// в нижнем регистре, кириллица — в punycode (`рф` → `xn--p1ai`).
pub fn normalize_domain(input: &str) -> Result<String, InputError> {
    let mut s = input.trim().to_lowercase();
    if s.is_empty() {
        return Err(InputError::Empty);
    }
    if let Some(rest) = s.split_once("://").map(|(_, r)| r.to_string()) {
        s = rest;
    }
    if let Some(i) = s.find(['/', '?', '#']) {
        s.truncate(i);
    }
    if let Some(i) = s.rfind(':') {
        if s[i + 1..].chars().all(|c| c.is_ascii_digit()) {
            s.truncate(i);
        }
    }
    let s = s.trim_start_matches("*.").trim_start_matches("+.").trim_matches('.');
    if s.is_empty() {
        return Err(InputError::BadDomain);
    }
    let mut labels = Vec::new();
    for label in s.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(InputError::BadDomain);
        }
        let ascii = if label.is_ascii() { label.to_string() } else { punycode_label(label)? };
        if !ascii.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || ascii.starts_with('-') || ascii.ends_with('-') {
            return Err(InputError::BadDomain);
        }
        labels.push(ascii);
    }
    Ok(labels.join("."))
}

/// Подсеть или адрес: `149.154.160.0/20`, `2001:b28:f23d::/48`, `1.1.1.1`.
pub fn normalize_cidr(input: &str) -> Result<String, InputError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(InputError::Empty);
    }
    let (addr, prefix) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    let ip: IpAddr = addr.parse().map_err(|_| InputError::BadIp)?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    let prefix = match prefix {
        Some(p) => p.parse::<u8>().ok().filter(|p| *p <= max).ok_or(InputError::BadIp)?,
        None => max,
    };
    Ok(format!("{ip}/{prefix}"))
}

/// Папка программы на Windows: абсолютный путь без завершающего слэша. На macOS — [`crate::macos::normalize_folder`].
pub fn normalize_folder(input: &str) -> Result<String, InputError> {
    let s = input.trim().trim_matches('"').replace('/', "\\");
    let s = s.trim_end_matches('\\');
    if s.is_empty() {
        return Err(InputError::Empty);
    }
    let bytes = s.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    let unc = s.starts_with("\\\\");
    if !(drive || unc) || s.contains("..") {
        return Err(InputError::BadPath);
    }
    Ok(s.to_string())
}

/// Папка программы для правила и Kill Switch на той системе, под которую собрана программа.
pub fn program_folder(input: &str) -> Result<String, InputError> {
    program_folder_on(Os::CURRENT, input, None)
}

/// Папка программы для правила и Kill Switch. `is_file` — подсказка службы, которая видит диск:
/// на Windows exe узнаётся по расширению, а на macOS у программы без пакета `.app` расширения нет.
pub fn program_folder_on(os: Os, input: &str, is_file: Option<bool>) -> Result<String, InputError> {
    match os {
        Os::Windows => windows_program_folder(input),
        Os::MacOs => crate::macos::program_folder(input, is_file),
    }
}

/// Windows: из пути к exe берётся папка установки: папки
/// версий (`app-1.0.9175`, `131.0.2903.70`, `version-6f8e…`) и `Versions` над ними пропускаются,
/// чтобы правило пережило обновление программы. Корень диска, Program Files, «Загрузки»,
/// профиль и другие общие папки не подходят: правило задело бы чужие программы.
fn windows_program_folder(input: &str) -> Result<String, InputError> {
    let path = normalize_folder(input)?;
    let mut folder = path.clone();
    if path.to_ascii_lowercase().ends_with(".exe") {
        folder = parent(&path).ok_or(InputError::BadPath)?.to_string();
        let mut stripped = false;
        while let Some(up) = parent(&folder) {
            let name = last_segment(&folder);
            if !is_version_dir(name) && !(stripped && name.eq_ignore_ascii_case("versions")) {
                break;
            }
            folder = up.to_string();
            stripped = true;
        }
    }
    if is_broad_folder(&folder) {
        return Err(InputError::TooBroad);
    }
    Ok(folder)
}

fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('\\').map(|(p, _)| p).filter(|p| !p.is_empty())
}

fn last_segment(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

/// `app-1.0.9175`, `131.0.2903.70`, `v2.4.1`, `version-6f8e1a2b3c4d5e6f`.
pub(crate) fn is_version_dir(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if let Some(hex) = lower.strip_prefix("version-") {
        return !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    let rest = lower.strip_prefix("app-").or_else(|| lower.strip_prefix('v')).unwrap_or(&lower);
    let number = rest.split(['-', '_', ' ']).next().unwrap_or("");
    number.starts_with(|c: char| c.is_ascii_digit()) && number.contains('.') && number.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Папки, где лежит много чужих программ.
fn is_broad_folder(folder: &str) -> bool {
    const BROAD: [&str; 20] = [
        "program files", "program files (x86)", "programdata", "windows", "system32", "users", "appdata", "local", "locallow", "roaming",
        "programs", "desktop", "downloads", "documents", "music", "pictures", "videos", "onedrive", "temp", "tmp",
    ];
    let parts: Vec<&str> = folder.split('\\').collect();
    if folder.starts_with("\\\\") {
        return parts.len() <= 4;
    }
    parts.len() <= 1 || (parts.len() == 3 && parts[1].eq_ignore_ascii_case("users")) || BROAD.contains(&last_segment(folder).to_lowercase().as_str())
}

/// Punycode одной метки домена по RFC 3492 с префиксом `xn--`.
fn punycode_label(label: &str) -> Result<String, InputError> {
    const BASE: u32 = 36;
    const T_MIN: u32 = 1;
    const T_MAX: u32 = 26;
    const SKEW: u32 = 38;
    const DAMP: u32 = 700;
    const INITIAL_BIAS: u32 = 72;
    const INITIAL_N: u32 = 128;

    fn adapt(mut delta: u32, points: u32, first: bool) -> u32 {
        delta /= if first { DAMP } else { 2 };
        delta += delta / points;
        let mut k = 0;
        while delta > ((BASE - T_MIN) * T_MAX) / 2 {
            delta /= BASE - T_MIN;
            k += BASE;
        }
        k + (BASE - T_MIN + 1) * delta / (delta + SKEW)
    }
    fn digit(d: u32) -> char {
        (if d < 26 { b'a' + d as u8 } else { b'0' + (d - 26) as u8 }) as char
    }

    let input: Vec<u32> = label.chars().map(|c| c as u32).collect();
    let mut output: String = label.chars().filter(|c| c.is_ascii()).collect();
    let basic = output.chars().count() as u32;
    let mut handled = basic;
    if basic > 0 {
        output.push('-');
    }
    let (mut n, mut delta, mut bias) = (INITIAL_N, 0u32, INITIAL_BIAS);
    while (handled as usize) < input.len() {
        let m = *input.iter().filter(|&&c| c >= n).min().ok_or(InputError::BadDomain)?;
        delta = delta.checked_add((m - n).checked_mul(handled + 1).ok_or(InputError::BadDomain)?).ok_or(InputError::BadDomain)?;
        n = m;
        for &c in &input {
            if c < n {
                delta = delta.checked_add(1).ok_or(InputError::BadDomain)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias { T_MIN } else if k >= bias + T_MAX { T_MAX } else { k - bias };
                    if q < t {
                        break;
                    }
                    output.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                output.push(digit(q));
                bias = adapt(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Ok(format!("xn--{output}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Примеры в тестах ниже — пути Windows; macOS проверяется в `crate::macos`.
    fn program_folder(p: &str) -> Result<String, InputError> {
        program_folder_on(Os::Windows, p, None)
    }

    #[test]
    fn domains_are_normalized() {
        assert_eq!(normalize_domain("https://WWW.YouTube.com/watch?v=1").unwrap(), "www.youtube.com");
        assert_eq!(normalize_domain(".ru").unwrap(), "ru");
        assert_eq!(normalize_domain("*.discord.gg").unwrap(), "discord.gg");
        assert_eq!(normalize_domain("example.com:8443").unwrap(), "example.com");
        assert_eq!(normalize_domain("рф").unwrap(), "xn--p1ai");
        assert_eq!(normalize_domain("госуслуги.рф").unwrap(), "xn--c1aapkosapc.xn--p1ai");
        assert_eq!(normalize_domain("яндекс.рф").unwrap(), "xn--d1acpjx3f.xn--p1ai");
        assert_eq!(normalize_domain("пример.испытание").unwrap(), "xn--e1afmkfd.xn--80akhbyknj4f");
        assert_eq!(normalize_domain("  "), Err(InputError::Empty));
        assert_eq!(normalize_domain("bad_domain!.com"), Err(InputError::BadDomain));
    }

    #[test]
    fn cidrs_are_normalized() {
        assert_eq!(normalize_cidr("149.154.160.0/20").unwrap(), "149.154.160.0/20");
        assert_eq!(normalize_cidr("1.1.1.1").unwrap(), "1.1.1.1/32");
        assert_eq!(normalize_cidr("2001:b28:f23d::/48").unwrap(), "2001:b28:f23d::/48");
        assert_eq!(normalize_cidr("10.0.0.0/33"), Err(InputError::BadIp));
        assert_eq!(normalize_cidr("example.com"), Err(InputError::BadIp));
    }

    #[test]
    fn folders_are_normalized() {
        assert_eq!(
            normalize_folder("\"C:/Users/a/AppData/Local/Discord/\"").unwrap(),
            "C:\\Users\\a\\AppData\\Local\\Discord"
        );
        assert_eq!(normalize_folder("Discord"), Err(InputError::BadPath));
        assert_eq!(normalize_folder("C:\\a\\..\\b"), Err(InputError::BadPath));
    }

    #[test]
    fn program_folder_skips_versions() {
        let ok = |p: &str| program_folder(p).unwrap();
        assert_eq!(ok(r"C:\Users\a\AppData\Local\Discord\app-1.0.9175\Discord.exe"), r"C:\Users\a\AppData\Local\Discord");
        assert_eq!(ok(r"C:\Users\a\AppData\Local\Roblox\Versions\version-6f8e1a2b3c4d5e6f\RobloxPlayerBeta.exe"), r"C:\Users\a\AppData\Local\Roblox");
        assert_eq!(ok(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"), r"C:\Program Files (x86)\Microsoft\Edge\Application");
        assert_eq!(ok(r"C:\Users\a\AppData\Roaming\Telegram Desktop\Telegram.EXE"), r"C:\Users\a\AppData\Roaming\Telegram Desktop");
        assert_eq!(ok(r"C:\Games\Some\Versions\game.exe"), r"C:\Games\Some\Versions");
        assert_eq!(ok(r"D:\Games\Steam"), r"D:\Games\Steam");
    }

    #[test]
    fn program_folder_refuses_broad_folders() {
        assert_eq!(program_folder(r"C:\Users\a\Downloads\tool.exe"), Err(InputError::TooBroad));
        assert_eq!(program_folder(r"C:\tool.exe"), Err(InputError::TooBroad));
        assert_eq!(program_folder(r"C:\Program Files\app.exe"), Err(InputError::TooBroad));
        assert_eq!(program_folder(r"C:\Users\a"), Err(InputError::TooBroad));
        assert_eq!(program_folder(r"C:\Users\a\AppData\Local\Programs"), Err(InputError::TooBroad));
    }

    #[test]
    fn settings_survive_missing_fields() {
        let s: Settings = serde_json::from_str(r#"{"mode":"sys_proxy"}"#).unwrap();
        assert_eq!(s.mode, Mode::SysProxy);
        assert_eq!(s.routing, Routing::Selected);
        assert!(s.kill_switch.enabled);
        assert!(s.auto_update && s.notify);
        assert_eq!(s.appearance, Appearance::default());
    }

    #[test]
    fn kill_switch_reads_old_and_new_programs() {
        let ks: KillSwitch = serde_json::from_str(r#"{"enabled":true,"programs":["C:\\A",{"folder":"C:\\B","enabled":false},{"folder":"C:\\C"}]}"#).unwrap();
        assert_eq!(ks.programs, vec![KsProgram::from(r"C:\A"), KsProgram { folder: r"C:\B".into(), enabled: false }, KsProgram::from(r"C:\C")]);
        assert_eq!(serde_json::to_value(&ks.programs[1]).unwrap(), serde_json::json!({ "folder": r"C:\B", "enabled": false }));
    }

    #[test]
    fn appearance_is_checked() {
        assert!(Appearance::default().is_valid());
        assert!(!Appearance { theme: "neon".into(), ..Appearance::default() }.is_valid());
        assert!(!Appearance { accent: "red".into(), ..Appearance::default() }.is_valid());
    }
}
