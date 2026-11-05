// «Маршруты» по макету: что пускать через VPN, исключения или сервисы, «Сейчас» и «Что-то не открывается?».

import { useStore } from '../lib/store';
import type { Routing, Target } from '../lib/types';
import { css } from './css';
import { TAG, appsText, connsText, metaOf, tagOf, useLive } from './live';
import { plural } from './model';
import { Cap, Chevron, H1, Letter, Switch, tintOf as tintFor } from './ui';
import { useWin } from './win';

/** Сервисы из макета: id в каталоге, название, цвет плитки. */
const SVCS: [string, string, string][] = [
  ['youtube', 'YouTube', '#ff3b30'],
  ['telegram', 'Telegram', '#2aabee'],
  ['discord', 'Discord', '#5865f2'],
  ['instagram', 'Instagram', '#e1306c'],
  ['chatgpt', 'ChatGPT', '#10a37f'],
];

export function Routes() {
  const store = useStore();
  const win = useWin();
  const { settings, state } = store;
  const { apps } = useLive(true);
  if (!settings || !state) return null;
  const routing = settings.routing;
  const all = routing === 'all_vpn';
  const list = settings.lists[routing];
  const applied = () => {
    if (store.isRunning) store.toast('Маршруты применены', undefined, 'ok');
  };

  const sites = list.filter((r) => r.target.kind === 'domain' || r.target.kind === 'ip').length;
  const progs = list.filter((r) => r.target.kind === 'program').length;
  const sum = (sites ? sites + ' ' + plural(sites, 'сайт', 'сайта', 'сайтов') : '') + (sites && progs ? ' и ' : '') + (progs ? progs + ' ' + plural(progs, 'приложение', 'приложения', 'приложений') : '');
  const ru = settings.russia_direct.domains || settings.russia_direct.ips;

  const svcOn = (id: string) => list.some((r) => r.target.kind === 'service' && r.target.value === id && r.enabled !== false && r.route === 'vpn');
  const setSvc = async (id: string, name: string, on: boolean) => {
    const t: Target = { kind: 'service', value: id };
    if (on) {
      if (await store.routeTarget(t, true, name, true)) applied();
      return;
    }
    const i = list.findIndex((r) => r.target.kind === 'service' && r.target.value === id);
    if (i >= 0) {
      await store.setRuleEnabled(routing, i, false);
      applied();
    }
  };

  const modes: [Routing, string, string][] = [
    ['all_vpn', 'Всё, кроме исключений', 'Российские сайты и локальная сеть — напрямую'],
    ['selected', 'Только выбранное', 'YouTube, Telegram и то, что добавите сами'],
  ];

  const total = apps.reduce((a, x) => a + x.conns, 0);
  const vpn = apps.reduce((a, x) => a + x.vpnConns, 0);
  const top = apps.slice().sort((a, b) => b.bytes - a.bytes).slice(0, 3);
  const liveOn = state.vpn === 'connected';

  return (
    <div style={css('display: flex; flex-direction: column; animation: tabin .28s cubic-bezier(.2,.7,.2,1)')}>
      <H1>Маршруты</H1>

      <Cap>Что пускать через VPN</Cap>
      <div role="radiogroup" aria-label="Что пускать через VPN" style={css('display: flex; flex-direction: column; gap: 8px; margin-top: 8px')}>
        {modes.map(([k, t, d]) => {
          const a = routing === k;
          return (
            <button
              key={k}
              role="radio"
              aria-checked={a ? 'true' : 'false'}
              onClick={() => {
                if (!a) void store.setRouting(k).then(applied);
              }}
              className="rcard sq20"
              style={{ background: 'var(--surface)', boxShadow: a ? 'inset 0 0 0 1px var(--frame), 0 2px 10px rgba(0,0,0,.04)' : 'inset 0 0 0 .4px var(--line)' }}
            >
              <span aria-hidden="true" className="rdo" style={{ boxShadow: a ? 'inset 0 0 0 2px var(--accent)' : 'inset 0 0 0 1.6px var(--text3)' }}>
                <i style={{ transform: `scale(${a ? 1 : 0})` }} />
              </span>
              <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px')}>
                <span style={css('font-size: 15px; line-height: 20px; font-weight: 600')}>{t}</span>
                <span style={css('font-size: 13px; line-height: 17px; color: var(--text2)')}>{d}</span>
              </span>
            </button>
          );
        })}
      </div>

      {all ? (
        <div style={css('animation: expandin .25s ease')}>
          <Cap>Напрямую, без VPN</Cap>
          <div className="sf sq20" style={css('margin-top: 8px; padding: 0 16px')}>
            <div className="rrow">
              <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
                <span style={css('font-size: 14px; line-height: 18px; font-weight: 500')}>Российские сайты и сервисы</span>
                <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2)')}>Госуслуги, банки, маркетплейсы</span>
              </span>
              <Switch label="Российские сайты и сервисы напрямую" on={ru} onChange={(v) => void store.setPrefs({ ru_domains: v, ru_ips: v }).then(applied)} />
            </div>
            <div style={css('height: .4px; background: var(--line)')} />
            <RulesRow title="Мои исключения" sum={sum} onClick={() => win.openSheet('rules')} />
          </div>
          <div className="note" style={css('margin-top: 8px; padding: 0 4px')}>
            Локальная сеть — роутер, принтер, NAS — всегда идёт напрямую.
          </div>
        </div>
      ) : (
        <div style={css('animation: expandin .25s ease')}>
          <Cap>Через VPN</Cap>
          <div className="sf sq20" style={css('margin-top: 8px; padding: 4px 16px')}>
            <div className="rrow" style={{ minHeight: 52 }}>
              <Letter name="З" tint="#8e8e93" size={28} radius={9} font={12} />
              <span style={css('flex: 1; font-size: 14px; font-weight: 500')}>Заблокированное в РФ</span>
              <Switch label="Заблокированное в РФ" on={settings.blocked_preset !== false} onChange={(v) => void store.setPrefs({ blocked_preset: v }).then(applied)} />
            </div>
            {SVCS.map(([id, name, tint]) => (
              <div key={id}>
                <div style={css('height: .4px; margin-left: 40px; background: var(--line)')} />
                <div className="rrow" style={{ minHeight: 52 }}>
                  <Letter name={name} tint={tint} size={28} radius={9} font={12} />
                  <span style={css('flex: 1; font-size: 14px; font-weight: 500')}>{name}</span>
                  <Switch label={name} on={svcOn(id)} onChange={(v) => void setSvc(id, name, v)} />
                </div>
              </div>
            ))}
            <div style={css('height: .4px; background: var(--line)')} />
            <RulesRow title="Мои сайты и приложения" sum={sum} onClick={() => win.openSheet('rules')} />
          </div>
          <div className="note" style={css('margin-top: 8px; padding: 0 4px')}>
            Адреса сервисов обновляются сами. Всё остальное идёт напрямую.
          </div>
        </div>
      )}

      <div style={css('display: flex; align-items: flex-end; justify-content: space-between; margin-top: 24px; padding: 0 4px')}>
        <span className="cap2">Сейчас</span>
        <button onClick={() => win.openSheet('conns')} style={css('border: 0; padding: 0; background: transparent; color: var(--accent); font-size: 13px; font-weight: 500; cursor: pointer; font-family: inherit')}>
          Все программы
        </button>
      </div>
      <div className="sf sq24" style={css('margin-top: 8px; padding: 16px')}>
        {liveOn ? (
          <>
            <div style={css('display: flex; align-items: baseline; gap: 6px')}>
              <span style={css('font-size: 15px; font-weight: 600; font-variant-numeric: tabular-nums')}>{connsText(total)}</span>
              <span style={css('font-size: 13px; color: var(--text2)')}>{appsText(apps.length)}</span>
            </div>
            <div aria-hidden="true" style={css('display: flex; gap: 3px; height: 8px; margin-top: 12px')}>
              <span style={{ ...css('border-radius: 999px; background: var(--accent); transition: width .5s cubic-bezier(.2,.7,.2,1)'), width: (total ? (vpn / total) * 100 : 0).toFixed(1) + '%' }} />
              <span style={css('flex: 1; border-radius: 999px; background: var(--fill)')} />
            </div>
            <div style={css('display: flex; gap: 16px; margin-top: 10px; font-size: 12.5px; color: var(--text2); font-variant-numeric: tabular-nums')}>
              <span style={css('display: flex; align-items: center; gap: 6px')}>
                <span aria-hidden="true" style={css('width: 7px; height: 7px; border-radius: 50%; background: var(--accent)')} />
                Через VPN <span style={css('color: var(--text); font-weight: 500')}>{vpn}</span>
              </span>
              <span style={css('display: flex; align-items: center; gap: 6px')}>
                <span aria-hidden="true" style={css('width: 7px; height: 7px; border-radius: 50%; background: var(--text3)')} />
                Напрямую <span style={css('color: var(--text); font-weight: 500')}>{total - vpn}</span>
              </span>
            </div>
            <div style={css('display: flex; flex-direction: column; gap: 2px; margin-top: 10px')}>
              {top.map((a) => {
                const tg = TAG[tagOf(a)];
                return (
                  <div key={a.key} style={css('display: flex; align-items: center; gap: 12px; padding: 8px 0')}>
                    <Letter name={a.name} tint={tintFor(a.name)} />
                    <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
                      <span style={css('font-size: 14px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{a.name}</span>
                      <span style={css('font-size: 12.5px; color: var(--text2); font-variant-numeric: tabular-nums')}>{metaOf(a)}</span>
                    </span>
                    <span className="tagc" style={{ background: tg[1], color: tg[2] }}>
                      {tg[0]}
                    </span>
                  </div>
                );
              })}
            </div>
          </>
        ) : (
          <div style={css('display: flex; align-items: center; gap: 12px')}>
            <span aria-hidden="true" style={css('flex: none; width: 36px; height: 36px; border-radius: 50%; background: var(--fill2); display: flex; align-items: center; justify-content: center')}>
              <span style={css('width: 8px; height: 8px; border-radius: 50%; background: var(--text3)')} />
            </span>
            <span style={css('display: flex; flex-direction: column; gap: 2px')}>
              <span style={css('font-size: 14px; font-weight: 600')}>VPN выключен</span>
              <span style={css('font-size: 12.5px; color: var(--text2)')}>Сейчас весь трафик идёт напрямую</span>
            </span>
          </div>
        )}
      </div>
      <button onClick={() => win.open('diag')} className="sf sq20 tp" style={css('display: flex; align-items: center; gap: 12px; margin-top: 12px; padding: 14px 16px; border: 0; color: var(--text); text-align: left; cursor: pointer; font-family: inherit')}>
        <span style={css('flex: 1; display: flex; flex-direction: column; gap: 2px')}>
          <span style={css('font-size: 14px; font-weight: 500')}>Что-то не открывается?</span>
          <span style={css('font-size: 12.5px; color: var(--text2)')}>Диагностика: серверы, блокировки, утечки</span>
        </span>
        <Chevron />
      </button>
    </div>
  );
}


function RulesRow({ title, sum, onClick }: { title: string; sum: string; onClick: () => void }) {
  return (
    <button onClick={onClick} className="rrow" style={css('cursor: pointer')}>
      <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
        <span style={css('font-size: 14px; line-height: 18px; font-weight: 500')}>{title}</span>
        <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2)')}>{sum || 'Пока пусто — добавьте сайт или приложение'}</span>
      </span>
      <Chevron />
    </button>
  );
}
