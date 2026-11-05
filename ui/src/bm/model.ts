// Подписи и расчёты для экранов по макету: даты, остаток трафика, задержка, «Авто».

import type { Connection, ServerView, StateView } from '../lib/types';

export const MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];

export const plural = (n: number, a: string, b: string, c: string) => {
  const m = n % 10;
  const h = n % 100;
  return m === 1 && h !== 11 ? a : m >= 2 && m <= 4 && (h < 12 || h > 14) ? b : c;
};

export const fmtClock = (s: number) => {
  s = Math.max(0, Math.floor(s));
  return Math.floor(s / 3600) + ':' + String(Math.floor(s / 60) % 60).padStart(2, '0') + ':' + String(s % 60).padStart(2, '0');
};

/** Скорость в мегабитах, как в макете: «47.0 Мб/с», без трафика — «0 Мб/с». */
export const mbit = (bytesPerSec: number) => (bytesPerSec * 8) / 1e6;

const GB = 1024 ** 3;
export const gb = (bytes: number) => bytes / GB;
export const gbText = (bytes: number) => gb(bytes).toFixed(1).replace('.', ',');
export const gbTotal = (bytes: number) => {
  const v = gb(bytes);
  return Number.isInteger(Math.round(v * 10) / 10) ? String(Math.round(v)) : v.toFixed(1).replace('.', ',');
};

export type SubStatus = 'expired' | 'traffic' | null;

/** Подписка неактивна: срок вышел или трафик кончился. */
export function subStatus(c: Connection | undefined): SubStatus {
  const info = c?.info;
  if (!info) return null;
  if (info.expire && info.expire * 1000 < Date.now()) return 'expired';
  if (info.total > 0 && info.upload + info.download >= info.total) return 'traffic';
  return null;
}

export function dayMonth(unix: number): string {
  const d = new Date(unix * 1000);
  return d.getDate() + ' ' + MONTHS[d.getMonth()];
}

/** Подключение, через которое сейчас идёт VPN (или пытается). */
export function liveIdOf(state: StateView | null, active: string | null): string | null {
  if (!state || state.vpn === 'off') return null;
  return state.connection?.id ?? active;
}

export const pingText = (v: number | null | undefined, tested: boolean) => (v == null ? (tested ? 'нет ответа' : '—') : v + ' мс');
export const pingColor = (v: number | null | undefined, tested: boolean) => (v == null ? (tested ? 'var(--bad)' : 'var(--text3)') : v < 100 ? 'var(--ok)' : v < 300 ? 'var(--warn)' : 'var(--bad)');

export function best(list: ServerView[]): ServerView | undefined {
  return list.filter((x) => x.delay != null).sort((a, b) => (a.delay ?? 0) - (b.delay ?? 0))[0];
}

// «Авто»: окно само выбирает самый быстрый сервер по последнему замеру. Запоминается у подключения.
const AUTO = 'klick-auto';
export function autoSet(): Record<string, boolean> {
  try {
    return JSON.parse(localStorage.getItem(AUTO) ?? '{}') as Record<string, boolean>;
  } catch {
    return {};
  }
}
export function setAuto(id: string, on: boolean) {
  const s = autoSet();
  if (on) s[id] = true;
  else delete s[id];
  try {
    localStorage.setItem(AUTO, JSON.stringify(s));
  } catch {
    /* без localStorage «Авто» живёт до перезапуска окна */
  }
}

const SKIP = 'klick-skip-confirm';
export function skipConfirm(): boolean {
  try {
    return localStorage.getItem(SKIP) === '1';
  } catch {
    return false;
  }
}
export function setSkipConfirm(on: boolean) {
  try {
    if (on) localStorage.setItem(SKIP, '1');
    else localStorage.removeItem(SKIP);
  } catch {
    /* не запомнится */
  }
}
