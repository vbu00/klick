// Общие элементы из макета: переключатель, сегменты, кнопка «назад», заголовок раздела, плитка с буквой.

import type { ReactNode } from 'react';
import { tint } from '../lib/rules';
import { Icon, type IconName } from './Icon';

export function Toggle({ on, onChange, label, disabled, big }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean; big?: boolean }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} disabled={disabled} className={`toggle${on ? ' on' : ''}${big ? ' big' : ''}`} onClick={() => onChange(!on)}>
      <i />
    </button>
  );
}

export function Seg<T extends string>({ value, options, onChange, className }: { value: T; options: [T, string][]; onChange: (v: T) => void; className?: string }) {
  return (
    <div className={className ? `seg ${className}` : 'seg'} role="tablist">
      {options.map(([v, label]) => (
        <button key={v} role="tab" aria-selected={v === value} className={v === value ? 'on' : ''} onClick={() => onChange(v)}>
          {label}
        </button>
      ))}
    </div>
  );
}

/** Пилюля «‹ Соединение» в начале подэкрана. */
export function Back({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button className="back" onClick={onClick}>
      <Icon name="chevron" className="flip" />
      {label}
    </button>
  );
}

export function SectionHead({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="sec-head">
      <span className="caps">{title}</span>
      {children}
    </div>
  );
}

/** Плитка правила: буква с цветом для сервисов и программ, значок для сайтов и IP. */
export function Tile({ label, icon, size = 30 }: { label: string; icon?: IconName; size?: number }) {
  if (icon) {
    return (
      <span className="tile-ico" style={{ width: size, height: size }}>
        <Icon name={icon} size={Math.round(size * 0.53)} />
      </span>
    );
  }
  return (
    <span className="tile-letter" style={{ width: size, height: size, background: tint(label) }}>
      {label.trim().charAt(0).toUpperCase()}
    </span>
  );
}

/** Строка списка внутри карточки: слева текст, справа — что угодно; со стрелкой, если ведёт дальше. */
export function Row({
  title,
  sub,
  left,
  right,
  onClick,
  chevron,
  wrap,
}: {
  title: ReactNode;
  sub?: ReactNode;
  left?: ReactNode;
  right?: ReactNode;
  onClick?: () => void;
  chevron?: boolean;
  /** Подпись в несколько строк: для пояснений, а не путей. */
  wrap?: boolean;
}) {
  const body = (
    <>
      {left}
      <div className={wrap ? 'row-text wrap' : 'row-text'}>
        <div className="row-title">{title}</div>
        {sub ? <div className="row-sub">{sub}</div> : null}
      </div>
      {right}
      {chevron ? <Icon name="chevron" className="row-chev" /> : null}
    </>
  );
  return onClick ? (
    <button className="row link" onClick={onClick}>
      {body}
    </button>
  ) : (
    <div className="row">{body}</div>
  );
}
