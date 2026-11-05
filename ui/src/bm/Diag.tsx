// «Диагностика» по макету: проверка серверов подписки и «Не открылось недавно».

import { useState } from 'react';
import { failureReason } from '../lib/i18n';
import { protocolName } from '../lib/format';
import { siteRule, siteTarget } from '../lib/live';
import { unicodeDomain } from '../lib/rules';
import { useStore } from '../lib/store';
import type { ServerView } from '../lib/types';
import { css } from './css';
import { cleanName, flagOf } from './flags';
import { useLive } from './live';
import { BackBtn, Cap, H1 } from './ui';
import { useWin } from './win';

export function Diag({ backLabel }: { backLabel: string }) {
  const store = useStore();
  const win = useWin();
  const { settings, catalog } = store;
  const [run, setRun] = useState(false);
  const [done, setDone] = useState(false);
  const [rows, setRows] = useState<ServerView[]>([]);
  const { fails } = useLive(true);
  if (!settings) return null;
  const act = settings.connections.find((c) => c.id === settings.active_connection);
  const list = settings.lists[settings.routing];

  const runDiag = async () => {
    if (run || !act) return;
    setRun(true);
    setDone(false);
    setRows(store.servers[act.id] ?? []);
    await store.testLatency();
    setRun(false);
    setDone(true);
  };
  const shown = done ? (store.servers[act?.id ?? ''] ?? rows) : rows;
  const okN = shown.filter((x) => x.delay != null).length;
  const hourAgo = Date.now() / 1000 - 3600;
  const recent = fails.filter((f) => f.at >= hourAgo).length;
  const seen = new Set<string>();
  const failRows = fails.filter((f) => (seen.has(f.host) ? false : (seen.add(f.host), true))).slice(0, 8);

  return (
    <div style={css('display: flex; flex-direction: column; animation: pushin .32s cubic-bezier(.2,.7,.2,1)')}>
      <BackBtn label={backLabel} onClick={win.back} />
      <H1>Диагностика</H1>

      <Cap>Серверы</Cap>
      <div className="sf sq24" style={css('margin-top: 8px; padding: 16px')}>
        <div style={css('font-size: 13.5px; line-height: 19px; color: var(--text2)')}>
          {act ? 'Проверим из вашей сети каждый сервер «' + act.name + '»: какие отвечают и с какой задержкой, а какие нет.' : 'Сначала добавьте подписку — проверять пока нечего.'}
        </div>
        {run || done ? (
          <div style={css('display: flex; flex-direction: column; gap: 2px; margin-top: 12px')}>
            {shown.map((x) => {
              const ok = x.delay != null;
              return (
                <div key={x.name} style={css('display: flex; align-items: center; gap: 12px; padding: 8px 0')}>
                  <span aria-hidden="true" style={{ ...css('flex: none; width: 28px; height: 20px; border-radius: 5px; box-shadow: inset 0 0 0 1px rgba(0,0,0,.06)'), background: flagOf(x.name) }} />
                  <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
                    <span style={css('font-size: 14px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{cleanName(x.name)}</span>
                    <span style={css('font-size: 12px; color: var(--text2)')}>{protocolName(x.kind)}</span>
                  </span>
                  {run ? <span className="skel" style={css('width: 64px')} /> : null}
                  {done ? (
                    <span className="tagc" style={{ ...css('animation: valPop .4s cubic-bezier(.2,.8,.2,1) both'), background: ok ? 'color-mix(in srgb, var(--ok) 14%, transparent)' : 'color-mix(in srgb, var(--bad) 14%, transparent)', color: ok ? 'var(--ok)' : 'var(--bad)' }}>
                      {ok ? 'Работает · ' + x.delay + ' мс' : 'Не отвечает'}
                    </span>
                  ) : null}
                </div>
              );
            })}
          </div>
        ) : null}
        {done ? (
          <div style={css('margin-top: 8px; padding: 10px 12px; border-radius: 12px; background: var(--bg); font-size: 13px; line-height: 18px; animation: fadeUp .4s ease both')}>
            <span style={css('font-weight: 600')}>{okN + ' из ' + shown.length + ' работают.'}</span>{' '}
            <span style={css('color: var(--text2)')}>{okN < shown.length ? 'Выберите «Авто» или один из работающих серверов на главной.' : 'Всё в порядке.'}</span>
          </div>
        ) : null}
        <button
          onClick={() => void runDiag()}
          aria-disabled={run || !act ? 'true' : 'false'}
          className="tp"
          style={{ ...css('width: 100%; height: 44px; margin-top: 14px; border: 0; border-radius: 999px; font-size: 14.5px; font-weight: 600; cursor: pointer; font-family: inherit'), background: run || !act ? 'var(--fill2)' : 'var(--accent)', color: run || !act ? 'var(--text2)' : 'var(--onAccent)' }}
        >
          {run ? 'Проверяем…' : done ? 'Проверить ещё раз' : 'Проверить серверы'}
        </button>
        <div className="note" style={css('margin-top: 8px; text-align: center')}>
          Обычно до 10 секунд
        </div>
      </div>

      <div style={css('display: flex; align-items: flex-end; justify-content: space-between; margin-top: 24px; padding: 0 4px')}>
        <span className="cap2">Не открылось недавно</span>
        <span style={css('font-size: 13px; color: var(--text2); font-variant-numeric: tabular-nums')}>{recent} за час</span>
      </div>
      <div className="sf sq20" style={css('margin-top: 8px; padding: 0 16px')}>
        {failRows.map((f, i) => {
          const host = unicodeDomain(f.host);
          const target = siteTarget(f.host, catalog);
          const rule = siteRule(list, f.host, catalog);
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
            {store.isRunning ? 'Пока всё открывается' : 'Появится, когда VPN включён'}
          </div>
        ) : null}
      </div>
    </div>
  );
}
