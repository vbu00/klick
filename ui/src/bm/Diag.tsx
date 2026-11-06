// «Диагностика» по макету: проверка серверов подписки из этой сети (служба: probe — работает ли сервер,
// режет ли его провайдер, подвисает ли загрузка) и «Не открылось недавно».

import { useEffect, useState } from 'react';
import { errorText, failureReason } from '../lib/i18n';
import { protocolName } from '../lib/format';
import { siteRule, siteTarget } from '../lib/live';
import { betterServer, probeCounts } from '../lib/probe';
import { unicodeDomain } from '../lib/rules';
import { useStore } from '../lib/store';
import type { ServerView } from '../lib/types';
import { css } from './css';
import { cleanName, flagOf } from './flags';
import { useLive } from './live';
import { BackBtn, Cap, Group, ToggleRow, H1 } from './ui';
import { useWin } from './win';

const OK = ['color-mix(in srgb, var(--ok) 14%, transparent)', 'var(--ok)'];
const WARN = ['color-mix(in srgb, var(--warn) 16%, transparent)', 'var(--warn)'];
const BAD = ['color-mix(in srgb, var(--bad) 14%, transparent)', 'var(--bad)'];

/** Метка итога у сервера: подписи из макета. */
function tagOf(s: ServerView): [string, string, string] | null {
  const p = s.probe;
  if (!p) return null;
  const ms = p.delay_ms ?? s.delay;
  switch (p.verdict) {
    case 'ok':
      return p.flaky ? ['Нестабильно', WARN[0], WARN[1]] : ['Работает' + (ms != null ? ' · ' + ms + ' мс' : ''), OK[0], OK[1]];
    case 'freeze':
      return ['Подвисает', WARN[0], WARN[1]];
    case 'tcp_blocked':
    case 'handshake_failed':
      return ['Режет провайдер', WARN[0], WARN[1]];
    default:
      return ['Не отвечает', BAD[0], BAD[1]];
  }
}

const clock = (since: number) => {
  const s = Math.max(0, Math.floor(Date.now() / 1000 - since));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
};

export function Diag({ backLabel }: { backLabel: string }) {
  const store = useStore();
  const win = useWin();
  const { settings, state, servers, probing, probeError, isRunning, catalog } = store;
  const { fails } = useLive(true);
  const [, tick] = useState(0);
  useEffect(() => {
    if (probing == null) return;
    const t = setInterval(() => tick((x) => x + 1), 1000);
    return () => clearInterval(t);
  }, [probing]);
  if (!settings || !state) return null;
  const conn = state.connection;
  const act = settings.connections.find((c) => c.id === (conn?.id ?? settings.active_connection));
  const list: ServerView[] = act ? (servers[act.id] ?? []) : [];
  const last = conn?.probe_last ?? null;
  const busy = probing != null;
  const error = !busy && act && probeError?.id === act.id ? probeError.error : null;
  const done = !busy && !!last;
  const c = probeCounts(list);
  const current = list.find((s) => s.selected) ?? list.find((s) => s.name === state.server);
  const better = betterServer(list, current);
  const tunOn = state.mode === 'tun' && isRunning;
  const listRules = settings.lists[settings.routing];

  const hourAgo = Date.now() / 1000 - 3600;
  const recent = fails.filter((f) => f.at >= hourAgo).length;
  const seen = new Set<string>();
  const failRows = fails.filter((f) => (seen.has(f.host) ? false : (seen.add(f.host), true))).slice(0, 8);
  const shown = busy || done ? list : [];
  const advice = better
    ? 'Ваш сервер здесь не работает — переключитесь на «' + cleanName(better.name, better.kind) + '»: он работает в этой сети.'
    : c.ok < c.checked
      ? 'Выберите на главной «Авто» или один из рабочих серверов.'
      : 'Всё в порядке.';

  return (
    <div style={css('display: flex; flex-direction: column; animation: pushin .32s cubic-bezier(.2,.7,.2,1)')}>
      <BackBtn label={backLabel} onClick={win.back} />
      <H1>Диагностика</H1>

      <Cap>Серверы</Cap>
      <div className="sf sq24" style={css('margin-top: 8px; padding: 16px')}>
        <div style={css('font-size: 13.5px; line-height: 19px; color: var(--text2)')}>
          {act ? 'Проверим из вашей сети каждый сервер «' + act.name + '»: какие работают, какие режет провайдер, а какие подвисают на загрузке.' : 'Сначала добавьте подписку — проверять пока нечего.'}
        </div>
        {shown.length ? (
          <div style={css('display: flex; flex-direction: column; gap: 2px; margin-top: 12px')}>
            {shown.map((x) => {
              const t = tagOf(x);
              return (
                <div key={x.name} style={css('display: flex; align-items: center; gap: 12px; padding: 8px 0')}>
                  <span aria-hidden="true" style={{ ...css('flex: none; width: 28px; height: 20px; border-radius: 5px; box-shadow: inset 0 0 0 1px rgba(0,0,0,.06)'), background: flagOf(x.name, x.kind) }} />
                  <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
                    <span style={css('font-size: 14px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{cleanName(x.name, x.kind)}</span>
                    <span style={css('font-size: 12px; color: var(--text2)')}>{protocolName(x.kind)}</span>
                  </span>
                  {busy ? <span className="skel" style={css('width: 64px')} /> : null}
                  {!busy && t ? (
                    <span className="tagc" title={x.probe?.flaky ? 'Часть попыток прошла, часть — нет' : undefined} style={{ ...css('animation: valPop .4s cubic-bezier(.2,.8,.2,1) both'), background: t[1], color: t[2] }}>
                      {t[0]}
                    </span>
                  ) : null}
                </div>
              );
            })}
          </div>
        ) : null}
        {done && c.checked ? (
          <div style={css('margin-top: 8px; padding: 10px 12px; border-radius: 12px; background: var(--bg); font-size: 13px; line-height: 18px; animation: fadeUp .4s ease both')}>
            <span style={css('font-weight: 600')}>{c.ok + ' из ' + c.checked + ' работают.'}</span> <span style={css('color: var(--text2)')}>{advice}</span>
          </div>
        ) : null}
        {error ? (
          <div role="alert" style={css('margin-top: 8px; padding: 10px 12px; border-radius: 12px; background: color-mix(in srgb, var(--bad) 10%, transparent); font-size: 13px; line-height: 18px')}>
            <span style={css('font-weight: 600')}>Проверка не удалась.</span> <span style={css('color: var(--text2)')}>{errorText(error.code, error.params)}</span>
          </div>
        ) : null}
        <button
          onClick={() => !busy && act && void store.probe()}
          aria-disabled={busy || !act ? 'true' : 'false'}
          className="tp"
          style={{ ...css('width: 100%; height: 44px; margin-top: 14px; border: 0; border-radius: 999px; font-size: 14.5px; font-weight: 600; cursor: pointer; font-family: inherit'), background: busy || !act ? 'var(--fill2)' : 'var(--accent)', color: busy || !act ? 'var(--text2)' : 'var(--onAccent)' }}
        >
          {busy && probing != null ? 'Проверяем… ' + clock(probing) : done ? 'Проверить ещё раз' : 'Проверить серверы'}
        </button>
        <div className="note" style={css('margin-top: 8px; text-align: center')}>
          Обычно 15–40 секунд
        </div>
        {act && tunOn ? (
          <div className="note" style={css('margin-top: 8px')}>
            Включён VPN (TUN): заморозку проверим, а пускает ли провайдер к серверам напрямую и не подменяет ли DNS — нет. Для полной проверки выключите VPN.
          </div>
        ) : null}
      </div>
      {act?.info?.probe_report ? (
        <Group>
          {[
            <ToggleRow
              key="share"
              title="Отправлять итоги провайдеру"
              sub="Какие серверы работают в вашей сети — без адресов сайтов"
              on={!!conn?.probe_share}
              onChange={(v) => void store.probeShare(v)}
            />,
          ]}
        </Group>
      ) : null}

      <div style={css('display: flex; align-items: flex-end; justify-content: space-between; margin-top: 24px; padding: 0 4px')}>
        <span className="cap2">Не открылось недавно</span>
        <span style={css('font-size: 13px; color: var(--text2); font-variant-numeric: tabular-nums')}>{recent} за час</span>
      </div>
      <div className="sf sq20" style={css('margin-top: 8px; padding: 0 16px')}>
        {failRows.map((f, i) => {
          const host = unicodeDomain(f.host);
          const target = siteTarget(f.host, catalog);
          const rule = siteRule(listRules, f.host, catalog);
          const vpn = rule ? rule.route === 'vpn' : f.route === 'vpn';
          const label = target.kind === 'service' ? (catalog.find((s) => s.id === target.value)?.name ?? host) : unicodeDomain(target.value);
          return (
            <div key={f.host + f.at}>
              {i > 0 ? <div style={css('height: .4px; background: var(--line)')} /> : null}
              <div className="rrow" style={{ minHeight: 56 }}>
                <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
                  <span style={css('font-size: 14px; line-height: 18px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{host}</span>
                  <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2)')}>{(f.route === 'vpn' ? 'Через VPN' : f.route === 'direct' ? 'Напрямую' : 'Заблокировано') + ' · ' + failureReason(f.error)}</span>
                </span>
                <button onClick={() => void store.routeTarget(target, !vpn, label)} className="tp" style={css('flex: none; height: 30px; padding: 0 12px; border: 0; border-radius: 999px; background: var(--fill2); color: var(--text); font-size: 12.5px; font-weight: 500; cursor: pointer; font-family: inherit')}>
                  {vpn ? 'Напрямую' : 'Через VPN'}
                </button>
              </div>
            </div>
          );
        })}
        {!failRows.length ? (
          <div className="note" style={css('text-align: center; padding: 20px 0')}>
            {isRunning ? 'Пока всё открывается' : 'Появится, когда VPN включён'}
          </div>
        ) : null}
      </div>
    </div>
  );
}
