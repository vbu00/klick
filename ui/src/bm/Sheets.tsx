// Листы снизу из макета: «Скорость», «Добавить подключение», «Мои исключения» / «Мои сайты и приложения»,
// «Программы в сети», «Способ подключения».

import { useEffect, useRef, useState } from 'react';
import { errorText } from '../lib/i18n';
import { detect as detectSource } from '../lib/source';
import { isIos } from '../lib/platform';
import { useStore, useTraffic } from '../lib/store';
import type { ErrorInfo, IpColumn, IpReport, ProgramView, Rule, SpeedView } from '../lib/types';
import { css } from './css';
import { FLAG, shortName } from './flags';
import { TAG, appsText, connsText, metaOf, tagOf, useLive } from './live';
import { fmtClock, liveIdOf } from './model';
import { MI } from './paths';
import { Letter, Sheet, SheetTitle, Seg, TINTS, tintOf } from './ui';
import { useWin } from './win';

const MONO = "ui-monospace, 'SF Mono', 'Cascadia Mono', Consolas, monospace";
const fadeUp = (d: string, dur = '.5s') => `fadeUp ${dur} cubic-bezier(.2,.8,.2,1) ${d} both`;

// ── Скорость ─────────────────────────────────────────────────────────────

const smoothPath = (pts: number[][]) => {
  let d = 'M' + pts[0][0].toFixed(1) + ' ' + pts[0][1].toFixed(1);
  for (let i = 1; i < pts.length; i++) {
    const [x0, y0] = pts[i - 1];
    const [x1, y1] = pts[i];
    const cx = (x0 + x1) / 2;
    d += ' C' + cx.toFixed(1) + ' ' + y0.toFixed(1) + ' ' + cx.toFixed(1) + ' ' + y1.toFixed(1) + ' ' + x1.toFixed(1) + ' ' + y1.toFixed(1);
  }
  return d;
};

/** Последний отчёт «Как вас видят сайты» живёт, пока открыто окно. */
let lastReport: IpReport | null = null;
const regionNames = (() => {
  try {
    return new Intl.DisplayNames(['ru'], { type: 'region' });
  } catch {
    return null;
  }
})();
const countryName = (c: IpColumn) => (c.country_code ? (regionNames?.of(c.country_code.toUpperCase()) ?? c.country ?? c.country_code) : c.country);
const where = (c: IpColumn) => [countryName(c), c.city].filter(Boolean).join(', ') || '—';
const flagCc = (c: IpColumn | null | undefined) => (c?.country_code && FLAG[c.country_code.toUpperCase()]) || 'var(--fill)';

export function SpeedSheet() {
  const win = useWin();
  const store = useStore();
  const { state, settings, servers, transport, toast } = store;
  const traffic = useTraffic();
  const open = win.sheet === 'speed';
  const on = state?.vpn === 'connected';
  const [pings, setPings] = useState<number[]>([]);
  const [report, setReport] = useState<IpReport | null>(lastReport);
  const [seeRun, setSeeRun] = useState(false);
  const [now, setNow] = useState(Date.now());

  // Задержка текущего сервера раз в 5 секунд, пока лист открыт и VPN работает.
  useEffect(() => {
    if (!open || !on) return;
    let alive = true;
    const tick = async () => {
      setNow(Date.now());
      const r = await transport.call<{ delay: number | null }>('delay').catch(() => null);
      if (alive && r && r.delay != null) setPings((p) => [...p, r.delay as number].slice(-24));
    };
    void tick();
    const t = setInterval(tick, 5000);
    const c = setInterval(() => setNow(Date.now()), 1000);
    return () => {
      alive = false;
      clearInterval(t);
      clearInterval(c);
    };
  }, [open, on, transport]);

  const [speed, setSpeed] = useState<SpeedView | null>(null);
  const [speedRun, setSpeedRun] = useState(false);
  const runSpeed = async () => {
    if (!on || speedRun) return;
    setSpeedRun(true);
    try {
      setSpeed(await transport.call<SpeedView>('speed_test'));
    } catch (e) {
      const err = e as ErrorInfo;
      toast(errorText(err?.code ?? 'unknown', err?.params), undefined, 'bad');
    }
    setSpeedRun(false);
  };

  const runSee = async () => {
    if (seeRun) return;
    setSeeRun(true);
    try {
      const r = await transport.call<IpReport>('ip_check');
      lastReport = r;
      setReport(r);
    } catch (e) {
      toast(errorText((e as ErrorInfo)?.code ?? 'unknown'), undefined, 'bad');
    }
    setSeeRun(false);
  };

  const dl = traffic.downHist;
  const ul = traffic.upHist;
  const mx = Math.max(10, Math.ceil((Math.max(...dl) * 1.15) / 10) * 10);
  const Wd = 316;
  const top = 10;
  const bot = 150;
  const P = (arr: number[]) => arr.map((v, i) => [(i * Wd) / (arr.length - 1), bot - Math.min(1, v / mx) * (bot - top)]);
  const dlp = P(dl);
  const dlPath = smoothPath(dlp);
  const ulPath = smoothPath(P(ul));
  const area = dlPath + ' L' + Wd + ' ' + bot + ' L0 ' + bot + ' Z';
  const dotY = dlp[dlp.length - 1][1].toFixed(1) + 'px';
  const upColor = 'color-mix(in srgb, var(--text) 45%, transparent)';
  const seen = dl.filter((v) => v > 0);
  const avg = seen.length ? seen.reduce((a, b) => a + b, 0) / seen.length : 0;
  const stats = [
    { label: 'Пик', value: Math.max(...dl).toFixed(1) + ' Мб/с', delay: '.2s' },
    { label: 'В среднем', value: avg.toFixed(1) + ' Мб/с', delay: '.25s' },
    { label: 'За сеанс', value: ((traffic.upSum + traffic.downSum) / 1024 ** 3).toFixed(2) + ' ГБ', delay: '.3s' },
  ];
  const liveId = liveIdOf(state, settings?.active_connection ?? null);
  const sv = liveId ? (servers[liveId] ?? []).find((s) => s.selected) : undefined;
  const elapsed = state?.since ? (now - state.since * 1000) / 1000 : 0;
  const sub = on && liveId ? (sv ? shortName(sv.name) + ' · ' : '') + 'сеанс ' + fmtClock(elapsed) : 'VPN выключен — показываю последнюю сессию';
  const pingNow = pings[pings.length - 1];
  const slots = Array.from({ length: 24 }, (_, i) => pings[pings.length - 24 + i]);

  const vpnOn = on;
  const rv = report?.via_vpn ?? null;
  const rd = report?.direct ?? null;
  const ready = !seeRun;
  const lead = vpnOn ? 'Сайты видят вас здесь' : 'VPN выключен — сайты видят настоящий адрес';
  const col = vpnOn ? rv : rd;
  const facts = [
    { k: 'IP-адрес', v: col?.ipv4 ?? col?.ipv6 ?? '—', font: MONO },
    { k: 'Провайдер', v: col?.provider ?? '—', font: 'inherit' },
  ];
  const yes = (v: boolean | null | undefined, good: boolean) => (v == null ? '—' : v === good ? 'да' : 'нет');
  const checks: [string, string][] = [
    ['Сайты не распознают VPN', yes(rv?.vpn_detected, false)],
    ['IPv6 не утекает', yes(report?.ipv6_leak, false)],
    ['DNS идёт через kl!ck', yes(report?.dns_protected, true)],
  ];
  const at = report ? new Date(report.checked_at * 1000) : null;
  const atText = !at ? 'ещё не проверяли' : Date.now() - at.getTime() < 60000 ? 'только что' : 'в ' + at.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });

  return (
    <Sheet open={open} onClose={win.closeSheet} label="Скорость" height={690} z={40} head={<SheetTitle title="Скорость" sub={sub} subStyle="font-size: 13px; color: var(--text2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis" />}>
      <div style={css('display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px')}>
        {(
          [
            ['Загрузка', 'var(--accent)', on ? (traffic.down * 8) / 1e6 : 0, '.05s'],
            ['Отдача', upColor, on ? (traffic.up * 8) / 1e6 : 0, '.1s'],
          ] as const
        ).map(([l, c, v, d]) => (
          <div key={l} className="st sq20" style={{ ...css('padding: 14px 16px'), animation: fadeUp(d) }}>
            <div style={css('display: flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--text2)')}>
              <span aria-hidden="true" style={{ ...css('width: 8px; height: 8px; border-radius: 50%'), background: c }} />
              {l}
            </div>
            <div style={css('display: flex; align-items: baseline; gap: 4px; margin-top: 6px')}>
              <span style={css('font-size: 28px; line-height: 32px; font-weight: 600; letter-spacing: -.5px; font-variant-numeric: tabular-nums')}>{v.toFixed(1)}</span>
              <span style={css('font-size: 13px; color: var(--text2)')}>Мб/с</span>
            </div>
          </div>
        ))}
      </div>

      <div className="st sq24" style={{ ...css('padding: 16px'), animation: fadeUp('.15s', '.55s') }}>
        <div style={css('display: flex; align-items: center; justify-content: space-between')}>
          <span style={css('font-size: 15px; font-weight: 600')}>За последнюю минуту</span>
          <span style={css('font-size: 12px; color: var(--text2)')}>Мб/с</span>
        </div>
        <div style={css('position: relative; margin-top: 12px')}>
          <svg width="100%" height="160" viewBox="0 0 316 160" preserveAspectRatio="none" aria-label="График скорости за минуту" role="img" style={css('display: block; overflow: visible')}>
            <defs>
              <linearGradient id="bmArea" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="var(--accent)" stopOpacity=".28" />
                <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
              </linearGradient>
            </defs>
            <line x1="0" y1="10" x2="316" y2="10" stroke="var(--line)" strokeWidth="1" strokeDasharray="3 4" vectorEffect="non-scaling-stroke" />
            <line x1="0" y1="80" x2="316" y2="80" stroke="var(--line)" strokeWidth="1" strokeDasharray="3 4" vectorEffect="non-scaling-stroke" />
            <line x1="0" y1="150" x2="316" y2="150" stroke="var(--line)" strokeWidth="1" vectorEffect="non-scaling-stroke" />
            <path d={area} fill="url(#bmArea)" style={css('animation: areaIn .9s cubic-bezier(.2,.8,.2,1) .7s both')} />
            <path d={ulPath} fill="none" stroke={upColor} strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" pathLength={1} strokeDasharray="1" strokeDashoffset="1" vectorEffect="non-scaling-stroke" style={css('animation: draw 1.3s cubic-bezier(.65,0,.35,1) .45s forwards')} />
            <path d={dlPath} fill="none" stroke="var(--accent)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" pathLength={1} strokeDasharray="1" strokeDashoffset="1" vectorEffect="non-scaling-stroke" style={css('animation: draw 1.3s cubic-bezier(.65,0,.35,1) .3s forwards')} />
          </svg>
          <span aria-hidden="true" style={{ ...css('position: absolute; left: calc(100% - 5px); width: 10px; height: 10px; margin-top: -5px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 3px var(--surface), 0 0 0 6px color-mix(in srgb, var(--accent) 25%, transparent); animation: fadeUp .4s ease 1.4s both; transition: top .6s cubic-bezier(.2,.8,.2,1)'), top: dotY }} />
          <span style={css('position: absolute; left: 2px; top: -2px; font-size: 10.5px; color: var(--text3); font-variant-numeric: tabular-nums')}>{mx}</span>
          <span style={css('position: absolute; left: 2px; top: 66px; font-size: 10.5px; color: var(--text3); font-variant-numeric: tabular-nums')}>{Math.round(mx / 2)}</span>
        </div>
        <div style={css('display: flex; justify-content: space-between; margin-top: 6px; font-size: 11px; color: var(--text3)')}>
          <span>−60 с</span>
          <span>−30 с</span>
          <span>сейчас</span>
        </div>
      </div>

      <div style={css('display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px')}>
        {stats.map((m) => (
          <div key={m.label} className="st sq20" style={{ ...css('padding: 12px 14px'), animation: fadeUp(m.delay) }}>
            <div style={css('font-size: 12px; color: var(--text2)')}>{m.label}</div>
            <div style={css('margin-top: 4px; font-size: 16px; font-weight: 600; font-variant-numeric: tabular-nums; white-space: nowrap')}>{m.value}</div>
          </div>
        ))}
      </div>

      <div className="st sq24" style={{ ...css('padding: 16px'), animation: fadeUp('.32s', '.55s') }}>
        <div style={css('display: flex; align-items: baseline; justify-content: space-between')}>
          <span style={css('font-size: 15px; font-weight: 600')}>Скорость сервера</span>
          <button onClick={() => void runSpeed()} aria-disabled={!on || speedRun ? 'true' : 'false'} style={{ ...css('border: 0; padding: 0; background: transparent; font-size: 13px; font-weight: 500; cursor: pointer; font-family: inherit'), color: on && !speedRun ? 'var(--accent)' : 'var(--text3)' }}>
            {speedRun ? 'Меряем…' : speed ? 'Ещё раз' : 'Проверить'}
          </button>
        </div>
        {speed && !speedRun ? (
          <div style={css('display: flex; align-items: baseline; gap: 16px; margin-top: 10px; animation: valPop .4s cubic-bezier(.2,.8,.2,1) both')}>
            <span style={css('font-size: 20px; line-height: 24px; font-weight: 600; font-variant-numeric: tabular-nums')}>↓ {speed.down != null ? speed.down.toFixed(1) : '—'}</span>
            <span style={css('font-size: 20px; line-height: 24px; font-weight: 600; font-variant-numeric: tabular-nums; color: var(--text2)')}>↑ {speed.up != null ? speed.up.toFixed(1) : '—'}</span>
            <span style={css('font-size: 13px; color: var(--text2)')}>Мб/с</span>
          </div>
        ) : null}
        <div style={css('margin-top: 6px; font-size: 12.5px; line-height: 17px; color: var(--text2)')}>
          {speedRun ? 'Качаем пробный файл через сервер — до полуминуты.' : speed?.server ? shortName(speed.server) : on ? 'Сколько реально даёт сервер.' : 'Замер — когда VPN включён.'}
        </div>
      </div>

      <div className="st sq24" style={{ ...css('padding: 16px'), animation: fadeUp('.35s', '.55s') }}>
        <div style={css('display: flex; align-items: baseline; justify-content: space-between')}>
          <span style={css('font-size: 15px; font-weight: 600')}>Задержка</span>
          <span style={css('font-size: 13px; color: var(--text2); font-variant-numeric: tabular-nums')}>
            сейчас <span style={{ fontWeight: 600, color: pingNow == null ? 'var(--text3)' : pingNow < 100 ? 'var(--ok)' : 'var(--warn)' }}>{pingNow == null ? '—' : pingNow + ' мс'}</span>
          </span>
        </div>
        <div aria-hidden="true" style={css('display: flex; align-items: flex-end; justify-content: space-between; height: 64px; margin-top: 14px')}>
          {slots.map((v, i) => (
            <span
              key={i}
              style={{
                ...css('width: 8px; border-radius: 4px; transform-origin: bottom; transition: height .5s cubic-bezier(.2,.8,.2,1)'),
                height: v == null ? 6 : Math.max(6, Math.min(64, (v / 90) * 64)),
                background: v == null ? 'var(--fill)' : v < 60 ? `color-mix(in srgb, var(--ok) ${55 + i * 2}%, transparent)` : v < 100 ? 'var(--warn)' : 'var(--bad)',
                animation: `barUp .6s cubic-bezier(.2,.8,.2,1) ${(0.45 + i * 0.022).toFixed(3)}s both`,
              }}
            />
          ))}
        </div>
        <div style={css('display: flex; justify-content: space-between; margin-top: 8px; font-size: 11px; color: var(--text3)')}>
          <span>2 мин назад</span>
          <span>сейчас</span>
        </div>
      </div>

      <div style={{ animation: fadeUp('.45s', '.55s') }}>
        <div className="st sq24" style={css('padding: 16px')}>
          <div style={css('display: flex; align-items: baseline; justify-content: space-between')}>
            <span style={css('font-size: 15px; font-weight: 600')}>Как вас видят сайты</span>
            <button onClick={() => void runSee()} style={css('border: 0; padding: 0; background: transparent; color: var(--accent); font-size: 13px; font-weight: 500; cursor: pointer; font-family: inherit')}>
              {seeRun ? 'Проверяем…' : 'Проверить'}
            </button>
          </div>
          <div style={css('display: flex; align-items: center; gap: 12px; margin-top: 14px')}>
            <span aria-hidden="true" style={{ ...css('flex: none; width: 44px; height: 44px; border-radius: 12px; box-shadow: inset 0 0 0 .4px rgba(0,0,0,.14)'), background: flagCc(col) }} />
            <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px')}>
              <span style={css('font-size: 12px; line-height: 15px; color: var(--text2)')}>{lead}</span>
              {ready ? <span style={css('font-size: 16px; line-height: 20px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; animation: valPop .4s cubic-bezier(.2,.8,.2,1) both')}>{col ? where(col) : 'Нажмите «Проверить»'}</span> : <span className="skel" style={css('width: 150px; height: 16px; margin: 2px 0')} />}
            </span>
          </div>
          <div style={css('display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; margin-top: 14px')}>
            {facts.map((f) => (
              <div key={f.k} style={css('min-width: 0; padding: 9px 12px; border-radius: 12px; background: var(--bg)')}>
                <div style={css('font-size: 11.5px; line-height: 14px; color: var(--text3)')}>{f.k}</div>
                {ready ? (
                  <div style={{ ...css('margin-top: 3px; font-size: 13.5px; line-height: 18px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-variant-numeric: tabular-nums'), fontFamily: f.font }}>{f.v}</div>
                ) : (
                  <span className="skel" style={css('display: block; width: 90%; height: 14px; margin-top: 5px')} />
                )}
              </div>
            ))}
          </div>
          {vpnOn && rd ? (
            <div style={css('margin-top: 8px; padding: 9px 12px; border-radius: 12px; box-shadow: inset 0 0 0 .4px var(--line)')}>
              <div style={css('font-size: 11.5px; line-height: 14px; color: var(--text3)')}>Без VPN</div>
              <div style={css('display: flex; align-items: center; gap: 8px; margin-top: 4px')}>
                <span aria-hidden="true" style={{ ...css('flex: none; width: 18px; height: 13px; border-radius: 3px; box-shadow: inset 0 0 0 .4px rgba(0,0,0,.15)'), background: flagCc(rd) }} />
                <span style={css('flex: 1; min-width: 0; font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{[countryName(rd), rd.provider].filter(Boolean).join(' · ') || '—'}</span>
                <span style={{ ...css('flex: none; font-size: 12px; color: var(--text2)'), fontFamily: MONO }}>{rd.ipv4 ?? rd.ipv6 ?? ''}</span>
              </div>
            </div>
          ) : null}
          <div style={css('height: .4px; margin: 14px 0 6px; background: var(--line)')} />
          {checks.map(([t, v]) => {
            const good = vpnOn && report?.via_vpn ? v === 'да' : null;
            const st = !vpnOn
              ? { v: 'VPN выключен', vc: 'var(--text3)', bg: 'var(--fill2)', fg: 'var(--text3)', ic: 'M8 12h8' }
              : good === true
                ? { v: seeRun ? '…' : v, vc: 'var(--text2)', bg: 'color-mix(in srgb, var(--ok) 16%, transparent)', fg: 'var(--ok)', ic: 'M5 12.5l4.5 4.5L19 7.5' }
                : v === 'нет'
                  ? { v: seeRun ? '…' : v, vc: 'var(--bad)', bg: 'color-mix(in srgb, var(--bad) 14%, transparent)', fg: 'var(--bad)', ic: 'M7.5 7.5l9 9M16.5 7.5l-9 9' }
                  : { v: seeRun ? '…' : '—', vc: 'var(--text3)', bg: 'var(--fill2)', fg: 'var(--text3)', ic: 'M8 12h8' };
            return (
              <div key={t} style={css('display: flex; align-items: center; gap: 10px; padding: 7px 0')}>
                <span aria-hidden="true" style={{ ...css('flex: none; width: 20px; height: 20px; border-radius: 50%; display: flex; align-items: center; justify-content: center'), background: st.bg, color: st.fg }}>
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                    <path d={st.ic} />
                  </svg>
                </span>
                <span style={css('flex: 1; min-width: 0; font-size: 13.5px')}>{t}</span>
                <span style={{ ...css('flex: none; font-size: 12.5px'), color: st.vc }}>{st.v}</span>
              </div>
            );
          })}
          <div style={css('margin-top: 8px; font-size: 11px; line-height: 15px; color: var(--text3)')}>{'Проверено ' + atText + '. Город и провайдер — примерно: базы геолокации расходятся.'}</div>
        </div>
      </div>
    </Sheet>
  );
}

// ── Добавить подключение ─────────────────────────────────────────────────

type Det = { kind: 'empty' | 'sub' | 'single' | 'text' | 'file' | 'bad'; host?: string; proto?: string; title?: string; text?: string; name?: string };

/** Что вставили: подписка (https, ключ Outline ssconf://), одна ссылка, список ссылок или конфигурация текстом —
 *  разбор общий с окном трея (lib/source), подписи — из макета. */
function detect(v: string): Det {
  v = v.trim();
  const d = detectSource(v);
  if (d.kind === 'empty') return { kind: 'empty' };
  if (d.kind === 'sub') return { kind: 'sub', host: (v.match(/^\w+:\/\/([^/:?#]+)/) || [])[1] || 'подписка' };
  if (d.kind === 'link') {
    const m = v.match(/^([a-z0-9+]+):\/\/(?:[^@\s]*@)?([^:/?#\s]+)/i);
    return { kind: 'single', host: m?.[2] ?? '', proto: (m?.[1] ?? '').toUpperCase().replace(/^HY2/, 'HYSTERIA2') };
  }
  if (d.kind === 'text') return { kind: 'text', title: d.title, text: d.text, name: d.name };
  return { kind: 'bad' };
}

const kb = (n: number) => Math.max(1, Math.round(n / 1024)) + ' КБ';
const fileKind = (name: string) => (/\.ya?ml$/i.test(name) ? 'Clash / Mihomo' : /\.json$/i.test(name) ? 'sing-box / Xray' : /\.conf$/i.test(name) ? 'WireGuard' : /\.txt$/i.test(name) ? 'Список ссылок' : 'Конфигурация');

export function AddSheet() {
  const win = useWin();
  const store = useStore();
  const open = win.sheet === 'add';
  const [tab, setTab] = useState<'link' | 'file'>('link');
  const [link, setLink] = useState('');
  const [name, setName] = useState('');
  const [from, setFrom] = useState<string | null>(null);
  const [file, setFile] = useState<File | null>(null);
  const [over, setOver] = useState(false);
  const [busy, setBusy] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const addRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!win.prefill) return;
    setTab('link');
    setLink(win.prefill.link);
    setName(win.prefill.name);
    setFrom(win.prefill.from);
    if (win.prefill.from) setTimeout(() => addRef.current?.focus(), 400);
  }, [win.prefill]);

  // Закрыли — через 420 мс, когда лист уехал, всё с начала.
  useEffect(() => {
    if (open) return;
    const t = setTimeout(() => {
      setTab('link');
      setLink('');
      setName('');
      setFrom(null);
      setFile(null);
    }, 420);
    return () => clearTimeout(t);
  }, [open]);

  const det: Det = tab === 'file' ? (file ? { kind: 'file' } : { kind: 'empty' }) : detect(link);
  const ok = (det.kind === 'sub' || det.kind === 'single' || det.kind === 'text' || det.kind === 'file') && !busy;
  const isLink = tab === 'link';
  const D = {
    empty: { t: isLink ? 'Ждём ссылку' : 'Ждём файл', x: isLink ? 'Скопируйте её у провайдера VPN и вставьте выше — тип определится сам.' : 'Конфиг Clash / Mihomo, sing-box или Xray.', bg: 'var(--fill2)', fg: 'var(--text3)', ic: 'M12 12h.01', sw: 6 },
    sub: { t: 'Ссылка на подписку', x: from ? 'Добавляйте, только если страницу подписки открыли вы сами — у своего VPN-сервиса.' : 'Подтянем серверы, лимит трафика и срок с ' + (det.host || '') + '.', bg: 'color-mix(in srgb, var(--ok) 15%, transparent)', fg: 'var(--ok)', ic: 'M5 12.5l4.5 4.5L19 7.5', sw: 2.6 },
    single: { t: 'Одиночная конфигурация', x: (det.proto || '') + ' · ' + (det.host || '') + ' — один сервер, без лимитов и срока.', bg: 'color-mix(in srgb, var(--ok) 15%, transparent)', fg: 'var(--ok)', ic: 'M5 12.5l4.5 4.5L19 7.5', sw: 2.6 },
    text: { t: det.title ?? 'Конфигурация текстом', x: det.text ?? '', bg: 'color-mix(in srgb, var(--ok) 15%, transparent)', fg: 'var(--ok)', ic: 'M5 12.5l4.5 4.5L19 7.5', sw: 2.6 },
    file: { t: 'Файл готов', x: file?.name ?? '', bg: 'color-mix(in srgb, var(--ok) 15%, transparent)', fg: 'var(--ok)', ic: 'M5 12.5l4.5 4.5L19 7.5', sw: 2.6 },
    bad: { t: 'Не похоже на ссылку', x: 'Нужна https://… от провайдера или ключ вида vless://…', bg: 'color-mix(in srgb, var(--bad) 14%, transparent)', fg: 'var(--bad)', ic: 'M7.5 7.5l9 9M16.5 7.5l-9 9', sw: 2.6 },
  }[det.kind];
  const taRing = det.kind === 'bad' ? 'inset 0 0 0 1.2px color-mix(in srgb, var(--bad) 70%, transparent)' : det.kind === 'sub' || det.kind === 'single' || det.kind === 'text' ? 'inset 0 0 0 1.2px color-mix(in srgb, var(--ok) 70%, transparent)' : 'inset 0 0 0 .4px var(--line)';

  const doAdd = async () => {
    if (!ok) return;
    setBusy(true);
    const done =
      det.kind === 'file' && file
        ? await store.importFile(file, name.trim() || undefined)
        : det.kind === 'text'
          ? await store.importText(name.trim() || det.name || 'Серверы', link.trim())
          : await store.addLink(link.trim(), name.trim() || undefined);
    setBusy(false);
    if (done) {
      win.setViewId(null);
      win.go('home');
      win.closeSheet();
    }
  };

  const paste = async () => {
    const t = await store.transport.clipboardText().catch(() => '');
    setFrom(null);
    setLink(t.trim());
  };

  const kinds = [
    { title: 'Ссылка на подписку', text: 'https://… от Remnawave, Marzban и подобных — в форматах Clash, sing-box, Xray или списком ссылок; ключ Outline ssconf://. Даёт список серверов, лимит трафика и срок действия. Обновляется сама.', icon: MI.link.d, fr: 'nonzero' },
    { title: 'Одиночная конфигурация', text: 'vless://, vmess://, trojan://, ss://, hysteria2:// — один сервер, без лимита и срока.', icon: MI.server.d, fr: 'evenodd' },
    { title: 'Файл конфигурации', text: '.yaml от Clash / Mihomo, .json от sing-box или Xray — на вкладке «Из файла».', icon: MI.doc.d, fr: 'evenodd' },
  ];

  return (
    <Sheet open={open} onClose={win.closeSheet} label="Добавить подключение" height={740} z={42} head={<SheetTitle title="Добавить подключение" />}>
      <Seg
        value={tab}
        label="Способ добавления"
        anim={fadeUp('.05s', '.45s')}
        options={[
          ['link', 'По ссылке'],
          ['file', 'Из файла'],
        ]}
        onChange={setTab}
      />

      {isLink ? (
        <>
          <div className="sf sq20" style={{ ...css('position: relative; flex: none; transition: box-shadow .25s ease'), boxShadow: taRing, animation: fadeUp('.1s') }}>
            {from ? (
              <div style={css('height: 124px; padding: 14px 16px 48px; display: flex; flex-direction: column; gap: 4px')}>
                <span className="cap2" style={css('flex: none')}>Подписка со страницы</span>
                <span style={css('flex: none; font-size: 17px; line-height: 22px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{from}</span>
                <span style={css('flex: none; font-size: 12.5px; line-height: 16px; color: var(--text2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>Остальное в ссылке — ваш ключ</span>
              </div>
            ) : (
              <textarea
                value={link}
                onChange={(e) => setLink(e.target.value)}
                aria-label="Ссылка или ключ"
                placeholder="https://panel.example.com/sub/…   или   vless://…"
                spellCheck={false}
                style={css("display: block; width: 100%; height: 124px; padding: 14px 16px 48px; border: 0; outline: none; resize: none; background: transparent; color: var(--text); font-size: 13px; line-height: 19px; font-family: ui-monospace, 'SF Mono', 'Cascadia Mono', Consolas, monospace")}
              />
            )}
            <div style={css('position: absolute; right: 10px; bottom: 10px; display: flex; gap: 6px')}>
              {link ? (
                <button
                  onClick={() => {
                    setLink('');
                    setName('');
                    setFrom(null);
                  }}
                  className="tp"
                  style={css('height: 30px; padding: 0 12px; border: 0; border-radius: 999px; background: var(--fill2); color: var(--text2); font-size: 12.5px; cursor: pointer')}
                >
                  Очистить
                </button>
              ) : null}
              <button onClick={() => void paste()} className="tp" style={css('display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 12px; border: 0; border-radius: 999px; background: var(--fill2); color: var(--text); font-size: 12.5px; font-weight: 500; cursor: pointer')}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <rect x="8.5" y="3" width="7" height="4" rx="1.2" />
                  <path d="M15.5 5H17a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h1.5" />
                </svg>
                Вставить из буфера
              </button>
            </div>
          </div>
          <Detect D={D} />
        </>
      ) : (
        <>
          {!file ? (
            <button
              onClick={() => fileRef.current?.click()}
              onDragOver={(e) => {
                e.preventDefault();
                setOver(true);
              }}
              onDragLeave={() => setOver(false)}
              onDrop={(e) => {
                e.preventDefault();
                setOver(false);
                const f = e.dataTransfer.files[0];
                if (f) setFile(f);
              }}
              className="tp"
              style={{ ...css('flex: none; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; width: 100%; height: 200px; border-radius: 24px; background: var(--surface); color: var(--text); cursor: pointer'), border: over ? '1.5px dashed var(--accent)' : '1.5px dashed color-mix(in srgb, var(--text3) 70%, transparent)', animation: fadeUp('.1s') }}
            >
              <span aria-hidden="true" style={css('width: 52px; height: 52px; margin-bottom: 6px; border-radius: 50%; background: var(--fill2); color: var(--text2); display: flex; align-items: center; justify-content: center')}>
                <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                  <path fillRule="evenodd" d={MI.doc.d} />
                </svg>
              </span>
              <span style={css('font-size: 15px; font-weight: 600')}>Перетащите файл сюда</span>
              <span style={css('font-size: 13px; color: var(--text2)')}>или нажмите, чтобы выбрать</span>
              <span style={css('font-size: 12px; color: var(--text3)')}>.yaml · .yml · .json · .txt</span>
            </button>
          ) : (
            <div className="sf sq20" style={css('flex: none; display: flex; align-items: center; gap: 12px; padding: 12px 14px; animation: valPop .4s cubic-bezier(.2,.8,.2,1) both')}>
              <span aria-hidden="true" style={css('flex: none; width: 40px; height: 40px; border-radius: 12px; background: color-mix(in srgb, var(--ok) 14%, transparent); color: var(--ok); display: flex; align-items: center; justify-content: center')}>
                <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                  <path fillRule="evenodd" d={MI.doc.d} />
                </svg>
              </span>
              <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
                <span style={css('font-size: 14px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{file.name}</span>
                <span style={css('font-size: 12.5px; color: var(--text2)')}>
                  {kb(file.size)} · {fileKind(file.name)}
                </span>
              </span>
              <button onClick={() => fileRef.current?.click()} className="tp" style={css('height: 30px; padding: 0 12px; border: 0; border-radius: 999px; background: var(--fill2); color: var(--text); font-size: 12.5px; cursor: pointer')}>
                Заменить
              </button>
            </div>
          )}
          <input
            ref={fileRef}
            type="file"
            accept=".yaml,.yml,.json,.txt,.conf"
            hidden
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) setFile(f);
              e.target.value = '';
            }}
          />
        </>
      )}

      {ok || (busy && det.kind !== 'empty') ? (
        <div className="sf sq12" style={css('flex: none; display: flex; align-items: center; height: 44px; padding: 0 14px; animation: fadeUp .35s cubic-bezier(.2,.8,.2,1) both')}>
          <input value={name} onChange={(e) => setName(e.target.value)} aria-label="Название" placeholder="Название (необязательно)" style={css('flex: 1; min-width: 0; border: 0; outline: none; background: transparent; color: var(--text); font-size: 14px; font-family: inherit')} />
        </div>
      ) : null}

      <button
        ref={addRef}
        onClick={() => void doAdd()}
        aria-disabled={ok ? 'false' : 'true'}
        className="tp"
        style={{ ...css('flex: none; height: 48px; border: 0; border-radius: 999px; font-size: 15px; font-weight: 600; transition: background-color .25s ease, color .25s ease'), background: ok ? 'var(--accent)' : 'var(--fill2)', color: ok ? 'var(--onAccent)' : 'var(--text3)', cursor: ok ? 'pointer' : 'default', animation: fadeUp('.2s') }}
      >
        {busy ? 'Добавляем…' : det.kind === 'single' ? 'Добавить конфигурацию' : 'Добавить подписку'}
      </button>

      <div className="cap2" style={{ ...css('margin-top: 12px; padding: 0 4px'), animation: fadeUp('.25s') }}>
        Что можно вставить
      </div>
      <div className="sf sq24" style={{ ...css('flex: none; padding: 2px 16px'), animation: fadeUp('.3s') }}>
        {kinds.map((k, i) => (
          <div key={k.title}>
            {i > 0 ? <div style={css('height: .4px; background: var(--line)')} /> : null}
            <div style={css('display: flex; align-items: flex-start; gap: 12px; padding: 14px 0')}>
              <span aria-hidden="true" style={css('flex: none; width: 36px; height: 36px; border-radius: 50%; background: var(--fill2); color: var(--text); display: flex; align-items: center; justify-content: center')}>
                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                  <path fillRule={k.fr as 'evenodd' | 'nonzero'} d={k.icon} />
                </svg>
              </span>
              <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px')}>
                <span style={css('font-size: 14px; line-height: 18px; font-weight: 600')}>{k.title}</span>
                <span style={css('font-size: 12.5px; line-height: 17px; color: var(--text2)')}>{k.text}</span>
              </span>
            </div>
          </div>
        ))}
      </div>
    </Sheet>
  );
}

function Detect({ D }: { D: { t: string; x: string; bg: string; fg: string; ic: string; sw: number } }) {
  return (
    <div className="st sq20" role="status" aria-live="polite" style={{ ...css('flex: none; display: flex; align-items: center; gap: 12px; padding: 12px 14px'), animation: fadeUp('.15s') }}>
      <span aria-hidden="true" style={{ ...css('flex: none; width: 32px; height: 32px; border-radius: 50%; display: flex; align-items: center; justify-content: center; transition: background-color .25s ease, color .25s ease'), background: D.bg, color: D.fg }}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={D.sw} strokeLinecap="round" strokeLinejoin="round">
          <path d={D.ic} />
        </svg>
      </span>
      <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
        <span style={css('font-size: 14px; line-height: 18px; font-weight: 600')}>{D.t}</span>
        <span style={css('font-size: 12.5px; line-height: 17px; color: var(--text2)')}>{D.x}</span>
      </span>
    </div>
  );
}

// ── Мои исключения / Мои сайты и приложения ──────────────────────────────

const isIp = (v: string) => /^[\d.]+(\/\d+)?$/.test(v) || (v.includes(':') && /^[\da-f:./]+$/i.test(v));

export function RulesSheet() {
  const win = useWin();
  const store = useStore();
  const { settings, transport, programNames } = store;
  const open = win.sheet === 'rules';
  const [tab, setTab] = useState<'sites' | 'apps'>('sites');
  const [input, setInput] = useState('');
  const [fresh, setFresh] = useState('');
  useEffect(() => {
    if (open) {
      setInput('');
      setFresh('');
    }
  }, [open]);
  if (!settings) return null;
  const position = settings.routing;
  const all = position === 'all_vpn';
  const route = all ? 'direct' : 'vpn';
  const list = settings.lists[position];
  const items = list
    .map((rule, index) => ({ rule, index }))
    .filter(({ rule }) => (tab === 'sites' ? rule.target.kind === 'domain' || rule.target.kind === 'ip' : rule.target.kind === 'program'));
  const title = (r: Rule) => {
    const t = r.target;
    if (t.kind === 'program') return programNames[t.value] ?? t.value.split('\\').filter(Boolean).pop()?.replace(/\.exe$/i, '') ?? t.value;
    if (t.kind === 'domain') return t.value.includes('.') ? t.value : '.' + t.value;
    return t.value.replace(/\/(32|128)$/, '');
  };

  const add = async () => {
    const raw = input.trim();
    if (!raw) return;
    if (tab === 'sites') {
      const v = raw.replace(/^https?:\/\//i, '').replace(/\/.*$/, '').toLowerCase();
      const target = isIp(v) ? ({ kind: 'ip', value: v } as const) : ({ kind: 'domain', value: v.replace(/^\./, '') } as const);
      if (list.some((r) => r.target.kind === target.kind && r.target.value === target.value)) {
        setInput('');
        return;
      }
      const n = await store.addRules(position, [{ target, route, enabled: true }]);
      if (n) {
        setInput('');
        setFresh(target.kind === 'domain' && !target.value.includes('.') ? '.' + target.value : target.value);
        if (store.isRunning) store.toast('Маршруты применены', undefined, 'ok');
      }
      return;
    }
    // Приложение: ищем среди запущенных по названию, иначе — выбрать .exe.
    const q = raw.toLowerCase().replace(/\.exe$/, '');
    const running = await transport.call<ProgramView[]>('programs').catch(() => [] as ProgramView[]);
    const hit = running.find((p) => p.name.toLowerCase() === q || (p.path.split('\\').pop() ?? '').toLowerCase().replace(/\.exe$/, '') === q) ?? running.find((p) => p.name.toLowerCase().includes(q));
    const path = hit?.folder ? hit.path : await transport.pickExe();
    if (!path) return;
    const n = await store.addRules(position, [{ target: { kind: 'program', value: path }, route, enabled: true }]);
    if (n) {
      setInput('');
      setFresh(hit?.name ?? '');
      if (store.isRunning) store.toast('Маршруты применены', undefined, 'ok');
    }
  };

  const has = input.trim().length > 0;
  return (
    <Sheet open={open} onClose={win.closeSheet} label={all ? 'Мои исключения' : isIos ? 'Мои сайты' : 'Мои сайты и приложения'} height={640} z={44} head={<SheetTitle title={all ? 'Мои исключения' : isIos ? 'Мои сайты' : 'Мои сайты и приложения'} sub={all ? 'Пойдут напрямую, мимо VPN' : 'Пойдут через VPN'} />}>
      {isIos ? null : (
      <Seg
        value={tab}
        anim={fadeUp('.05s', '.45s')}
        options={[
          ['sites', 'Сайты'],
          ['apps', 'Приложения'],
        ]}
        onChange={(t) => {
          setTab(t);
          setInput('');
          setFresh('');
        }}
      />
      )}
      <div className="sf sq12" style={{ ...css('flex: none; display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 6px 0 14px'), animation: fadeUp('.1s', '.45s') }}>
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void add()}
          aria-label={tab === 'sites' ? 'domain.com или .ru' : 'Название программы, например Steam'}
          placeholder={tab === 'sites' ? 'domain.com или .ru' : 'Название программы, например Steam'}
          style={css('flex: 1; min-width: 0; border: 0; outline: none; background: transparent; color: var(--text); font-size: 14px; font-family: inherit')}
        />
        <button onClick={() => void add()} className="tp" style={{ ...css('height: 36px; padding: 0 14px; border: 0; border-radius: 999px; font-size: 13.5px; font-weight: 600; cursor: pointer; font-family: inherit'), background: has ? 'var(--accent)' : 'var(--fill2)', color: has ? 'var(--onAccent)' : 'var(--text3)' }}>
          Добавить
        </button>
      </div>
      {items.length ? (
        <div className="sf sq20" style={{ ...css('flex: none; padding: 6px 16px'), animation: fadeUp('.15s', '.45s') }}>
          {items.map(({ rule, index }, i) => {
            const name = title(rule);
            return (
              <div key={rule.target.kind + rule.target.value}>
                {i > 0 ? <div style={css('height: .4px; margin-left: 44px; background: var(--line)')} /> : null}
                <div className="rrow" style={{ minHeight: 52, animation: name === fresh || rule.target.value === fresh ? 'valPop .4s cubic-bezier(.2,.8,.2,1) both' : 'none', opacity: rule.enabled === false ? 0.5 : 1 }} title={rule.target.kind === 'program' ? rule.target.value : undefined}>
                  <Letter name={name} tint={tab === 'apps' ? tintOf(name) : TINTS[i % 6]} />
                  <span style={css('flex: 1; min-width: 0; font-size: 14px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{name}</span>
                  {rule.route !== route ? (
                    <span className="tagc" style={{ background: rule.route === 'vpn' ? 'color-mix(in srgb, var(--accent) 14%, transparent)' : rule.route === 'block' ? 'color-mix(in srgb, var(--bad) 14%, transparent)' : 'var(--fill2)', color: rule.route === 'vpn' ? 'var(--accent)' : rule.route === 'block' ? 'var(--bad)' : 'var(--text2)' }}>
                      {rule.route === 'vpn' ? 'VPN' : rule.route === 'block' ? 'Блок' : 'Напрямую'}
                    </span>
                  ) : null}
                  <button onClick={() => void store.removeRule(position, index)} aria-label={'Удалить ' + name} className="tp trb" style={css('flex: none; height: 30px; padding: 0 10px; margin-right: -10px; border: 0; border-radius: 999px; font-size: 13.5px; font-weight: 500; font-family: inherit; cursor: pointer')}>
                    Удалить
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="note" style={css('text-align: center; padding: 20px 0')}>
          {tab === 'sites' ? 'Сайтов пока нет' : 'Приложений пока нет'}
        </div>
      )}
      <div className="note" style={css('padding: 0 4px')}>
        {tab === 'sites' ? 'Правило действует на домен и все его поддомены.' : 'Правило для программы целиком — удобно для игр и торрентов. Работает в режиме VPN (TUN).'}
      </div>
    </Sheet>
  );
}

// ── Программы в сети ─────────────────────────────────────────────────────

export function ConnsSheet() {
  const win = useWin();
  const open = win.sheet === 'conns';
  const { apps } = useLive(open);
  const { isRunning } = useStore();
  const total = apps.reduce((a, x) => a + x.conns, 0);
  const sorted = apps.slice().sort((a, b) => b.bytes - a.bytes);
  const maxB = Math.max(1, ...sorted.map((a) => a.bytes));
  return (
    <Sheet open={open} onClose={win.closeSheet} label={isIos ? 'Все сайты' : 'Все программы'} height={640} z={44} bodyStyle="flex: 1; min-height: 0; overflow-y: auto; padding: 20px 16px 24px" head={<SheetTitle title={isIos ? 'Сайты в сети' : 'Программы в сети'} sub={connsText(total) + ' · ' + appsText(apps.length)} subStyle="font-size: 13px; color: var(--text2); font-variant-numeric: tabular-nums" />}>
      <div className="sf sq20" style={css('padding: 4px 16px')}>
        {sorted.map((a, i) => {
          const tg = TAG[tagOf(a)];
          const delay = (0.05 + i * 0.05).toFixed(2) + 's';
          return (
            <div key={a.key}>
              {i > 0 ? <div style={css('height: .4px; margin-left: 44px; background: var(--line)')} /> : null}
              <div style={{ ...css('display: flex; align-items: center; gap: 12px; padding: 12px 0'), animation: `fadeUp .45s cubic-bezier(.2,.8,.2,1) ${delay} both` }}>
                <Letter name={a.name} tint={tintOf(a.name)} />
                <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px')}>
                  <span style={css('font-size: 14px; line-height: 18px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{a.name}</span>
                  <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2); white-space: nowrap; font-variant-numeric: tabular-nums')}>{metaOf(a)}</span>
                  <span aria-hidden="true" style={css('height: 4px; border-radius: 999px; background: var(--fill); overflow: hidden')}>
                    <span style={{ ...css('display: block; height: 4px; border-radius: 999px; transform-origin: left'), width: Math.max(6, (a.bytes / maxB) * 100).toFixed(0) + '%', background: tagOf(a) === 'dir' ? 'var(--text3)' : 'var(--accent)', animation: `growbar .8s cubic-bezier(.2,.8,.2,1) ${delay} both` }} />
                  </span>
                </span>
                <span className="tagc" style={{ background: tg[1], color: tg[2] }}>
                  {tg[0]}
                </span>
              </div>
            </div>
          );
        })}
        {!sorted.length ? (
          <div className="note" style={css('text-align: center; padding: 20px 0')}>
            {isRunning ? 'Пока никто не ходит в сеть' : 'VPN выключен — программы появятся после подключения'}
          </div>
        ) : null}
      </div>
      <div className="note" style={css('margin-top: 10px; padding: 0 4px')}>
        {isIos ? 'Видны сайты, которые сейчас открыты. Маршрут каждого — по вашим правилам.' : 'Видны программы, которые сейчас ходят в сеть. Маршрут каждой — по вашим правилам.'}
      </div>
    </Sheet>
  );
}

// ── Способ подключения ───────────────────────────────────────────────────

const IC2 = {
  pc: 'M4 5h16v11H4zM9 20h6M12 16v4',
  game: 'M7 7h10a4 4 0 0 1 4 4v3a2.5 2.5 0 0 1-4.5 1.5L15 14H9l-1.5 1.5A2.5 2.5 0 0 1 3 14v-3a4 4 0 0 1 4-4zM7 10.5h3M8.5 9v3M15.5 10.5h.01M17.5 12h.01',
  net: 'M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18zM3 12h18M12 3c2.4 2.6 3.6 5.6 3.6 9s-1.2 6.4-3.6 9c-2.4-2.6-3.6-5.6-3.6-9S9.6 5.6 12 3z',
};

export function MethodSheet() {
  const win = useWin();
  const store = useStore();
  const { settings } = store;
  const open = win.sheet === 'method';
  const [mode, setModeLocal] = useState(settings?.mode ?? 'tun');
  useEffect(() => {
    if (settings) setModeLocal(settings.mode);
  }, [settings?.mode]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!settings) return null;
  const tun = mode === 'tun';
  const setMethod = (m: 'tun' | 'sys_proxy') => {
    if (m === mode) return;
    setModeLocal(m);
    void store.setMode(m).then(() => {
      if (store.state?.vpn === 'connected') store.toast(m === 'tun' ? 'Переключились на VPN (TUN)' : 'Переключились на системный прокси', undefined, 'ok');
    });
  };
  const accentChip = { bg: 'color-mix(in srgb, var(--accent) 14%, transparent)', fg: 'var(--text)', ring: 'inset 0 0 0 1px color-mix(in srgb, var(--accent) 55%, transparent)', dot: 'var(--accent)' };
  const grayChip = { bg: 'var(--surface)', fg: 'var(--text2)', ring: 'inset 0 0 0 .4px var(--line)', dot: 'var(--text3)' };
  type N = { x: number; y: number; label: string; op: number; box?: { sub: string; ic: string }; chip?: typeof accentChip };
  const nodes: N[] = tun
    ? [
        { x: 52, y: 95, label: 'Компьютер', op: 1, box: { sub: 'все программы', ic: IC2.pc } },
        { x: 52, y: 150, label: 'Игры', op: 0, box: { sub: 'и часть программ', ic: IC2.game } },
        { x: 174, y: 95, label: 'kl!ck · VPN', op: 1, chip: accentChip },
        { x: 174, y: 150, label: 'Напрямую', op: 0, chip: grayChip },
        { x: 296, y: 95, label: 'Интернет', op: 1, box: { sub: 'сайты', ic: IC2.net } },
      ]
    : [
        { x: 52, y: 52, label: 'Браузеры', op: 1, box: { sub: 'Telegram и др.', ic: IC2.pc } },
        { x: 52, y: 140, label: 'Игры', op: 1, box: { sub: 'и часть программ', ic: IC2.game } },
        { x: 174, y: 52, label: 'Системный прокси', op: 1, chip: accentChip },
        { x: 174, y: 140, label: 'Напрямую', op: 1, chip: grayChip },
        { x: 296, y: 95, label: 'Интернет', op: 1, box: { sub: 'сайты', ic: IC2.net } },
      ];
  const points = tun
    ? ['Весь трафик компьютера идёт через VPN: браузеры, игры, Telegram, торренты.', 'Нужны права администратора — Windows спросит один раз при установке.']
    : ['Windows сама передаёт прокси программам — браузеры и большинство приложений подхватят его.', 'Игры, UDP-трафик и программы, которые не слушают настройки Windows, идут напрямую.'];
  const ks = settings.kill_switch.enabled;
  return (
    <Sheet open={open} onClose={win.closeSheet} label="Способ подключения" height={600} z={46} fade={false} bodyStyle="flex: 1; min-height: 0; overflow-y: auto; padding: 12px 16px 24px; display: flex; flex-direction: column; gap: 12px" head={<SheetTitle title="Способ подключения" />}>
      <Seg
        role="radiogroup"
        label="Способ подключения"
        anim={fadeUp('.05s', '.45s')}
        value={mode}
        options={[
          ['tun', 'VPN (TUN)'],
          ['sys_proxy', 'Системный прокси'],
        ]}
        onChange={setMethod}
      />
      <div className="sf sq24" style={{ ...css('position: relative; flex: none; height: 190px; overflow: hidden'), animation: fadeUp('.1s') }}>
        <svg width="100%" height="190" viewBox="0 0 348 190" aria-hidden="true" style={css('position: absolute; inset: 0')}>
          <path d={tun ? 'M76 95 L120 95 M228 95 L272 95' : 'M76 52 L104 52 M244 52 C262 52 262 92 274 94'} fill="none" stroke="var(--accent)" strokeWidth="2" strokeDasharray="5 6" strokeLinecap="round" style={css('animation: flow .9s linear infinite')} />
          <path d={tun ? 'M76 150 L140 150' : 'M76 140 L128 140 M220 140 C258 140 262 100 274 98'} fill="none" stroke="var(--text3)" strokeWidth="2" strokeDasharray="5 6" strokeLinecap="round" style={{ animation: 'flow 1.3s linear infinite', opacity: tun ? 0 : 1 }} />
        </svg>
        {nodes.map((n, i) => (
          <div key={i} style={{ ...css('position: absolute; transform: translate(-50%, -50%); display: flex; flex-direction: column; align-items: center; gap: 4px; text-align: center; transition: left .5s cubic-bezier(.32,.72,0,1), top .5s cubic-bezier(.32,.72,0,1), opacity .3s ease'), left: n.x, top: n.y, opacity: n.op }}>
            {n.chip ? (
              <span style={{ ...css('display: inline-flex; align-items: center; gap: 6px; height: 28px; padding: 0 12px; border-radius: 999px; font-size: 12.5px; font-weight: 600; white-space: nowrap'), background: n.chip.bg, color: n.chip.fg, boxShadow: n.chip.ring }}>
                <span aria-hidden="true" style={{ ...css('width: 6px; height: 6px; border-radius: 50%'), background: n.chip.dot }} />
                {n.label}
              </span>
            ) : null}
            {n.box ? (
              <>
                <span aria-hidden="true" style={css('width: 40px; height: 40px; border-radius: 12px; background: var(--fill2); color: var(--text); display: flex; align-items: center; justify-content: center')}>
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                    <path d={n.box.ic} />
                  </svg>
                </span>
                <span style={css('font-size: 12px; font-weight: 600; white-space: nowrap')}>{n.label}</span>
                <span style={css('margin-top: -3px; font-size: 11px; color: var(--text2); white-space: nowrap')}>{n.box.sub}</span>
              </>
            ) : null}
          </div>
        ))}
      </div>
      <div className="sf sq20" style={{ ...css('flex: none; padding: 4px 16px'), animation: fadeUp('.15s') }}>
        {points.map((t, i) => (
          <div key={t}>
            {i > 0 ? <div style={css('height: .4px; background: var(--line)')} /> : null}
            <div style={css('display: flex; gap: 10px; padding: 10px 0; font-size: 13px; line-height: 18px')}>
              <span aria-hidden="true" style={{ ...css('flex: none; width: 7px; height: 7px; margin-top: 6px; border-radius: 50%'), background: i === 0 ? 'var(--accent)' : 'var(--text3)' }} />
              <span style={css('color: var(--text2)')}>{t}</span>
            </div>
          </div>
        ))}
      </div>
      {!tun && ks ? (
        <div className="sq20" style={css('flex: none; display: flex; gap: 10px; padding: 12px 14px; background: color-mix(in srgb, var(--warn) 12%, transparent); font-size: 13px; line-height: 18px; animation: valPop .4s cubic-bezier(.2,.8,.2,1) both')}>
          <span aria-hidden="true" style={css('flex: none; font-weight: 700; color: var(--warn)')}>
            !
          </span>
          <span>Kill Switch в режиме прокси надёжен не для всех программ — лучше VPN (TUN).</span>
        </div>
      ) : null}
      <button onClick={win.closeSheet} className="tp" style={{ ...css('flex: none; height: 48px; border: 0; border-radius: 999px; background: var(--accent); color: var(--onAccent); font-size: 15px; font-weight: 600; cursor: pointer; font-family: inherit'), animation: fadeUp('.2s') }}>
        Готово
      </button>
    </Sheet>
  );
}

