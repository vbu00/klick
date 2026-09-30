// Тестовая служба для превью в браузере: ведёт себя как настоящая, но ничего не трогает.

import { isMac } from './platform';
import { inKillSwitch, programRule, siteRule } from './live';
import type { Transport } from './transport';
import type {
  AboutView,
  Connection,
  ConnectionView,
  ConnView,
  FailureView,
  IpColumn,
  IpReport,
  KEvent,
  KsProgramView,
  LogLine,
  ProgramView,
  Routing,
  Rule,
  ServerView,
  Service,
  Settings,
  StateView,
  UpdateView,
} from './types';

export type Scenario = 'off' | 'empty' | 'connected' | 'reconnecting' | 'down' | 'error' | 'neighbors';

const GB = 1024 ** 3;
const nowS = () => Math.floor(Date.now() / 1000);
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

function seed(): { connections: Connection[]; servers: Record<string, ServerView[]> } {
  const sub: Connection = {
    id: 'alex',
    name: 'Remnawave · Alex',
    kind: 'subscription',
    info: { upload: 3.1 * GB, download: 39.5 * GB, total: 200 * GB, expire: nowS() + 49 * 86400 },
    updated_at: nowS() - 3600,
    update_interval_hours: 12,
    selected_server: 'Нидерланды · Amsterdam',
  };
  const link: Connection = {
    id: 'grpc',
    name: 'grpc',
    kind: 'link',
    info: null,
    updated_at: nowS(),
    update_interval_hours: null,
    selected_server: 'grpc',
  };
  return {
    connections: [sub, link],
    servers: {
      alex: [
        { name: 'Нидерланды · Amsterdam', kind: 'vless', delay: 48, selected: true },
        { name: 'Германия · Frankfurt', kind: 'vless', delay: 61, selected: false },
        { name: 'Финляндия · Helsinki', kind: 'hysteria2', delay: 39, selected: false },
        { name: 'США · New York', kind: 'trojan', delay: 142, selected: false },
        { name: 'Турция · Istanbul', kind: 'vless', delay: null, selected: false },
      ],
      grpc: [{ name: 'grpc', kind: 'vless', delay: 57, selected: true }],
    },
  };
}

const USER = 'C:\\Users\\user\\AppData';

function seedLists(): Record<Routing, Rule[]> {
  return {
    selected: [
      { target: { kind: 'service', value: 'claude' }, route: 'vpn' },
      { target: { kind: 'service', value: 'youtube' }, route: 'vpn' },
      { target: { kind: 'program', value: `${USER}\\Local\\Discord` }, route: 'vpn' },
      { target: { kind: 'program', value: `${USER}\\Local\\Roblox` }, route: 'vpn' },
      { target: { kind: 'domain', value: 'notion.so' }, route: 'vpn' },
      { target: { kind: 'ip', value: '149.154.160.0/20' }, route: 'vpn' },
      { target: { kind: 'domain', value: 'ads.example.com' }, route: 'block' },
    ],
    all_vpn: [
      { target: { kind: 'program', value: 'C:\\Program Files (x86)\\Steam' }, route: 'direct' },
      { target: { kind: 'domain', value: 'sber.ru' }, route: 'direct' },
      { target: { kind: 'domain', value: 'kinopoisk.ru' }, route: 'vpn' },
    ],
  };
}

const CATALOG: Service[] = (
  [
    ['youtube', 'YouTube', 'youtube.com youtu.be googlevideo.com ytimg.com'],
    ['discord', 'Discord', 'discord.com discord.gg discordapp.com discord.media'],
    ['telegram', 'Telegram', 'telegram.org t.me telegra.ph'],
    ['instagram', 'Instagram', 'instagram.com cdninstagram.com'],
    ['facebook', 'Facebook', 'facebook.com fbcdn.net messenger.com'],
    ['whatsapp', 'WhatsApp', 'whatsapp.com whatsapp.net'],
    ['x', 'X (Twitter)', 'twitter.com x.com twimg.com'],
    ['chatgpt', 'ChatGPT', 'openai.com chatgpt.com oaistatic.com'],
    ['claude', 'Claude', 'claude.ai claude.com anthropic.com'],
    ['gemini', 'Gemini', 'gemini.google.com aistudio.google.com'],
    ['spotify', 'Spotify', 'spotify.com scdn.co'],
    ['roblox', 'Roblox', 'roblox.com rbxcdn.com'],
    ['notion', 'Notion', 'notion.so notion.site'],
    ['linkedin', 'LinkedIn', 'linkedin.com licdn.com'],
    ['netflix', 'Netflix', 'netflix.com nflxvideo.net'],
    ['soundcloud', 'SoundCloud', 'soundcloud.com sndcdn.com'],
    ['proton', 'Proton', 'proton.me protonmail.com'],
    ['speedtest', 'Speedtest', 'speedtest.net ookla.com'],
  ] as const
).map(([id, name, d]) => ({ id, name, domains: d.split(' '), cidrs: id === 'telegram' ? ['149.154.160.0/20'] : [] }));

const PROGRAMS: ProgramView[] = [
  { name: 'Google Chrome', path: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe', folder: 'C:\\Program Files\\Google\\Chrome\\Application', connections: 42 },
  { name: 'Discord', path: `${USER}\\Local\\Discord\\app-1.0.9259\\Discord.exe`, folder: `${USER}\\Local\\Discord`, connections: 14 },
  { name: 'Telegram Desktop', path: `${USER}\\Roaming\\Telegram Desktop\\Telegram.exe`, folder: `${USER}\\Roaming\\Telegram Desktop`, connections: 9 },
  { name: 'Steam', path: 'C:\\Program Files (x86)\\Steam\\steam.exe', folder: 'C:\\Program Files (x86)\\Steam', connections: 7 },
  { name: 'Roblox', path: `${USER}\\Local\\Roblox\\Versions\\version-6f8e1a2b3c4d5e6f\\RobloxPlayerBeta.exe`, folder: `${USER}\\Local\\Roblox`, connections: 5 },
  { name: 'Spotify', path: `${USER}\\Roaming\\Spotify\\Spotify.exe`, folder: `${USER}\\Roaming\\Spotify`, connections: 4 },
  { name: 'Яндекс Музыка', path: `${USER}\\Local\\Programs\\YandexMusic\\Яндекс Музыка.exe`, folder: `${USER}\\Local\\Programs\\YandexMusic`, connections: 3 },
  { name: 'Figma', path: `${USER}\\Local\\Figma\\app-125.4.9\\Figma.exe`, folder: `${USER}\\Local\\Figma`, connections: 2 },
  { name: 'tool', path: 'C:\\Users\\user\\Downloads\\tool.exe', folder: null, connections: 1 },
];

/** Соединения превью: хост, программа, куда пошло, байты в секунду. */
const FLOWS: [string, string, ConnView['route'], number][] = [
  ['rr3---sn-4g5e6nzz.googlevideo.com', 'chrome.exe', 'vpn', 900_000],
  ['www.youtube.com', 'chrome.exe', 'vpn', 40_000],
  ['gateway.discord.gg', 'Discord.exe', 'vpn', 12_000],
  ['media.discordapp.net', 'Discord.exe', 'vpn', 60_000],
  ['claude.ai', 'claude.exe', 'vpn', 8_000],
  ['149.154.167.51', 'Telegram.exe', 'vpn', 5_000],
  ['chatgpt.com', 'chrome.exe', 'vpn', 14_000],
  ['temu.com', 'chrome.exe', 'direct', 9_000],
  ['ya.ru', 'chrome.exe', 'direct', 3_000],
  ['online.sberbank.ru', 'chrome.exe', 'direct', 6_000],
  ['steamcommunity.com', 'steam.exe', 'direct', 20_000],
  ['music.yandex.ru', 'Яндекс Музыка.exe', 'direct', 160_000],
  ['ads.example.com', 'chrome.exe', 'block', 0],
];

/** Что в превью считается заблокированным в России (готовый набор). */
const BLOCKED = ['googlevideo.com', 'youtube.com', 'discord.gg', 'discordapp.net', 'claude.ai', 'chatgpt.com', 'instagram.com', '149.154.167.51'];

const exePath: Record<string, string> = {
  'chrome.exe': PROGRAMS[0].path,
  'Discord.exe': PROGRAMS[1].path,
  'Telegram.exe': PROGRAMS[2].path,
  'steam.exe': PROGRAMS[3].path,
  'Яндекс Музыка.exe': PROGRAMS[6].path,
};

function mockFolder(path: string): string {
  const parts = path.split('\\');
  if (!/\.exe$/i.test(path)) return path;
  parts.pop();
  while (parts.length > 1 && /^(app-|v)?\d+(\.\d+)+|^version-[0-9a-f]+$/i.test(parts[parts.length - 1])) parts.pop();
  if (parts[parts.length - 1]?.toLowerCase() === 'versions') parts.pop();
  const last = parts[parts.length - 1]?.toLowerCase() ?? '';
  if (parts.length <= 1 || ['downloads', 'desktop', 'documents', 'program files', 'program files (x86)'].includes(last)) fail('input.folder_too_broad');
  return parts.join('\\');
}

function ipColumn(c: Partial<IpColumn>): IpColumn {
  return { ipv4: null, ipv6: null, country: null, country_code: null, city: null, provider: null, asn: null, reverse_dns: null, vpn_detected: null, datacenter: null, error: null, ...c };
}

type Err = { code: string; params?: Record<string, unknown> };
const fail = (code: string, params?: Record<string, unknown>): never => {
  throw { code, params } as Err;
};

class MockService {
  private events = new Set<(e: KEvent) => void>();
  private service = new Set<(up: boolean) => void>();
  private st!: StateView;
  private settings!: Settings;
  private servers!: Record<string, ServerView[]>;
  private timer?: ReturnType<typeof setInterval>;
  private level = 4;
  private flowsSince = Date.now();
  private log: LogLine[] = [];

  constructor(scenario: Scenario) {
    this.reset(scenario);
  }

  reset(sc: Scenario) {
    clearInterval(this.timer);
    const { connections, servers } = seed();
    this.servers = servers;
    const empty = sc === 'empty';
    this.settings = {
      mode: 'tun',
      routing: 'selected',
      lists: seedLists(),
      connections: empty ? [] : connections,
      active_connection: empty ? null : 'alex',
      kill_switch: {
        enabled: true,
        programs: [
          { folder: 'C:\\Program Files\\qBittorrent', enabled: true },
          { folder: `${USER}\\Roaming\\Telegram Desktop`, enabled: true },
          { folder: `${USER}\\Local\\Discord`, enabled: true },
          { folder: 'C:\\Program Files\\Mozilla Firefox', enabled: false },
          { folder: 'D:\\Games\\OldGame', enabled: true },
        ],
      },
      blocked_preset: true,
      russia_direct: { domains: true, ips: true },
      on_server_down: 'reconnect',
      restore_on_logon: false,
      auto_update: true,
      notify: true,
      // `?theme=light` — посмотреть превью в светлой теме.
      appearance: { theme: new URLSearchParams(location.search).get('theme') === 'light' ? 'light' : 'dark', base: 'graphite', accent: '#30d158' },
      on_exit: 'ask',
      language: 'ru',
    };
    this.log = [
      [-3600, 'info', 'служба запущена, данные: C:\\ProgramData\\klick · подключений: 2 · сервисов в каталоге: 18'],
      [-3590, 'info', 'Kill Switch: защищено exe 14, снято старых фильтров 0, адаптер нет'],
      [-2540, 'info', 'ядро запущено, режим VPN (TUN)'],
      [-2538, 'info', 'подключено: сервер отвечает, задержка 48 мс'],
      [-1800, 'warn', 'сервер не ответил на проверку, попытка 1 из 3'],
      [-1797, 'info', 'связь восстановлена'],
      [-600, 'info', 'подписка обновлена по расписанию'],
      [-120, 'error', 'ядро: соединение через VPN не удалось (адрес скрыт)'],
    ].map(([d, level, text]) => ({ at: Date.now() + (d as number) * 1000, level: level as LogLine['level'], text: text as string }));
    const vpn: StateView['vpn'] = (
      { off: 'off', empty: 'off', connected: 'connected', reconnecting: 'reconnecting', down: 'server_down', error: 'error', neighbors: 'connected' } as const
    )[sc];
    const running = vpn === 'connected' || vpn === 'reconnecting' || vpn === 'server_down';
    this.st = {
      vpn,
      mode: 'tun',
      routing: 'selected',
      kill_switch: true,
      connection: this.view(),
      server: this.active()?.selected_server ?? null,
      since: running ? nowS() - 2537 : null,
      attempt: vpn === 'reconnecting' ? [2, 3] : null,
      error: vpn === 'error' ? 'core.start_failed' : null,
    };
    if (running) this.startTraffic();
    if (sc === 'neighbors') setTimeout(() => this.emit({ ev: 'notice', code: 'neighbors.conflict', params: { names: ['zapret'] } }), 700);
    this.service.forEach((cb) => cb(true));
  }

  private active(): Connection | undefined {
    return this.settings.connections.find((c) => c.id === this.settings.active_connection);
  }

  private view(): ConnectionView | null {
    const c = this.active();
    return c ? { id: c.id, name: c.name, info: c.info } : null;
  }

  /** Куда ядро превью отправит соединение: Kill Switch, правила списка, готовые наборы. */
  private routeOf(host: string, path: string | null): ConnView['route'] {
    const list = this.settings.lists[this.settings.routing];
    if (inKillSwitch(this.settings.kill_switch, path)) return 'vpn';
    const rule = programRule(list, path) ?? siteRule(list, host, CATALOG);
    if (rule) return rule.route;
    if (this.settings.routing === 'selected') return this.settings.blocked_preset !== false && BLOCKED.some((d) => host === d || host.endsWith(`.${d}`)) ? 'vpn' : 'direct';
    return this.settings.russia_direct.domains && /.(ru|рф)$/.test(host) ? 'direct' : 'vpn';
  }

  private running() {
    return this.st.vpn === 'connected' || this.st.vpn === 'reconnecting' || this.st.vpn === 'server_down';
  }

  private emit(e: KEvent) {
    this.events.forEach((cb) => cb(e));
  }

  private emitState() {
    this.st = { ...this.st, connection: this.view(), server: this.active()?.selected_server ?? null };
    this.emit({ ev: 'state', state: this.st });
  }

  private startTraffic() {
    clearInterval(this.timer);
    this.timer = setInterval(() => {
      this.level = Math.min(14, Math.max(0.3, this.level + (Math.random() - 0.45) * 2.2));
      const down = (this.level * 1e6) / 8;
      const up = ((this.level * (0.12 + Math.random() * 0.2)) * 1e6) / 8;
      this.emit({ ev: 'traffic', up, down });
    }, 1000);
  }

  onEvent(cb: (e: KEvent) => void) {
    this.events.add(cb);
    return () => this.events.delete(cb);
  }

  onService(cb: (up: boolean) => void) {
    this.service.add(cb);
    cb(true);
    return () => this.service.delete(cb);
  }

  async call(cmd: string, args: any): Promise<unknown> {
    await wait(120);
    switch (cmd) {
      case 'status':
      case 'subscribe':
        return this.st;
      case 'settings':
        return this.settings;
      case 'connect': {
        if (!this.active()) fail('vpn.no_connection');
        if (this.running()) return this.st;
        this.st = { ...this.st, vpn: 'connecting', error: null };
        this.emitState();
        await wait(900);
        this.st = { ...this.st, vpn: 'connected', since: nowS(), attempt: null };
        this.startTraffic();
        this.emitState();
        return this.st;
      }
      case 'disconnect':
        clearInterval(this.timer);
        this.st = { ...this.st, vpn: 'off', since: null, attempt: null, error: null };
        this.emitState();
        return this.st;
      case 'set_mode':
        this.settings.mode = args.mode;
        this.st = { ...this.st, mode: args.mode };
        this.emitState();
        return this.st;
      case 'set_routing':
        this.settings.routing = args.routing;
        this.st = { ...this.st, routing: args.routing };
        this.emitState();
        return this.st;
      case 'servers':
        return this.servers[this.settings.active_connection ?? ''] ?? [];
      case 'test_latency': {
        await wait(1400);
        const list = this.servers[this.settings.active_connection ?? ''] ?? [];
        list.forEach((s) => {
          const base = s.delay ?? 90 + Math.random() * 80;
          s.delay = Math.random() < 0.12 ? null : Math.max(18, Math.round(base * (0.85 + Math.random() * 0.3)));
        });
        return list;
      }
      case 'select_server': {
        const list = this.servers[this.settings.active_connection ?? ''] ?? [];
        if (!list.some((s) => s.name === args.name)) fail('server.not_found');
        list.forEach((s) => (s.selected = s.name === args.name));
        const a = this.active();
        if (a) a.selected_server = args.name;
        if (this.running()) this.emit({ ev: 'notice', code: 'server.switched', params: { server: args.name, reason: 'user' } });
        this.emitState();
        return this.st;
      }
      case 'select_connection': {
        if (!this.settings.connections.some((c) => c.id === args.id)) fail('conn.not_found');
        this.settings.active_connection = args.id;
        if (this.running()) {
          this.st = { ...this.st, vpn: 'connecting' };
          this.emitState();
          await wait(700);
          this.st = { ...this.st, vpn: 'connected', since: nowS() };
        }
        this.emitState();
        return this.st;
      }
      case 'refresh_connection': {
        await wait(900);
        if (Math.random() < 0.15) fail('sub.http_status', { status: 502 });
        const c = this.settings.connections.find((x) => x.id === args.id);
        if (!c) return fail('conn.not_found');
        c.updated_at = nowS();
        this.emitState();
        return { id: c.id, name: c.name, info: c.info };
      }
      case 'remove_connection': {
        const wasActive = this.settings.active_connection === args.id;
        if (wasActive && this.running()) {
          clearInterval(this.timer);
          this.st = { ...this.st, vpn: 'off', since: null };
        }
        this.settings.connections = this.settings.connections.filter((c) => c.id !== args.id);
        if (wasActive) this.settings.active_connection = this.settings.connections[0]?.id ?? null;
        this.emitState();
        return this.st;
      }
      case 'add_connection': {
        const src = String(args.source ?? '').trim();
        const scheme = src.split('://')[0]?.toLowerCase();
        await wait(800);
        const id = Math.random().toString(16).slice(2, 10);
        let c: Connection;
        if (scheme === 'https' || scheme === 'http') {
          c = { id, name: args.name || new URL(src).hostname, kind: 'subscription', info: { upload: 0, download: 1.2 * GB, total: 100 * GB, expire: nowS() + 30 * 86400 }, updated_at: nowS(), update_interval_hours: 12, selected_server: 'Нидерланды · Amsterdam' };
          this.servers[id] = [
            { name: 'Нидерланды · Amsterdam', kind: 'vless', delay: null, selected: true },
            { name: 'Турция · Istanbul', kind: 'vless', delay: null, selected: false },
          ];
        } else if (['vless', 'vmess', 'trojan', 'ss', 'hysteria2', 'hy2', 'tuic'].includes(scheme ?? '')) {
          const name = args.name || decodeURIComponent(src.split('#')[1] ?? '') || 'Сервер';
          c = { id, name, kind: 'link', info: null, updated_at: nowS(), update_interval_hours: null, selected_server: name };
          this.servers[id] = [{ name, kind: scheme ?? '', delay: null, selected: true }];
        } else {
          return fail('input.unknown_format');
        }
        this.settings.connections.push(c);
        if (!this.settings.active_connection) this.settings.active_connection = id;
        this.emitState();
        return { id, name: c.name, info: c.info };
      }
      case 'import_file': {
        const id = Math.random().toString(16).slice(2, 10);
        const name = String(args.file_name ?? 'Файл').replace(/\.[^.]+$/, '');
        const c: Connection = { id, name, kind: 'file', info: null, updated_at: nowS(), update_interval_hours: null, selected_server: 'proxy-1' };
        this.servers[id] = [{ name: 'proxy-1', kind: 'vless', delay: null, selected: true }];
        this.settings.connections.push(c);
        if (!this.settings.active_connection) this.settings.active_connection = id;
        this.emitState();
        return { id, name, info: null };
      }
      case 'set_preferences': {
        const p = args.prefs ?? {};
        if (p.blocked_preset !== undefined) this.settings.blocked_preset = p.blocked_preset;
        if (p.ru_domains !== undefined) this.settings.russia_direct.domains = p.ru_domains;
        if (p.ru_ips !== undefined) this.settings.russia_direct.ips = p.ru_ips;
        if (p.on_server_down !== undefined) this.settings.on_server_down = p.on_server_down;
        if (p.restore_on_logon !== undefined) this.settings.restore_on_logon = p.restore_on_logon;
        if (p.auto_update !== undefined) this.settings.auto_update = p.auto_update;
        if (p.notify !== undefined) this.settings.notify = p.notify;
        if (p.appearance !== undefined) this.settings.appearance = p.appearance;
        if (p.on_exit !== undefined) this.settings.on_exit = p.on_exit;
        return this.settings;
      }
      case 'kill_switch_set':
        this.settings.kill_switch.enabled = args.enabled;
        this.st = { ...this.st, kill_switch: args.enabled };
        this.emitState();
        return this.settings.kill_switch;
      case 'kill_switch_add': {
        const folder = mockFolder(String(args.folder));
        if (this.settings.kill_switch.programs.some((p) => p.folder.toLowerCase() === folder.toLowerCase())) fail('list.duplicate');
        this.settings.kill_switch.programs.push({ folder, enabled: true });
        return this.settings.kill_switch;
      }
      case 'kill_switch_remove': {
        const before = this.settings.kill_switch.programs.length;
        this.settings.kill_switch.programs = this.settings.kill_switch.programs.filter((p) => p.folder.toLowerCase() !== String(args.folder).toLowerCase());
        if (this.settings.kill_switch.programs.length === before) fail('list.not_found');
        return this.settings.kill_switch;
      }
      case 'kill_switch_program': {
        const p = this.settings.kill_switch.programs.find((x) => x.folder.toLowerCase() === String(args.folder).toLowerCase());
        if (!p) return fail('list.not_found');
        p.enabled = args.enabled;
        return this.settings.kill_switch;
      }
      case 'program_names': {
        const known: Record<string, string> = {
          'C:\\Program Files\\qBittorrent': 'qBittorrent',
          'C:\\Program Files\\Mozilla Firefox': 'Firefox',
          'C:\\Program Files (x86)\\Steam': 'Steam',
          'C:\\Program Files\\Google\\Chrome\\Application': 'Google Chrome',
          [`${USER}\\Roaming\\Telegram Desktop`]: 'Telegram Desktop',
          [`${USER}\\Local\\Discord`]: 'Discord',
          [`${USER}\\Local\\Roblox`]: 'Roblox',
          [`${USER}\\Local\\Programs\\YandexMusic`]: 'Яндекс Музыка',
        };
        return Object.fromEntries((args.folders as string[]).filter((f) => known[f]).map((f) => [f, known[f]]));
      }
      case 'kill_switch_status':
        return this.settings.kill_switch.programs.map((p): KsProgramView => ({ ...p, exes: p.folder.startsWith('D:\\Games\\OldGame') ? 0 : 3 }));
      case 'resume':
        return this.st;
      case 'about': {
        const about: AboutView = isMac
          ? { version: '0.4.0', core_version: 'v1.19.31', mixed_port: 7890, data_dir: '/Library/Application Support/klick', os: 'macOS 15.1 Sequoia · Apple Silicon', dev: false }
          : { version: '0.4.0', core_version: 'v1.19.31', mixed_port: 7890, data_dir: 'C:\\ProgramData\\klick', os: 'Windows 11 · 25H2 · x64', dev: false };
        return about;
      }
      case 'log':
        return this.log;
      case 'log_clear':
        this.log = [];
        return [];
      case 'report':
        return `kl!ck 0.4.0 · ядро v1.19.31 · Windows 11 · 25H2 · x64\nСостояние: ${this.st.vpn} · режим ${this.st.mode} · положение ${this.st.routing}\n(превью: тестовая служба)`;
      case 'check_update': {
        await wait(1100);
        const u: UpdateView = { current: '0.4.0', latest: '0.3.0', url: 'https://github.com/vbu00/klick/releases/tag/v0.3.0', newer: false };
        return u;
      }
      case 'catalog':
        return { services: CATALOG };
      case 'programs':
        await wait(300);
        return PROGRAMS;
      case 'list_add': {
        const list = this.settings.lists[args.position as Routing];
        const rule: Rule = structuredClone(args.rule);
        const t = rule.target;
        if (t.kind === 'domain') {
          const v = t.value.trim().toLowerCase().replace(/^[a-z]+:\/\//, '').split(/[/?#]/)[0].replace(/^\*?\./, '');
          if (!/^[a-z0-9а-яё-]+(\.[a-z0-9а-яё-]+)*$/.test(v)) fail('input.bad_domain');
          t.value = v;
        } else if (t.kind === 'ip') {
          if (!/^[\d.]+(\/\d{1,2})?$|^[0-9a-f:]+(\/\d{1,3})?$/i.test(t.value.trim())) fail('input.bad_ip');
          t.value = t.value.includes('/') ? t.value.trim() : `${t.value.trim()}/32`;
        } else if (t.kind === 'program') {
          t.value = mockFolder(t.value);
        }
        if (list.some((r) => r.target.kind === t.kind && r.target.value.toLowerCase() === t.value.toLowerCase())) fail('list.duplicate');
        list.push({ ...rule, enabled: rule.enabled ?? true });
        return list;
      }
      case 'list_remove': {
        const list = this.settings.lists[args.position as Routing];
        if (args.index >= list.length) fail('list.bad_index');
        list.splice(args.index, 1);
        return list;
      }
      case 'list_set_route': {
        const list = this.settings.lists[args.position as Routing];
        if (!list[args.index]) fail('list.bad_index');
        list[args.index].route = args.route;
        return list;
      }
      case 'list_set_enabled': {
        const list = this.settings.lists[args.position as Routing];
        if (!list[args.index]) fail('list.bad_index');
        list[args.index].enabled = args.enabled;
        return list;
      }
      case 'connections': {
        if (!this.running()) return [];
        const secs = (Date.now() - this.flowsSince) / 1000 + 40;
        return FLOWS.map(([host, process, route, rate], i): ConnView => ({
          host,
          process,
          process_path: exePath[process] ?? null,
          route: route === 'block' ? route : this.routeOf(host, exePath[process] ?? null),
          rule: route === 'direct' ? 'Match' : 'RuleSet',
          network: i === 5 ? 'udp' : 'tcp',
          download: Math.round(rate * secs * (0.9 + (i % 3) * 0.05)),
          upload: Math.round(rate * secs * 0.08),
        }));
      }
      case 'failures': {
        const now = nowS();
        const list: FailureView[] = [
          { host: 'chatgpt.com', route: 'vpn', error: 'dial tcp 104.18.32.47:443: i/o timeout', at: now - 95 },
          { host: 'example.org', route: 'direct', error: 'connection reset by peer', at: now - 640 },
          { host: 'cdn.game-files.net', route: 'direct', error: 'EOF', at: now - 1900 },
        ];
        return this.running() ? list : [];
      }
      case 'ip_check': {
        await wait(1600);
        const tun = this.settings.mode === 'tun';
        const report: IpReport = {
          via_vpn: this.running()
            ? ipColumn({ ipv4: '185.199.110.42', country: 'Netherlands', country_code: 'NL', city: 'Amsterdam', provider: 'Datacamp Limited', asn: 60068, reverse_dns: 'unn-185-199-110-42.datapacket.com', vpn_detected: true, datacenter: true })
            : null,
          direct: ipColumn({ ipv4: '95.24.113.7', ipv6: '2a02:2698:7c21::1f', country: 'Russia', country_code: 'RU', city: 'Moscow', provider: 'Home ISP', asn: 8402, reverse_dns: 'host-95-24-113-7.example.net', vpn_detected: false, datacenter: false }),
          ipv6_leak: this.running() && tun && this.settings.routing === 'all_vpn' ? false : null,
          dns_protected: this.running() && tun ? true : null,
          checked_at: nowS(),
        };
        return report;
      }
      default:
        return fail('unknown');
    }
  }
}

let autostart = true;

export function createMockTransport(): Transport {
  const params = new URLSearchParams(location.search);
  const svc = new MockService((params.get('s') as Scenario) || 'off');
  (window as any).__klickMock = { setScenario: (s: Scenario) => svc.reset(s) };
  const view = params.get('view') === 'tray' ? 'tray' : 'main';
  return {
    kind: 'mock',
    window: view,
    call: <T>(cmd: string, args?: unknown) => svc.call(cmd, args) as Promise<T>,
    onEvent: (cb) => {
      const off = svc.onEvent(cb);
      return () => void off();
    },
    onService: (cb) => {
      const off = svc.onService(cb);
      return () => void off();
    },
    win: { minimize() {}, hide() {} },
    pickExe: async () => (isMac ? '/Applications/Discord.app' : 'C:\\Games\\Genshin Impact\\Genshin Impact Game\\GenshinImpact.exe'),
    openUrl: async (url) => void window.open(url, '_blank', 'noopener'),
    openMain: (target) => console.info('[превью] открыть главное окно', target ?? ''),
    hideTray: () => console.info('[превью] спрятать окно трея'),
    fitTray: () => undefined,
    clipboardText: async () => 'vless://00000000-0000-0000-0000-000000000000@example.com:443?type=grpc#Превью',
    repairService: async () => console.info('[превью] перезапуск службы'),
    exit: async (clearProxy) => console.info('[превью] выход', { clearProxy }),
    isActive: async () => document.visibilityState === 'visible' && document.hasFocus(),
    notify: async (title, text, target) => console.info('[превью] уведомление Windows:', title, text, target),
    onNavigate: () => () => undefined,
    onExitRequest: () => () => undefined,
    autostart: {
      get: async () => autostart,
      set: async (on) => {
        autostart = on;
      },
    },
  };
}
