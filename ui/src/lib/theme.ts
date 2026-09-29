// Темы из макета: палитры основ, цвета акцента и выбор темы. Тему хранит служба (настройки у ПК одни),
// а окно держит копию в localStorage, чтобы при запуске сразу открыться в своих цветах.

import type { Appearance, ThemeBase, ThemeMode } from './types';
import { tk, tmap } from './lang';

type Palette = Record<string, string>;

export const PALETTES: Record<ThemeBase, Palette> = {
  graphite: { page: '#101012', win: '#1a1a1d', card: '#222226', cardHover: '#26262b', hover: '#2a2a2f', elem: '#2e2e33', elemHover: '#36363c', line: '#2c2c31', raised: '#3a3a40', ring: '#4a4a50', text: '#f2f2f4', text2: '#c7c7cc', text3: '#a1a1a8', dim: '#8e8e93', dim2: '#6e6e73', sunken: '#141416', sunken2: '#1e1e22', dots: '#3c3c42', toast: '#2c2c31', onText: '#111111', shadow: 'rgba(0,0,0,.6)', navShadow: '0 10px 30px rgba(0,0,0,.45)' },
  midnight: { page: '#0b0d14', win: '#121521', card: '#1a1e2d', cardHover: '#1e2334', hover: '#222739', elem: '#262c40', elemHover: '#2d3449', line: '#242a3c', raised: '#333a52', ring: '#454d68', text: '#eef0f7', text2: '#c3c8d8', text3: '#9aa1b7', dim: '#8990a8', dim2: '#676e86', sunken: '#0e1019', sunken2: '#161a27', dots: '#30364a', toast: '#242a3c', onText: '#0b0d14', shadow: 'rgba(0,0,0,.6)', navShadow: '0 10px 30px rgba(0,0,0,.45)' },
  oled: { page: '#000000', win: '#000000', card: '#111113', cardHover: '#161618', hover: '#1a1a1d', elem: '#1e1e21', elemHover: '#26262a', line: '#1c1c1f', raised: '#2c2c30', ring: '#3e3e44', text: '#f5f5f7', text2: '#c7c7cc', text3: '#a1a1a8', dim: '#8e8e93', dim2: '#6e6e73', sunken: '#0a0a0b', sunken2: '#0c0c0e', dots: '#2a2a2e', toast: '#1c1c1f', onText: '#000000', shadow: 'rgba(0,0,0,.8)', navShadow: '0 10px 30px rgba(0,0,0,.45)' },
  light: { page: '#e7e7eb', win: '#f5f5f7', card: '#ffffff', cardHover: '#f7f7f9', hover: '#efeff2', elem: '#ededf1', elemHover: '#e3e3e8', line: '#e4e4e9', raised: '#e3e3e8', ring: '#c7c7cc', text: '#1c1c1e', text2: '#3a3a3c', text3: '#5f5f66', dim: '#7c7c83', dim2: '#a0a0a7', sunken: '#ffffff', sunken2: '#fafafb', dots: '#d2d2d8', toast: '#ffffff', onText: '#ffffff', shadow: 'rgba(0,0,0,.14)', navShadow: '0 4px 16px rgba(0,0,0,.06)' },
};

export const ACCENTS = ['#30d158', '#0a84ff', '#bf5af2', '#ff9f0a', '#64d2ff', '#ff375f'];

export const BASES: [ThemeBase, string][] = [
  ['graphite', tk('Графит')],
  ['midnight', tk('Полночь')],
  ['oled', 'OLED'],
  ['light', tk('Светлая')],
];

export const THEMES: [ThemeMode, string][] = [
  ['system', tk('Системная')],
  ['light', tk('Светлая')],
  ['dark', tk('Тёмная')],
  ['custom', tk('Своя')],
];

export const THEME_DESC: Record<ThemeMode, string> = tmap({
  system: 'Повторяет тему Windows и переключается вместе с ней.',
  light: 'Светлый фон и тёмный текст — удобно при ярком освещении.',
  dark: 'Тёмный фон — меньше нагрузки на глаза вечером.',
  custom: 'Выберите основу и цвет акцента ниже.',
});

const lum = (hex: string) => {
  const n = parseInt(hex.slice(1), 16);
  return (0.2126 * ((n >> 16) & 255)) / 255 + (0.7152 * ((n >> 8) & 255)) / 255 + (0.0722 * (n & 255)) / 255;
};

/** Переменные CSS темы, как в макете: светлая основа — тёмно-зелёный акцент, иначе — яркий. */
export function resolveTheme(a: Appearance, systemDark: boolean): Palette {
  let base: ThemeBase;
  let accent: string;
  if (a.theme === 'custom') {
    base = a.base;
    accent = a.accent;
  } else {
    const dark = a.theme === 'dark' || (a.theme === 'system' && systemDark);
    base = dark ? 'graphite' : 'light';
    accent = dark ? '#30d158' : '#1fa34a';
  }
  return { ...PALETTES[base], accent, onAccent: base === 'light' || lum(accent) < 0.45 ? '#ffffff' : '#0b0b0c', scheme: base === 'light' ? 'light' : 'dark' };
}

const CACHE = 'klick-theme';
let last = '';

export function applyTheme(v: Palette) {
  const key = JSON.stringify(v);
  if (key === last) return;
  last = key;
  const root = document.documentElement;
  for (const [k, value] of Object.entries(v)) {
    if (k !== 'scheme') root.style.setProperty(`--${k}`, value);
  }
  root.style.colorScheme = v.scheme;
  try {
    localStorage.setItem(CACHE, key);
  } catch {
    /* без localStorage тема применится, когда придут настройки */
  }
}

/** При запуске — сразу последняя тема, пока служба не ответила. */
export function applyCachedTheme() {
  try {
    const v = JSON.parse(localStorage.getItem(CACHE) ?? 'null') as Palette | null;
    if (v) applyTheme(v);
  } catch {
    /* нет копии — останется «Графит» из theme.css */
  }
}

export function systemDark(): boolean {
  return typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)').matches : true;
}
