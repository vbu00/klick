// Соединения и неудачи из службы — раз в 2 секунды, пока экран открыт и VPN работает.

import { useEffect, useMemo, useState } from 'react';
import { groupLive, type AppRow } from '../lib/live';
import { isIos } from '../lib/platform';
import { useStore, useVisible } from '../lib/store';
import type { ConnView, FailureView } from '../lib/types';
import { plural } from './model';

export function useLive(active: boolean) {
  const { transport, isRunning, settings, catalog, programNames } = useStore();
  const [conns, setConns] = useState<ConnView[]>([]);
  const [fails, setFails] = useState<FailureView[]>([]);
  // Спрятанное в трей окно службу не опрашивает; показали — сразу свежие данные.
  const visible = useVisible();
  const on = active && isRunning && visible;
  useEffect(() => {
    if (!on) {
      setConns([]);
      return;
    }
    let alive = true;
    const tick = async () => {
      const [c, f] = await Promise.all([transport.call<ConnView[]>('connections').catch(() => null), transport.call<FailureView[]>('failures').catch(() => null)]);
      if (!alive) return;
      if (c) setConns(c);
      if (f) setFails(f);
    };
    void tick();
    const t = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [on, transport]);
  const list = useMemo(() => (settings ? settings.lists[settings.routing] : []), [settings]);
  const apps = useMemo(() => {
    const rows = groupLive(conns, list, settings?.kill_switch, catalog, programNames);
    if (!isIos) return rows;
    // iOS: программ нет — самые активные сайты.
    return rows
      .flatMap((a) => a.sites)
      .map((s): AppRow => ({ key: s.host, name: s.host, path: null, conns: s.conns, vpnConns: s.vpn ? s.conns : 0, bytes: s.bytes, sites: [], vpn: s.vpn, why: s.why, locked: false }));
  }, [conns, list, settings?.kill_switch, catalog, programNames]);
  return { conns, fails, apps };
}

export type Tag = 'vpn' | 'dir' | 'mix';
export const TAG: Record<Tag, [string, string, string]> = {
  vpn: ['VPN', 'color-mix(in srgb, var(--accent) 14%, transparent)', 'var(--accent)'],
  dir: ['Напрямую', 'var(--fill2)', 'var(--text2)'],
  mix: ['Частично', 'color-mix(in srgb, var(--warn) 14%, transparent)', 'var(--warn)'],
};

export const tagOf = (a: AppRow): Tag => (a.vpnConns <= 0 ? 'dir' : a.vpnConns >= a.conns ? 'vpn' : 'mix');

export const connsText = (n: number) => n + ' ' + plural(n, 'соединение', 'соединения', 'соединений');
export const appsText = (n: number) => (isIos ? n + ' ' + plural(n, 'сайт', 'сайта', 'сайтов') : n + ' ' + plural(n, 'программа', 'программы', 'программ'));
export const metaOf = (a: AppRow) => connsText(a.conns) + ' · ' + (a.bytes / 1e6).toFixed(1).replace('.', ',') + ' МБ';
