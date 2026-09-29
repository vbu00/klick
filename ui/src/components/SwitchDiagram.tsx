// Схема из макета для листа «Сменить подключение?»: сейчас трафик идёт через одно подключение, после — через другое.

import { Icon } from './Icon';
import { t } from '../lib/lang';

export function SwitchDiagram({ liveName, targetName }: { liveName: string; targetName: string }) {
  return (
    <div className="switch-viz">
      <div className="switch-area">
        <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <path className="now" d="M14 50 C32 50,30 22,50 22 C70 22,68 50,86 50" vectorEffect="non-scaling-stroke" />
          <path className="next" d="M14 50 C32 50,30 78,50 78 C70 78,68 50,86 50" vectorEffect="non-scaling-stroke" />
        </svg>
        <div className="switch-node" style={{ left: '14%' }}>
          <div className="switch-node-ico">
            <span className="monitor">
              <i />
              <b />
            </span>
          </div>
          <b>{t('Компьютер')}</b>
          <span>{t('приложения')}</span>
        </div>
        <div className="switch-chip live" style={{ top: '22%' }}>
          <i />
          <span>{liveName}</span>
        </div>
        <div className="switch-chip target" style={{ top: '78%' }}>
          <i />
          <span>{targetName}</span>
        </div>
        <div className="switch-node" style={{ left: '86%' }}>
          <div className="switch-node-ico">
            <Icon name="globe" size={20} />
          </div>
          <b>{t('Интернет')}</b>
          <span>{t('сайты')}</span>
        </div>
      </div>
      <div className="switch-legend">
        <span>
          <i className="solid" />
          {t('трафик сейчас')}
        </span>
        <span>
          <i className="dashed" />
          {t('после переключения')}
        </span>
      </div>
    </div>
  );
}
