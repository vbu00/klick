// «Сейчас в сети» по макету v4: соединения по программам и сайтам, один тумблер — «через VPN».
// Тумблер пишет правило в «мой список» текущего положения. Порядок как в ядре (бриф): Kill Switch,
// потом правила программ, потом сайтов, сервисов и IP — поэтому, если у программы своё правило,
// её сайты по одному не переключаются.

import { baseProcess, programName, serviceForHost, unicodeDomain } from './rules';
import type { ConnView, KillSwitch, Route, Routing, Rule, Service, Target } from './types';

/** Что решает, куда пойдёт соединение, — для тумблера и подписи. */
export type Why = 'killswitch' | 'program' | 'rule' | 'core';

export interface SiteRow {
  /** Хост без `www.`, как его видит человек. */
  host: string;
  app: string;
  appKey: string;
  path: string | null;
  conns: number;
  bytes: number;
  vpn: boolean;
  why: Why;
  /** Правило какого вида запишет тумблер. */
  target: Target;
}

export interface AppRow {
  key: string;
  name: string;
  path: string | null;
  conns: number;
  vpnConns: number;
  bytes: number;
  sites: SiteRow[];
  /** Тумблер программы: всё её идёт через VPN. */
  vpn: boolean;
  why: Why;
  /** Правило программы в списке (включённое) — сайты по одному не переключаются. */
  locked: boolean;
}

/** Понятные имена частых программ: `msedge.exe` → Edge, `Google Chrome Helper` → Chrome. */
const ALIASES: Record<string, string> = {
  msedge: 'Edge',
  chrome: 'Chrome',
  firefox: 'Firefox',
  opera: 'Opera',
  browser: 'Яндекс Браузер',
  telegram: 'Telegram',
  discord: 'Discord',
  steam: 'Steam',
  steamwebhelper: 'Steam',
  spotify: 'Spotify',
  svchost: 'Система',
  system: 'Система',
  // macOS
  'google chrome': 'Chrome',
  'microsoft edge': 'Edge',
  'yandex': 'Яндекс Браузер',
  'brave browser': 'Brave',
  'com.apple.webkit.networking': 'Safari и WebKit',
  mdnsresponder: 'Система',
  'plugin-container': 'Firefox',
};

export function appName(c: Pick<ConnView, 'process' | 'process_path'>, names: Record<string, string>): string {
  if (c.process_path && names[c.process_path]) return names[c.process_path];
  if (!c.process) return 'Неизвестная программа';
  const base = baseProcess(c.process);
  return ALIASES[base.toLowerCase()] ?? programName(c.process);
}

const isIp = (h: string) => /^[\d.]+$/.test(h) || h.includes(':');

/** Сайт для правила: `rr3.googlevideo.com` → сервис YouTube; `cdn.example.co.uk` → `example.co.uk`. */
export function siteTarget(host: string, catalog: Service[]): Target {
  const svc = serviceForHost(host, catalog);
  if (svc) return { kind: 'service', value: svc.id };
  if (isIp(host)) return { kind: 'ip', value: host };
  return { kind: 'domain', value: baseDomain(host) };
}

/** Сайт без поддоменов: две последние части, три — у `co.uk`, `com.ru` и похожих. */
export function baseDomain(host: string): string {
  const labels = host.toLowerCase().replace(/\.$/, '').split('.');
  if (labels.length <= 2) return labels.join('.');
  const sld = labels[labels.length - 2];
  const tld = labels[labels.length - 1];
  const threeParts = tld.length === 2 && ['co', 'com', 'net', 'org', 'gov', 'edu', 'ac', 'or', 'ne', 'go', 'msk', 'spb'].includes(sld);
  return labels.slice(threeParts ? -3 : -2).join('.');
}

const lower = (s: string) => s.toLowerCase();

/** Путь внутри папки. Разделитель — `\\` на Windows и `/` на macOS. */
function inFolder(path: string, folder: string): boolean {
  const p = lower(path);
  const f = lower(folder).replace(/[\\/]+$/, '');
  return p === f || p.startsWith(`${f}\\`) || p.startsWith(`${f}/`);
}

/** Сколько уровней в пути папки: глубже — точнее. */
const depth = (folder: string) => folder.split(/[\\/]/).filter(Boolean).length;

function hostMatches(host: string, domain: string): boolean {
  const h = lower(host);
  const d = lower(domain).replace(/^\./, '');
  return h === d || h.endsWith(`.${d}`);
}

function ipInCidr(ip: string, cidr: string): boolean {
  const [net, bitsText] = cidr.split('/');
  const toNum = (s: string) => s.split('.').reduce((a, x) => (a << 8) + (Number(x) & 255), 0) >>> 0;
  if (!/^[\d.]+$/.test(ip) || !/^[\d.]+$/.test(net)) return lower(ip) === lower(net);
  const bits = bitsText === undefined ? 32 : Number(bitsText);
  const mask = bits === 0 ? 0 : (0xffffffff << (32 - bits)) >>> 0;
  return (toNum(ip) & mask) === (toNum(net) & mask);
}

/** Включённое правило программы для exe: самая глубокая папка. */
export function programRule(list: Rule[], path: string | null): Rule | undefined {
  if (!path) return undefined;
  return list
    .filter((r) => r.enabled !== false && r.target.kind === 'program' && inFolder(path, r.target.value))
    .sort((a, b) => depth(b.target.value) - depth(a.target.value))[0];
}

/** Включённое правило сайта, сервиса или IP для хоста: точное раньше общего, своё раньше сервиса. */
export function siteRule(list: Rule[], host: string, catalog: Service[]): Rule | undefined {
  let best: { rule: Rule; rank: number } | undefined;
  for (const rule of list) {
    if (rule.enabled === false) continue;
    const t = rule.target;
    let rank = -1;
    if (t.kind === 'domain' && hostMatches(host, t.value)) rank = t.value.split('.').length * 2 + 1;
    else if (t.kind === 'ip' && isIp(host) && ipInCidr(host, t.value)) rank = 100 + Number(t.value.split('/')[1] ?? 32) * 2 + 1;
    else if (t.kind === 'service') {
      const svc = catalog.find((s) => s.id === t.value);
      const d = svc?.domains.filter((x) => hostMatches(host, x)).sort((a, b) => b.split('.').length - a.split('.').length)[0];
      if (d) rank = d.split('.').length * 2;
      else if (svc && isIp(host) && svc.cidrs.some((c) => ipInCidr(host, c))) rank = 100;
    }
    if (rank >= 0 && (!best || rank > best.rank)) best = { rule, rank };
  }
  return best?.rule;
}

export function inKillSwitch(ks: KillSwitch | undefined, path: string | null): boolean {
  return !!ks?.enabled && !!path && ks.programs.some((p) => p.enabled && inFolder(path, p.folder));
}

/** Куда идёт соединение: Kill Switch, правило программы, правило сайта — или как решило ядро. */
function decide(c: ConnView, list: Rule[], ks: KillSwitch | undefined, catalog: Service[]): { vpn: boolean; why: Why } {
  if (inKillSwitch(ks, c.process_path)) return { vpn: true, why: 'killswitch' };
  const p = programRule(list, c.process_path);
  if (p) return { vpn: p.route === 'vpn', why: 'program' };
  const s = siteRule(list, c.host, catalog);
  if (s) return { vpn: s.route === 'vpn', why: 'rule' };
  return { vpn: c.route === 'vpn', why: 'core' };
}

/** Программы и их сайты. Сначала те, что качают больше. */
export function groupLive(conns: ConnView[], list: Rule[], ks: KillSwitch | undefined, catalog: Service[], names: Record<string, string>): AppRow[] {
  const apps = new Map<string, AppRow & { siteMap: Map<string, SiteRow> }>();
  for (const c of conns) {
    if (c.route === 'block') continue;
    const key = lower(c.process_path ?? c.process ?? '?');
    const name = appName(c, names);
    let app = apps.get(key);
    if (!app) {
      app = { key, name, path: c.process_path, conns: 0, vpnConns: 0, bytes: 0, sites: [], vpn: false, why: 'core', locked: false, siteMap: new Map() };
      apps.set(key, app);
    }
    const d = decide(c, list, ks, catalog);
    const bytes = c.download + c.upload;
    app.conns += 1;
    app.bytes += bytes;
    if (d.vpn) app.vpnConns += 1;
    const host = unicodeDomain(c.host.replace(/^www\./, '')) || '—';
    const site = app.siteMap.get(host);
    if (site) {
      site.conns += 1;
      site.bytes += bytes;
    } else {
      app.siteMap.set(host, { host, app: name, appKey: key, path: c.process_path, conns: 1, bytes, vpn: d.vpn, why: d.why, target: siteTarget(c.host, catalog) });
    }
  }
  return [...apps.values()]
    .map(({ siteMap, ...a }) => {
      const sites = [...siteMap.values()].sort((x, y) => y.bytes - x.bytes);
      const ksLocked = inKillSwitch(ks, a.path);
      const rule = programRule(list, a.path);
      const why: Why = ksLocked ? 'killswitch' : rule ? 'program' : 'core';
      const vpn = ksLocked || (rule ? rule.route === 'vpn' : sites.length > 0 && sites.every((s) => s.vpn));
      return { ...a, sites, vpn, why, locked: ksLocked || !!rule };
    })
    .sort((x, y) => y.bytes - x.bytes);
}

/** Правило программы для exe, включённое или нет: самая глубокая папка. Индекс в списке или -1. */
export function findProgramRule(list: Rule[], path: string): number {
  let best = -1;
  list.forEach((r, i) => {
    if (r.target.kind !== 'program' || !inFolder(path, r.target.value)) return;
    if (best < 0 || depth(r.target.value) > depth(list[best].target.value)) best = i;
  });
  return best;
}

/** Точное правило для цели в списке (включённое или нет). */
export function findRule(list: Rule[], target: Target): number {
  return list.findIndex((r) => r.target.kind === target.kind && lower(r.target.value) === lower(target.value));
}

/** Что сделать с правилом, чтобы цель пошла через VPN (`vpn`) или напрямую. */
export type Plan = { op: 'add'; rule: Rule } | { op: 'set'; index: number; route: Route; enable: boolean } | { op: 'none' };

export function planToggle(list: Rule[], target: Target, vpn: boolean): Plan {
  const route: Route = vpn ? 'vpn' : 'direct';
  const index = target.kind === 'program' ? findProgramRule(list, target.value) : findRule(list, target);
  if (index < 0) return { op: 'add', rule: { target, route, enabled: true } };
  const r = list[index];
  if (r.route === route && r.enabled !== false) return { op: 'none' };
  return { op: 'set', index, route, enable: true };
}

/** Цель программы для правила: папку по пути exe найдёт служба. */
export function programTarget(path: string): Target {
  return { kind: 'program', value: path };
}

export function positionVerb(routing: Routing): string {
  return routing === 'selected' ? 'Через VPN' : 'Напрямую';
}
