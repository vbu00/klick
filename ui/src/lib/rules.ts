// Как показывать правила, сервисы и программы: имена, подписи, цвет плитки.

import { targetKind } from './i18n';
import type { ConnView, Routing, Route, Rule, Service } from './types';

/** Цвета плиток с первой буквой — из макета. */
const TINTS = ['#a8c7fa', '#f5c26b', '#c4b5fd', '#9ad9c0', '#f4a7b9', '#b8c4d0'];

export function tint(key: string): string {
  let h = 0;
  for (const ch of key) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return TINTS[h % TINTS.length];
}

/** Что правило делает по умолчанию в этом положении тумблера. */
export const defaultRoute: Record<Routing, Route> = { all_vpn: 'direct', selected: 'vpn' };

/** Папки, по имени которых программу не узнать. */
const GENERIC = new Set(['app', 'application', 'bin', 'bin64', 'x64', 'x86', 'win64', 'win32', 'binaries', 'current', 'release']);
/** Пакет Microsoft Store: `Имя_Версия_Архитектура__Издатель`. */
const MSIX = /^(.+?)_\d+(\.\d+)+_(x64|x86|arm64|arm|neutral)__[a-z0-9]+$/i;

/** Название программы по папке правила: «…\Discord» → Discord, «…\Claude_2.9939.2.0_x64__…\app» → Claude,
 *  «/Applications/Discord.app» → Discord. */
export function programTitle(folder: string): string {
  const parts = folder.split(/[\\/]/).filter(Boolean);
  for (let i = parts.length - 1; i > 0; i--) {
    const m = MSIX.exec(parts[i]);
    if (m) return m[1].split('.').pop() || m[1];
    if (/\.app$/i.test(parts[i])) return parts[i].slice(0, -4);
    if (!GENERIC.has(parts[i].toLowerCase())) return parts[i];
  }
  return parts[parts.length - 1] ?? folder;
}

/** Путь покороче: «C:\Users\user\AppData\Local\Discord» → «…\AppData\Local\Discord». Разделитель — как в самом пути. */
export function shortPath(path: string, keep = 3): string {
  const sep = path.includes('\\') ? '\\' : '/';
  const parts = path.split(sep).filter((p, i) => p || i > 0);
  return parts.length > keep + 1 ? `…${sep}${parts.slice(-keep).join(sep)}` : path;
}

export function ruleTitle(rule: Rule, catalog: Service[], names: Record<string, string> = {}): string {
  const t = rule.target;
  switch (t.kind) {
    case 'service':
      return catalog.find((s) => s.id === t.value)?.name ?? t.value;
    case 'program':
      return names[t.value] ?? programTitle(t.value);
    case 'domain':
      return t.value.includes('.') ? unicodeDomain(t.value) : `.${unicodeDomain(t.value)}`;
    case 'ip':
      return t.value.endsWith('/32') || t.value.endsWith('/128') ? t.value.replace(/\/(32|128)$/, '') : t.value;
  }
}

export function ruleSub(rule: Rule): string {
  const t = rule.target;
  switch (t.kind) {
    case 'service':
      return targetKind.service;
    case 'program':
      return shortPath(t.value);
    case 'domain':
      return t.value.includes('.') ? 'сайт и поддомены' : 'вся зона';
    case 'ip':
      return /\/(32|128)$/.test(t.value) ? 'адрес' : 'подсеть';
  }
}

/** Сервис каталога, к которому относится хост: `rr3---sn.googlevideo.com` → YouTube. */
export function serviceForHost(host: string, catalog: Service[]): Service | undefined {
  const h = host.toLowerCase();
  return catalog.find((s) => s.domains.some((d) => h === d || h.endsWith(`.${d}`)));
}

/** Браузер ходит на любые сайты, поэтому у него интереснее сайт, чем имя программы. */
const BROWSERS = new Set([
  'chrome', 'msedge', 'firefox', 'opera', 'brave', 'browser', 'vivaldi', 'arc', 'zen', 'librewolf', 'waterfox', 'thorium', 'chromium', 'yandex',
  // macOS: имена процессов без «Helper»
  'google chrome', 'microsoft edge', 'brave browser', 'safari', 'com.apple.webkit.networking', 'plugin-container',
]);

/** Имя процесса без `.exe` и без помощника macOS: «Google Chrome Helper (Renderer)» → «Google Chrome». */
export function baseProcess(process: string): string {
  return process.replace(/\.exe$/i, '').replace(/ Helper( \([^)]*\))?$/, '');
}

/** «steam.exe» → «Steam». */
export function programName(process: string): string {
  const name = baseProcess(process);
  return name.charAt(0).toUpperCase() + name.slice(1);
}

/** Как назвать соединение на схеме: сервис; у браузера — сайт; иначе программа; иначе адрес. */
export function flowLabel(c: ConnView, catalog: Service[]): string {
  const svc = serviceForHost(c.host, catalog);
  if (svc) return svc.name;
  const exe = c.process ? baseProcess(c.process).toLowerCase() : undefined;
  if (c.host && (!exe || BROWSERS.has(exe))) return unicodeDomain(c.host.replace(/^www\./, ''));
  return c.process ? programName(c.process) : c.host;
}

/** 1 программа, 2 программы, 5 программ. */
export function plural(n: number, forms: [string, string, string]): string {
  const m10 = n % 10;
  const m100 = n % 100;
  const form = m10 === 1 && m100 !== 11 ? forms[0] : m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14) ? forms[1] : forms[2];
  return `${n} ${form}`;
}

/** Домен из punycode обратно в буквы: `xn--p1ai` → `рф`. */
export function unicodeDomain(domain: string): string {
  return domain
    .split('.')
    .map((label) => {
      if (!label.startsWith('xn--')) return label;
      try {
        return punyDecode(label.slice(4));
      } catch {
        return label;
      }
    })
    .join('.');
}

/** Декодер punycode по RFC 3492. */
function punyDecode(input: string): string {
  const base = 36;
  const tMin = 1;
  const tMax = 26;
  const adapt = (delta: number, points: number, first: boolean) => {
    delta = first ? Math.floor(delta / 700) : delta >> 1;
    delta += Math.floor(delta / points);
    let k = 0;
    while (delta > ((base - tMin) * tMax) >> 1) {
      delta = Math.floor(delta / (base - tMin));
      k += base;
    }
    return k + Math.floor(((base - tMin + 1) * delta) / (delta + 38));
  };
  const digit = (c: number) => (c >= 48 && c <= 57 ? c - 22 : c >= 65 && c <= 90 ? c - 65 : c >= 97 && c <= 122 ? c - 97 : base);
  const out: number[] = [];
  const dash = input.lastIndexOf('-');
  for (let j = 0; j < Math.max(dash, 0); j++) out.push(input.charCodeAt(j));
  let n = 128;
  let i = 0;
  let bias = 72;
  for (let idx = dash > 0 ? dash + 1 : 0; idx < input.length; ) {
    const old = i;
    for (let w = 1, k = base; ; k += base) {
      if (idx >= input.length) throw new Error('punycode');
      const d = digit(input.charCodeAt(idx++));
      if (d >= base) throw new Error('punycode');
      i += d * w;
      const t = k <= bias ? tMin : k >= bias + tMax ? tMax : k - bias;
      if (d < t) break;
      w *= base - t;
    }
    const len = out.length + 1;
    bias = adapt(i - old, len, old === 0);
    n += Math.floor(i / len);
    i %= len;
    out.splice(i++, 0, n);
  }
  return String.fromCodePoint(...out);
}
