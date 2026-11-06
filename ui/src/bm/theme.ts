// Темы главного окна — значения из макета: основы, акценты и как из них складываются переменные CSS.
// Окно трея живёт на своих переменных (lib/theme.ts), их это не трогает.

import type { Appearance, ThemeBase, ThemeMode } from '../lib/types';

export type Vars = Record<string, string>;

export const BASES: Record<ThemeBase, Vars> = {
  light: { bg: '#f5f5f7', surface: '#ffffff', frame: '#d4d4d4', line: '#e4e4e4', fill: '#e4e4e4', fill2: '#eaeaea', text: '#202020', text2: '#767676', text3: '#a3a3a8', icon: '#919191', navIcon: '#8e8e93', ok: '#1fa64a', okTint: 'rgba(31,166,74,.12)', warn: '#d97a00', bad: '#e5372b', pop: '0 12px 32px rgba(0,0,0,.12), inset 0 0 0 .4px #e4e4e4', fade: '#f5f5f7', accentToast: '#1c1c1e' },
  graphite: { bg: '#1a1a1d', surface: '#222226', frame: '#2c2c31', line: '#3a3a40', fill: '#2e2e33', fill2: '#2a2a2f', text: '#f2f2f4', text2: '#8e8e93', text3: '#6e6e73', icon: '#8e8e93', navIcon: '#8e8e93', ok: '#32d74b', okTint: 'rgba(50,215,75,.14)', warn: '#ff9f0a', bad: '#ff453a', pop: '0 16px 40px rgba(0,0,0,.5), inset 0 0 0 .4px #3a3a40', fade: '#1a1a1d', accentToast: '#2e2e33' },
  midnight: { bg: '#121521', surface: '#1a1e2d', frame: '#242a3c', line: '#333a52', fill: '#262c40', fill2: '#222739', text: '#eef0f7', text2: '#8990a8', text3: '#676e86', icon: '#8990a8', navIcon: '#8990a8', ok: '#32d74b', okTint: 'rgba(50,215,75,.14)', warn: '#ff9f0a', bad: '#ff453a', pop: '0 16px 40px rgba(0,0,0,.5), inset 0 0 0 .4px #333a52', fade: '#121521', accentToast: '#262c40' },
  oled: { bg: '#000000', surface: '#111113', frame: '#1c1c1f', line: '#2c2c30', fill: '#1e1e21', fill2: '#1a1a1d', text: '#f5f5f7', text2: '#8e8e93', text3: '#6e6e73', icon: '#8e8e93', navIcon: '#8e8e93', ok: '#32d74b', okTint: 'rgba(50,215,75,.14)', warn: '#ff9f0a', bad: '#ff453a', pop: '0 16px 40px rgba(0,0,0,.7), inset 0 0 0 .4px #2c2c30', fade: '#000000', accentToast: '#1e1e21' },
};

/** Название, цвет для светлой основы, цвет для тёмных. В настройках хранится цвет для тёмных. */
export const ACCENTS: [string, string, string][] = [
  ['Зелёный', '#34c759', '#30d158'],
  ['Синий', '#007aff', '#0a84ff'],
  ['Фиолетовый', '#af52de', '#bf5af2'],
  ['Оранжевый', '#ff9500', '#ff9f0a'],
  ['Голубой', '#32ade6', '#64d2ff'],
  ['Розовый', '#ff2d55', '#ff375f'],
];

export const THEME_OPTS: [ThemeMode, string][] = [
  ['system', 'Системная'],
  ['light', 'Светлая'],
  ['dark', 'Тёмная'],
  ['custom', 'Своя'],
];

export const BASE_OPTS: [ThemeBase, string][] = [
  ['graphite', 'Графит'],
  ['midnight', 'Полночь'],
  ['oled', 'OLED'],
  ['light', 'Светлая'],
];

export function accentEntry(stored: string | undefined): [string, string, string] {
  const s = (stored ?? '').toLowerCase();
  return ACCENTS.find((a) => a[1] === s || a[2] === s) ?? ACCENTS[0];
}

export function baseKey(a: Appearance, sysDark: boolean): ThemeBase {
  if (a.theme === 'light') return 'light';
  if (a.theme === 'dark') return 'graphite';
  if (a.theme === 'system') return sysDark ? 'graphite' : 'light';
  return a.base;
}

export function accentOf(a: Appearance, base: ThemeBase): string {
  const e = accentEntry(a.accent);
  return base === 'light' ? e[1] : e[2];
}

export function varsFor(a: Appearance, sysDark: boolean): Vars {
  const k = baseKey(a, sysDark);
  const name = accentEntry(a.accent)[0];
  return { ...BASES[k], accent: accentOf(a, k), onAccent: k !== 'light' && (name === 'Голубой' || name === 'Оранжевый') ? '#111113' : '#ffffff' };
}

const CACHE = 'klick-bm-appearance';

export function cachedAppearance(): Appearance {
  try {
    const v = JSON.parse(localStorage.getItem(CACHE) ?? 'null') as Appearance | null;
    if (v && v.theme) return v;
  } catch {
    /* нет копии */
  }
  return { theme: 'system', base: 'graphite', accent: '#30d158' };
}

export function cacheAppearance(a: Appearance) {
  try {
    localStorage.setItem(CACHE, JSON.stringify(a));
  } catch {
    /* без localStorage тема применится, когда придут настройки */
  }
}

/** «12, 34, 56» из #hex или rgb() — для холста на главной. */
export function toRgb(c: string): string {
  c = (c || '').trim();
  if (c[0] === '#') {
    const h = c.length === 4 ? c.slice(1).split('').map((x) => x + x).join('') : c.slice(1, 7);
    return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)).join(',');
  }
  const m = c.match(/\d+(\.\d+)?/g);
  return m ? m.slice(0, 3).join(',') : '128,128,128';
}
