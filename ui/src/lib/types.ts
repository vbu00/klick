// Типы канала управления: те же, что в klick-proto на стороне службы.

export type Mode = 'tun' | 'sys_proxy';
export type Routing = 'all_vpn' | 'selected';
export type Route = 'vpn' | 'direct' | 'block';
export type VpnState = 'off' | 'connecting' | 'connected' | 'reconnecting' | 'server_down' | 'error';
export type ConnectionKind = 'subscription' | 'link' | 'file';

export interface SubInfo {
  upload: number;
  download: number;
  /** Лимит в байтах; 0 — без лимита. */
  total: number;
  /** Окончание, секунды Unix. */
  expire: number | null;
  /** Объявление панели (в том числе причина отказа). */
  announce?: string | null;
  /** Поддержка, только https. */
  support_url?: string | null;
  /** Страница подписки, только https. */
  page_url?: string | null;
  /** Панель принимает итоги проверки серверов: тогда можно включить отправку. */
  probe_report?: number;
}

export interface ConnectionView {
  id: string;
  name: string;
  info: SubInfo | null;
  /** Отправлять итоги проверки серверов панели подписки. */
  probe_share?: boolean;
  /** Последняя проверка серверов: живёт до смены сети или перезапуска службы. */
  probe_last?: { checked_at: number; report: ProbeReport } | null;
}

/** Проверка серверов из этой сети: работают ли они или провайдер их режет. */
export type ProbeVerdict = 'ok' | 'freeze' | 'tcp_blocked' | 'handshake_failed' | 'no_answer';

/** Итог проверки одного сервера; необязательных полей нет, если их не мерили. */
export interface ServerProbe {
  verdict: ProbeVerdict;
  /** Нестабильно: часть попыток прошла. */
  flaky: boolean;
  tcp_ok?: number;
  tcp_tries?: number;
  tcp_ms?: number;
  delay_ms?: number;
  dl_bytes?: number;
  dl_stalled?: boolean;
  /** Ответ DNS провайдера против настоящего: different — подменён, bogus — подставной адрес. */
  dns?: 'same' | 'different' | 'bogus' | 'nxdomain';
}

export interface ProbeReport {
  network: {
    /** Шла ли проверка при включённом VPN (TUN). */
    vpn: boolean;
    /** Прямые проверки TCP и DNS сделаны (без VPN (TUN)). */
    direct_checked: boolean;
    asn?: number;
    /** Оператор и регион — только когда включена отправка панели. */
    operator?: string;
    country?: string;
    region?: string;
    net_type?: string;
  };
  /** Сколько серверов проверено. */
  servers: number;
  /** Отправка панели; нет поля — не отправляли. */
  shared?: 'sent' | 'off' | 'later' | 'failed';
}

export interface StateView {
  vpn: VpnState;
  mode: Mode;
  routing: Routing;
  kill_switch: boolean;
  connection: ConnectionView | null;
  server: string | null;
  since: number | null;
  attempt: [number, number] | null;
  error: string | null;
  /** Каким должен быть системный прокси; null — снят. */
  system_proxy?: { host: string; port: number; bypass: string[] } | null;
  /** Вышла новая версия, которую можно поставить в один клик. */
  update?: string | null;
}

export interface Connection {
  id: string;
  name: string;
  kind: ConnectionKind;
  info: SubInfo | null;
  updated_at: number | null;
  update_interval_hours: number | null;
  selected_server: string | null;
  /** Отправлять итоги проверки серверов панели подписки. */
  probe_share?: boolean;
}

export type Target =
  | { kind: 'service'; value: string }
  | { kind: 'program'; value: string }
  | { kind: 'domain'; value: string }
  | { kind: 'ip'; value: string };

export interface Rule {
  target: Target;
  route: Route;
  /** Выключенное правило остаётся в списке, но не работает. */
  enabled?: boolean;
}

export interface Settings {
  mode: Mode;
  routing: Routing;
  /** Свой список у каждого положения тумблера. */
  lists: Record<Routing, Rule[]>;
  /** «Заблокированное в РФ — через VPN» в положении «Только выбранное». */
  blocked_preset?: boolean;
  connections: Connection[];
  active_connection: string | null;
  kill_switch: KillSwitch;
  russia_direct: { domains: boolean; ips: boolean };
  on_server_down: ServerDown;
  restore_on_logon: boolean;
  auto_update: boolean;
  notify: boolean;
  /** «Серверы по странам»: список серверов в окне сгруппирован по странам. Старая служба поле не присылает. */
  servers_by_country?: boolean;
  appearance: Appearance;
  on_exit: ExitAction;
  language: string;
}

export type ExitAction = 'ask' | 'disconnect' | 'keep';

export type ServerDown = 'reconnect' | 'next_working' | 'fastest';

export interface KsProgram {
  folder: string;
  enabled: boolean;
}

export interface KillSwitch {
  enabled: boolean;
  programs: KsProgram[];
}

/** Программа Kill Switch с тем, что нашлось на диске: exes = 0 — программа не найдена. */
export interface KsProgramView extends KsProgram {
  exes: number;
  /** Режим системного прокси: программа шла мимо прокси kl!ck и осталась без сети — ей нужен TUN. */
  no_proxy?: boolean;
}

export type ThemeMode = 'system' | 'light' | 'dark' | 'custom';
export type ThemeBase = 'graphite' | 'midnight' | 'oled' | 'light';

export interface Appearance {
  theme: ThemeMode;
  base: ThemeBase;
  accent: string;
}

/** Что можно поменять в настройках поведения: передаются только изменённые поля. */
export interface Preferences {
  blocked_preset?: boolean;
  ru_domains?: boolean;
  ru_ips?: boolean;
  on_server_down?: ServerDown;
  restore_on_logon?: boolean;
  auto_update?: boolean;
  notify?: boolean;
  servers_by_country?: boolean;
  appearance?: Appearance;
  on_exit?: ExitAction;
  language?: string;
}

export interface AboutView {
  version: string;
  core_version: string | null;
  mixed_port: number;
  data_dir: string;
  os: string;
  dev: boolean;
}

export interface LogLine {
  at: number;
  level: 'error' | 'warn' | 'info' | 'debug' | 'trace';
  text: string;
}

/** Итог «Проверить скорость», Мбит/с. */
export interface SpeedView {
  server: string | null;
  down: number | null;
  up: number | null;
}

export interface UpdateView {
  current: string;
  latest: string | null;
  url: string | null;
  newer: boolean;
  /** Ставится в один клик (install_update); иначе — скачать по url. */
  installable?: boolean;
}

export interface ServerView {
  name: string;
  /** Протокол: vless, hysteria2, trojan… */
  kind: string;
  /** Задержка через сервер, мс; null — не проверяли или нет ответа. */
  delay: number | null;
  selected: boolean;
  /** Итог последней проверки серверов; нет — не проверяли. */
  probe?: ServerProbe;
  /** Скачивание через сервер по последнему замеру скорости, Мбит/с. */
  speed?: number | null;
}

/** Сервис из каталога: понятное имя для набора доменов и подсетей. */
export interface Service {
  id: string;
  name: string;
  domains: string[];
  cidrs: string[];
}

/** Программа из «Запущено сейчас». */
export interface ProgramView {
  name: string;
  path: string;
  /** Папка, которую займёт правило; null — программа лежит в общей папке вроде «Загрузок». */
  folder: string | null;
  connections: number;
}

/** Соединение из «Сейчас в сети». */
export interface ConnView {
  host: string;
  process: string | null;
  process_path: string | null;
  route: Route;
  rule: string;
  network: string;
  upload: number;
  download: number;
}

/** Неудачное соединение из «Не открывается?». */
export interface FailureView {
  host: string;
  route: Route;
  error: string;
  at: number;
}

/** Колонка «Как вас видят сайты». */
export interface IpColumn {
  ipv4: string | null;
  ipv6: string | null;
  country: string | null;
  country_code: string | null;
  city: string | null;
  provider: string | null;
  asn: number | null;
  reverse_dns: string | null;
  vpn_detected: boolean | null;
  datacenter: boolean | null;
  error: string | null;
}

export interface IpReport {
  /** null, когда VPN выключен. */
  via_vpn: IpColumn | null;
  direct: IpColumn;
  ipv6_leak: boolean | null;
  dns_protected: boolean | null;
  checked_at: number;
}

export interface ErrorInfo {
  code: string;
  params?: Record<string, unknown>;
}

/** Ссылка `klick://add`, которую окно ещё не показало. `url` нет — ссылка не прошла проверки. */
export interface PendingLink {
  url: string | null;
  name: string | null;
  /** Домен подписки: его показываем вместо всей ссылки — токен в ней секрет. */
  host: string | null;
}

export type KEvent =
  | { ev: 'state'; state: StateView }
  | { ev: 'traffic'; up: number; down: number }
  | { ev: 'notice'; code: string; params?: Record<string, unknown> }
  | { ev: 'proxy_apply'; host: string; port: number; bypass: string[] }
  | { ev: 'proxy_clear' }
  /** Настройки изменились (в этом окне, в другом или из консоли) — перечитать. */
  | { ev: 'settings' }
  /** Задержку серверов подключения измерили — в этом окне или в другом. */
  | { ev: 'servers'; connection: string }
  /** Ход обновления в один клик: скачано `got` из `total` байт, потом проверка подписи и установка. */
  | { ev: 'update_progress'; stage: 'download' | 'verify' | 'install'; got: number; total: number };
