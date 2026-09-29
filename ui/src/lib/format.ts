// Форматы как в макете: скорость в Mb/s, объём в МБ/ГБ, задержка в мс.

import { lang, t } from './lang';

const GB = 1024 ** 3;
const MB = 1024 ** 2;

/** Скорость из байт в секунду. */
export function fmtRate(bytesPerSec: number): string {
  const mbit = (bytesPerSec * 8) / 1e6;
  return mbit >= 1000 ? (mbit / 1000).toFixed(2) + ' Gb/s' : mbit.toFixed(2) + ' Mb/s';
}

export function fmtBytes(bytes: number): string {
  return bytes >= GB ? (bytes / GB).toFixed(2) + ' ' + t('ГБ') : (bytes / MB).toFixed(1) + ' ' + t('МБ');
}

/** «42.6 из 200 ГБ». */
export function fmtTraffic(used: number, total: number): string {
  const u = used / GB;
  const all = total / GB;
  const tt = Number.isInteger(Math.round(all * 10) / 10) ? Math.round(all).toString() : all.toFixed(1);
  return t('{used} из {total} ГБ', { used: u.toFixed(1), total: tt });
}

export function fmtTimer(sinceUnix: number | null, nowMs: number): string {
  if (!sinceUnix) return '0:00:00';
  const s = Math.max(0, Math.floor(nowMs / 1000 - sinceUnix));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
}

export function fmtDate(unix: number): string {
  return new Date(unix * 1000).toLocaleDateString(lang() === 'en' ? 'en-GB' : 'ru-RU', { day: 'numeric', month: 'long' });
}

export function daysLeft(unix: number): number {
  return Math.max(0, Math.round((unix * 1000 - Date.now()) / 864e5));
}

export function pingColor(delay: number | null | undefined): string {
  if (delay == null) return 'var(--red)';
  if (delay < 80) return 'var(--accent)';
  if (delay < 160) return 'var(--text)';
  return 'var(--orange)';
}

export function pingText(delay: number | null | undefined): string {
  return delay == null ? t('нет ответа') : t('{n} мс', { n: delay });
}

const PROTOCOLS: Record<string, string> = {
  vless: 'VLESS',
  vmess: 'VMess',
  trojan: 'Trojan',
  ss: 'Shadowsocks',
  shadowsocks: 'Shadowsocks',
  ssr: 'ShadowsocksR',
  hysteria: 'Hysteria',
  hysteria2: 'Hysteria2',
  tuic: 'TUIC',
  wireguard: 'WireGuard',
  socks5: 'SOCKS5',
  http: 'HTTP',
  anytls: 'AnyTLS',
};

export function protocolName(kind: string): string {
  return PROTOCOLS[kind.toLowerCase()] ?? kind;
}

/** Линия и заливка графика скорости за 60 секунд в поле 300×90. */
export function buildPath(hist: number[], max: number): { line: string; area: string } {
  const n = hist.length;
  if (n === 0) return { line: '', area: '' };
  const pts = hist.map((v, i) => [(i / (n - 1)) * 300, 88 - (v / max) * 84] as const);
  const line = pts.map(([x, y], i) => (i ? 'L' : 'M') + x.toFixed(1) + ' ' + y.toFixed(1)).join(' ');
  return { line, area: `${line} L300 89 L0 89 Z` };
}
