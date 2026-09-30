// Лист «Как работает режим» из макета: схема, два пункта и «Выбрать». Режимов у нас два — без «Proxy».

import { useState } from 'react';
import { OS_NAME } from '../lib/platform';
import type { Mode } from '../lib/types';
import { Sheet } from './Chrome';
import { Seg } from './Controls';
import { Icon } from './Icon';

interface ModeDiagram {
  top: [string, string];
  bottom: [string, string];
  pill: string;
  /** Часть программ идёт мимо: так в системном прокси. */
  split: boolean;
  points: [string, string][];
}

const MODES: Record<Mode, ModeDiagram> = {
  tun: {
    top: ['Все программы', 'включая игры'],
    bottom: ['Службы', 'и UDP-трафик'],
    pill: 'TUN-адаптер',
    split: false,
    points: [
      ['var(--accent)', 'Виртуальный сетевой адаптер перехватывает трафик всех программ — ничего не нужно настраивать.'],
      ['var(--accent)', 'Что пойдёт через VPN, а что напрямую, решает тумблер «Куда направлять».'],
    ],
  },
  sys_proxy: {
    top: ['Большинство', 'браузеры, Telegram'],
    bottom: ['Игры', 'и часть программ'],
    pill: 'Системный прокси',
    split: true,
    points: [
      ['var(--accent)', `${OS_NAME} сама передаёт адрес прокси программам — большинство подхватывает его без настройки.`],
      ['var(--dim)', `Игры, UDP-трафик и программы, которые не слушаются настроек ${OS_NAME}, идут напрямую.`],
    ],
  },
};

export function ModeInfo({ initial, current, onPick, onClose }: { initial: Mode; current: Mode; onPick: (m: Mode) => void; onClose: () => void }) {
  const [view, setView] = useState<Mode>(initial);
  const d = MODES[view];
  return (
    <Sheet onClose={onClose}>
      <h3>Как работает режим</h3>
      <Seg
        className="seg-sm mt14"
        value={view}
        options={[
          ['tun', 'VPN (TUN)'],
          ['sys_proxy', 'Системный прокси'],
        ]}
        onChange={setView}
      />
      <div className="mode-viz">
        <div className="mode-area">
          <svg key={view} viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
            {d.split ? (
              <>
                <path className="idle" d="M14 74 C32 74,32 26,50 26" vectorEffect="non-scaling-stroke" />
                <path className="on" d="M14 26 L50 26" vectorEffect="non-scaling-stroke" />
                <path className="on" d="M50 26 C68 26,68 50,86 50" vectorEffect="non-scaling-stroke" />
                <path className="past" d="M14 74 L50 74" vectorEffect="non-scaling-stroke" />
                <path className="past" d="M50 74 C68 74,68 50,86 50" vectorEffect="non-scaling-stroke" />
              </>
            ) : (
              <>
                <path className="idle" d="M14 74 L50 74" vectorEffect="non-scaling-stroke" />
                <path className="idle" d="M50 74 C68 74,68 50,86 50" vectorEffect="non-scaling-stroke" />
                <path className="on" d="M14 26 L50 26" vectorEffect="non-scaling-stroke" />
                <path className="on" d="M14 74 C32 74,32 26,50 26" vectorEffect="non-scaling-stroke" />
                <path className="on" d="M50 26 C68 26,68 50,86 50" vectorEffect="non-scaling-stroke" />
              </>
            )}
          </svg>
          <div className="mode-node" style={{ left: '14%', top: '26%' }}>
            <div className="mode-node-ico hot">
              <span className="monitor sm">
                <i />
                <b />
              </span>
            </div>
            <b>{d.top[0]}</b>
            <span>{d.top[1]}</span>
          </div>
          <div className="mode-node" style={{ left: '14%', top: '74%' }}>
            <div className="mode-node-ico" style={{ color: d.split ? 'var(--text2)' : 'var(--accent)' }}>
              <span className="monitor sm">
                <i />
                <b />
              </span>
            </div>
            <b>{d.bottom[0]}</b>
            <span>{d.bottom[1]}</span>
          </div>
          <div className="mode-pill" style={{ top: '26%' }}>
            <i />
            {d.pill}
          </div>
          <div className="mode-isp" style={{ top: '74%', color: d.split ? 'var(--text)' : 'var(--dim2)' }}>
            Провайдер
          </div>
          <div className="mode-node" style={{ left: '86%', top: '50%' }}>
            <div className="mode-node-ico">
              <Icon name="globe" size={18} />
            </div>
            <b>Интернет</b>
            <span>сайты</span>
          </div>
        </div>
      </div>
      <div className="mode-points">
        {d.points.map(([dot, text]) => (
          <div key={text}>
            <i style={{ background: dot }} />
            {text}
          </div>
        ))}
      </div>
      <div className="sheet-actions" style={{ marginTop: 16 }}>
        <button onClick={onClose}>Понятно</button>
        {view !== current ? (
          <button className="accent" onClick={() => onPick(view)}>
            Выбрать
          </button>
        ) : null}
      </div>
    </Sheet>
  );
}
