// Лист «Как работает Kill Switch» из макета. Подписи — по тому, как устроено у нас:
// защищённые программы при включённом VPN идут только через VPN, остальные — как решает тумблер.

import { useState } from 'react';
import { routingTitle } from '../lib/i18n';
import type { Routing } from '../lib/types';
import { Sheet } from './Chrome';
import { Seg } from './Controls';
import { Icon } from './Icon';
import { t } from '../lib/lang';

type View = 'on' | 'off';

export function KsInfo({ vpnOn, routing, protectedCount, onClose }: { vpnOn: boolean; routing: Routing; protectedCount: number; onClose: () => void }) {
  const [view, setView] = useState<View>(vpnOn ? 'on' : 'off');
  const on = view === 'on';
  const othersVpn = routing === 'all_vpn';
  return (
    <Sheet onClose={onClose}>
      <h3>{t('Как работает Kill Switch')}</h3>
      <Seg
        className="seg-sm mt14"
        value={view}
        options={[
          ['on', t('VPN включён')],
          ['off', t('VPN выключен')],
        ]}
        onChange={setView}
      />
      <div className="mode-viz">
        <div className="mode-area ks-area">
          <svg key={view + routing} viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
            {on ? (
              <>
                <path className="on" d="M16 26 L50 26" vectorEffect="non-scaling-stroke" />
                <path className="on" d="M50 26 C68 26,68 50,84 50" vectorEffect="non-scaling-stroke" />
                {othersVpn ? (
                  <>
                    <path className="on" d="M16 74 C32 74,32 26,50 26" vectorEffect="non-scaling-stroke" />
                    <path className="idle" d="M16 74 L50 74" vectorEffect="non-scaling-stroke" />
                    <path className="idle" d="M50 74 C68 74,68 50,84 50" vectorEffect="non-scaling-stroke" />
                  </>
                ) : (
                  <>
                    <path className="idle" d="M16 74 C32 74,32 26,50 26" vectorEffect="non-scaling-stroke" />
                    <path className="past" d="M16 74 L50 74" vectorEffect="non-scaling-stroke" />
                    <path className="past" d="M50 74 C68 74,68 50,84 50" vectorEffect="non-scaling-stroke" />
                  </>
                )}
              </>
            ) : (
              <>
                <path className="idle" d="M50 26 C68 26,68 50,84 50" vectorEffect="non-scaling-stroke" />
                <path className="past" d="M16 74 L50 74" vectorEffect="non-scaling-stroke" />
                <path className="past" d="M50 74 C68 74,68 50,84 50" vectorEffect="non-scaling-stroke" />
                <path className="cut" d="M16 26 L30 26" vectorEffect="non-scaling-stroke" />
              </>
            )}
          </svg>
          {on ? null : (
            <span className="ks-cut">
              <Icon name="winClose" size={10} />
            </span>
          )}
          <div className="mode-node" style={{ left: '16%', top: '26%' }}>
            <div className={on ? 'mode-node-ico' : 'mode-node-ico cutoff'}>
              <span className="monitor sm">
                <i />
                <b />
              </span>
            </div>
            <b>{t('Защищённые')}</b>
            <span>{t('{n} в списке', { n: protectedCount })}</span>
          </div>
          <div className="mode-node" style={{ left: '16%', top: '74%' }}>
            <div className="mode-node-ico">
              <span className="monitor sm">
                <i />
                <b />
              </span>
            </div>
            <b>{t('Остальные')}</b>
            <span>{on ? t('по тумблеру') : t('приложения')}</span>
          </div>
          <div className={on ? 'mode-pill' : 'mode-pill off'} style={{ top: '26%' }}>
            <i />
            VPN
          </div>
          <div className="mode-isp" style={{ top: '74%', color: on && othersVpn ? 'var(--dim2)' : 'var(--text)' }}>
            {t('Провайдер')}
          </div>
          <div className="mode-node" style={{ left: '84%', top: '50%' }}>
            <div className="mode-node-ico">
              <Icon name="globe" size={18} />
            </div>
            <b>{t('Интернет')}</b>
            <span>{t('сайты')}</span>
          </div>
        </div>
      </div>
      <div className="mode-points">
        <div>
          <i style={{ background: on ? 'var(--accent)' : 'var(--red)' }} />
          {on
            ? t('Защищённые программы ходят только через VPN — даже если тумблер пустил бы их напрямую. Остальные — как решает тумблер: сейчас «{routing}».', { routing: routingTitle[routing] })
            : t('Защищённые программы остаются без интернета — их данные не уйдут через провайдера. Остальные работают напрямую.')}
        </div>
        {on ? null : (
          <div>
            <i style={{ background: 'var(--dim)' }} />
            {t('Локальная сеть для них открыта: роутер, принтер, игры по локалке.')}
          </div>
        )}
      </div>
      <div className="sheet-actions" style={{ marginTop: 16 }}>
        <button onClick={onClose}>{t('Понятно')}</button>
      </div>
    </Sheet>
  );
}
