// Живая схема: компьютер → через VPN или напрямую → интернет, на линиях — сервисы и программы из реальных соединений.

import { flowLabel } from '../lib/rules';
import type { ConnView, Mode, Route, Service } from '../lib/types';
import { Icon } from './Icon';
import { plural, t } from '../lib/lang';

/** Кто идёт по линии: самые активные сверху, остальные — «+N». */
function lane(conns: ConnView[], route: Route, catalog: Service[]): { names: string[]; more: number } {
  const bytes = new Map<string, number>();
  for (const c of conns) {
    if (c.route !== route) continue;
    const name = flowLabel(c, catalog);
    bytes.set(name, (bytes.get(name) ?? 0) + c.download + c.upload);
  }
  const sorted = [...bytes.entries()].sort((a, b) => b[1] - a[1]).map(([n]) => n);
  return { names: sorted.slice(0, 3), more: Math.max(0, sorted.length - 3) };
}

export function LiveSchema({ running, mode, server, conns, catalog }: { running: boolean; mode: Mode; server: string | null; conns: ConnView[]; catalog: Service[] }) {
  const vpn = lane(conns, 'vpn', catalog);
  const direct = lane(conns, 'direct', catalog);
  const block = lane(conns, 'block', catalog);
  const programs = new Set(conns.map((c) => c.process).filter(Boolean)).size;
  const vpnOn = running && vpn.names.length > 0;
  const directOn = !running || direct.names.length > 0;
  const text = (l: { names: string[]; more: number }) => (l.names.length ? l.names.join(', ') + (l.more ? ` +${l.more}` : '') : t('пока никого'));

  return (
    <div className="schema">
      <div className="schema-area">
        <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <path className={vpnOn ? 'vpn flow' : 'idle'} d="M14 50 C32 50,30 24,50 24 C70 24,68 50,86 50" vectorEffect="non-scaling-stroke" />
          <path className={directOn ? 'direct flow' : 'idle'} d="M14 50 C32 50,30 76,50 76 C70 76,68 50,86 50" vectorEffect="non-scaling-stroke" />
        </svg>
        <div className="switch-node" style={{ left: '14%' }}>
          <div className="switch-node-ico">
            <span className="monitor">
              <i />
              <b />
            </span>
          </div>
          <b>{t('Компьютер')}</b>
          <span>{running ? plural(programs, ['программа', 'программы', 'программ']) : t('приложения')}</span>
        </div>
        <div className={running ? 'switch-chip live' : 'switch-chip off'} style={{ top: '24%' }}>
          <i />
          <span>{running ? server ?? 'VPN' : t('VPN выключен')}</span>
        </div>
        <div className="switch-chip plain" style={{ top: '76%' }}>
          <span>{t('Напрямую')}</span>
        </div>
        <div className="switch-node" style={{ left: '86%' }}>
          <div className="switch-node-ico">
            <Icon name="globe" size={20} />
          </div>
          <b>{t('Интернет')}</b>
          <span>{t('сайты')}</span>
        </div>
      </div>
      <div className="lanes">
        {running ? (
          <>
            <div>
              <i style={{ background: 'var(--accent)' }} />
              <span>
                <b>{t('через VPN')}</b> — {text(vpn)}
              </span>
            </div>
            <div>
              <i style={{ background: 'var(--dim)' }} />
              <span>
                <b>{t('напрямую')}</b> — {text(direct)}
              </span>
            </div>
            {block.names.length ? (
              <div>
                <i style={{ background: 'var(--red)' }} />
                <span>
                  <b>{t('блок')}</b> — {text(block)}
                </span>
              </div>
            ) : null}
            {mode === 'sys_proxy' ? <div className="lanes-note">{t('В режиме системного прокси видны только программы, которые идут через прокси.')}</div> : null}
          </>
        ) : (
          <div className="lanes-note">{t('VPN выключен: всё идёт напрямую. Схема оживёт после подключения.')}</div>
        )}
      </div>
    </div>
  );
}
