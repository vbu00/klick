// Общие детали из макета: лист снизу, переключатель, радио, стрелка, «назад», диалоги.

import { useEffect, useRef, useState, type ReactNode } from 'react';
import { css } from './css';

/** Лист держится в DOM ещё 420 мс после закрытия — пока уезжает вниз. */
export function useMounted(open: boolean): boolean {
  const [mounted, setMounted] = useState(open);
  useEffect(() => {
    if (open) {
      setMounted(true);
      return;
    }
    const t = setTimeout(() => setMounted(false), 420);
    return () => clearTimeout(t);
  }, [open]);
  return open || mounted;
}

export function CloseX({ onClick }: { onClick: () => void }) {
  return (
    <button onClick={onClick} aria-label="Закрыть" className="tp" style={css('width: 30px; height: 30px; border: 0; border-radius: 50%; background: var(--fill2); color: var(--text2); display: flex; align-items: center; justify-content: center; cursor: pointer')}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
        <path d="M2 2l8 8M10 2 2 10" />
      </svg>
    </button>
  );
}

const FADE = css('position: absolute; left: 0; right: 0; top: 0; height: 20px; background: linear-gradient(180deg, var(--bg) 0%, color-mix(in srgb, var(--bg) 70%, transparent) 45%, color-mix(in srgb, var(--bg) 0%, transparent) 100%)');

export function Sheet(props: {
  open: boolean;
  onClose: () => void;
  label: string;
  height: number;
  z: number;
  head: ReactNode;
  children: ReactNode;
  fade?: boolean;
  bodyStyle?: string;
}) {
  const { open, onClose } = props;
  const mounted = useMounted(open);
  return (
    <>
      <button
        aria-label="Закрыть"
        tabIndex={open ? 0 : -1}
        onClick={onClose}
        style={{ ...css('position: absolute; inset: 0; border: 0; padding: 0; background: rgba(0,0,0,.28); transition: opacity .35s ease; cursor: default'), zIndex: props.z, opacity: open ? 1 : 0, pointerEvents: open ? 'auto' : 'none' }}
      />
      <div
        role="dialog"
        aria-label={props.label}
        aria-hidden={open ? 'false' : 'true'}
        className="sq24"
        style={{
          ...css('position: absolute; left: 0; right: 0; bottom: 0; display: flex; flex-direction: column; border-bottom-left-radius: 0; border-bottom-right-radius: 0; background: var(--bg); box-shadow: 0 -10px 40px rgba(0,0,0,.12), inset 0 .4px 0 var(--line); overflow: hidden'),
          height: props.height,
          zIndex: props.z + 1,
          transform: open ? 'translateY(0)' : 'translateY(105%)',
          transition: open ? 'transform .55s cubic-bezier(.32,.72,0,1)' : 'transform .38s cubic-bezier(.32,.72,0,1)',
        }}
      >
        <div style={css('flex: none; display: flex; justify-content: center; padding-top: 8px')}>
          <span aria-hidden="true" style={css('width: 36px; height: 5px; border-radius: 3px; background: var(--fill)')} />
        </div>
        <div style={css('flex: none; display: flex; align-items: center; gap: 12px; padding: 12px 16px 10px')}>
          {props.head}
          <CloseX onClick={onClose} />
        </div>
        {props.fade !== false ? (
          <div aria-hidden="true" style={css('position: relative; flex: none; height: 0; z-index: 2; pointer-events: none')}>
            <div style={FADE} />
          </div>
        ) : null}
        {mounted ? (
          <div className="ksc" style={css(props.bodyStyle ?? 'flex: 1; min-height: 0; overflow-y: auto; padding: 20px 16px 24px; display: flex; flex-direction: column; gap: 12px')}>
            {props.children}
          </div>
        ) : null}
      </div>
    </>
  );
}

export function SheetTitle({ title, sub, subStyle }: { title: string; sub?: ReactNode; subStyle?: string }) {
  if (sub === undefined) return <div style={css('flex: 1; min-width: 0; font-size: 20px; line-height: 26px; font-weight: 600')}>{title}</div>;
  return (
    <div style={css('flex: 1; min-width: 0')}>
      <div style={css('font-size: 20px; line-height: 26px; font-weight: 600')}>{title}</div>
      <div style={css(subStyle ?? 'font-size: 13px; color: var(--text2)')}>{sub}</div>
    </div>
  );
}

export function Switch({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      role="switch"
      aria-checked={on ? 'true' : 'false'}
      aria-label={label}
      aria-disabled={disabled ? 'true' : undefined}
      onClick={() => !disabled && onChange(!on)}
      className="swt"
      style={{ background: on ? 'var(--accent)' : 'var(--fill)', opacity: disabled ? 0.5 : undefined, cursor: disabled ? 'default' : undefined }}
    >
      <i style={{ transform: `translateX(${on ? '16px' : '0px'})` }} />
    </button>
  );
}

/** Радио: кольцо и точка акцента. */
export function Radio({ on, thin }: { on: boolean; thin?: boolean }) {
  return (
    <span aria-hidden="true" className="rdo" style={{ boxShadow: on ? 'inset 0 0 0 2px var(--accent)' : thin ? 'inset 0 0 0 1.6px var(--text3)' : 'inset 0 0 0 1.6px var(--text3)' }}>
      <i style={{ transform: `scale(${on ? 1 : 0})` }} />
    </span>
  );
}

export function Chevron() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="var(--text3)" aria-hidden="true">
      <path d="M17.3 12.8 9.8 20.3a1.13 1.13 0 1 1-1.6-1.6l6.71-6.7L8.2 5.3a1.13 1.13 0 1 1 1.6-1.6l7.5 7.5a1.13 1.13 0 0 1 0 1.6z" />
    </svg>
  );
}

export function BackBtn({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="tp"
      style={css('align-self: flex-start; display: flex; align-items: center; gap: 4px; height: 32px; margin-bottom: 12px; padding: 0 14px 0 8px; border: 0; border-radius: 999px; background: var(--surface); box-shadow: inset 0 0 0 .4px var(--line); color: var(--text); font-size: 13.5px; cursor: pointer; font-family: inherit')}
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M6.7 12.8l7.5 7.5a1.13 1.13 0 1 0 1.6-1.6L9.09 12l6.71-6.7a1.13 1.13 0 1 0-1.6-1.6L6.7 11.2a1.13 1.13 0 0 0 0 1.6z" />
      </svg>
      {label}
    </button>
  );
}

export const H1 = ({ children }: { children: ReactNode }) => <div style={css('font-size: 24px; line-height: 32px; font-weight: 600; letter-spacing: -.3px')}>{children}</div>;

export const Cap = ({ children, style }: { children: ReactNode; style?: string }) => (
  <div className="cap2" style={css(style ?? 'margin-top: 24px; padding: 0 4px')}>
    {children}
  </div>
);

export const Hair = ({ style }: { style?: string }) => <div style={css(style ?? 'height: .4px; background: var(--line)')} />;

/** Строка настроек с тумблером. */
export function ToggleRow({ title, sub, on, onChange, onOpen }: { title: string; sub: ReactNode; on: boolean; onChange: (v: boolean) => void; onOpen?: () => void }) {
  return (
    <div className="rrow" style={css('padding: 14px 0')}>
      <span
        onClick={onOpen}
        role={onOpen ? 'button' : undefined}
        tabIndex={onOpen ? 0 : undefined}
        onKeyDown={onOpen ? (e) => (e.key === 'Enter' || e.key === ' ') && onOpen() : undefined}
        style={{ ...css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px'), cursor: onOpen ? 'pointer' : undefined }}
      >
        <span style={css('font-size: 14px; line-height: 18px; font-weight: 500')}>{title}</span>
        <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2)')}>{sub}</span>
      </span>
      <Switch label={title} on={on} onChange={onChange} />
    </div>
  );
}

/** Строка настроек со стрелкой. */
export function NavRow({ title, sub, value, onClick }: { title: string; sub: ReactNode; value?: ReactNode; onClick: () => void }) {
  return (
    <button onClick={onClick} className="rrow" style={css('padding: 14px 0; cursor: pointer')}>
      <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
        <span style={css('font-size: 14px; line-height: 18px; font-weight: 500')}>{title}</span>
        <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2)')}>{sub}</span>
      </span>
      <span style={css('font-size: 13.5px; color: var(--text2)')}>{value ?? ''}</span>
      <Chevron />
    </button>
  );
}

/** Группа строк на карточке с волосяными разделителями. */
export function Group({ children, style }: { children: ReactNode[]; style?: string }) {
  const rows = children.filter(Boolean);
  return (
    <div className="sf sq20" style={css(style ?? 'margin-top: 8px; padding: 0 16px')}>
      {rows.map((r, i) => (
        <div key={i}>
          {i > 0 ? <Hair /> : null}
          {r}
        </div>
      ))}
    </div>
  );
}

/** Сегменты с ползунком, как «По ссылке / Из файла». */
export function Seg<T extends string>({ value, options, onChange, label, role = 'tablist', anim, weightOn }: { value: T; options: [T, string][]; onChange: (v: T) => void; label?: string; role?: 'tablist' | 'radiogroup'; anim?: string; weightOn?: number }) {
  const idx = Math.max(0, options.findIndex(([k]) => k === value));
  const n = options.length;
  return (
    <div className="sf sq12" role={role} aria-label={label} style={{ ...css('position: relative; flex: none; display: flex; height: 40px; padding: 4.4px'), animation: anim }}>
      <span
        aria-hidden="true"
        style={{ ...css('position: absolute; top: 4.4px; bottom: 4.4px; left: 4.4px; border-radius: 9px; background: var(--fill); transition: transform .38s cubic-bezier(.32,.72,0,1), box-shadow .25s ease'), width: `calc((100% - 8.8px) / ${n})`, transform: `translateX(${idx * 100}%)` }}
      />
      {options.map(([k, l]) => {
        const on = k === value;
        return (
          <button
            key={k}
            role={role === 'tablist' ? 'tab' : 'radio'}
            aria-selected={role === 'tablist' ? (on ? 'true' : 'false') : undefined}
            aria-checked={role === 'radiogroup' ? (on ? 'true' : 'false') : undefined}
            onClick={() => onChange(k)}
            style={{ ...css('position: relative; flex: 1; border: 0; border-radius: 9px; background: transparent; font-size: 13.5px; cursor: pointer; font-family: inherit; transition: color .2s ease'), color: on ? 'var(--text)' : 'var(--text2)', fontWeight: on && weightOn ? weightOn : 500 }}
          >
            {l}
          </button>
        );
      })}
    </div>
  );
}

/** Окно поверх: переименовать, удалить. */
export function Dialog(props: { open: boolean; title: string; text: string; okLabel: string; okBg: string; onOk: () => void; onClose: () => void; input?: { value: string; onChange: (v: string) => void; label: string } }) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (props.open && props.input) setTimeout(() => ref.current?.select(), 30);
  }, [props.open, props.input]);
  return (
    <>
      <button aria-label="Закрыть" tabIndex={props.open ? 0 : -1} onClick={props.onClose} style={{ ...css('position: absolute; inset: 0; z-index: 60; border: 0; padding: 0; background: rgba(0,0,0,.32); transition: opacity .25s ease; cursor: default'), opacity: props.open ? 1 : 0, pointerEvents: props.open ? 'auto' : 'none' }} />
      {props.open ? (
        <div role="alertdialog" aria-label={props.title} className="sq24" style={css('position: absolute; left: 50%; top: 50%; z-index: 61; width: 316px; margin: -110px 0 0 -158px; padding: 20px; background: var(--surface); box-shadow: 0 24px 60px rgba(0,0,0,.25), inset 0 0 0 .4px var(--line); animation: popin .22s cubic-bezier(.2,.8,.2,1)')}>
          <div style={css('font-size: 17px; line-height: 22px; font-weight: 600')}>{props.title}</div>
          <div style={css('margin-top: 6px; font-size: 13.5px; line-height: 19px; color: var(--text2)')}>{props.text}</div>
          {props.input ? (
            <div className="sf sq12" style={css('display: flex; align-items: center; height: 44px; margin-top: 14px; padding: 0 14px; background: var(--bg)')}>
              <input
                ref={ref}
                value={props.input.value}
                onChange={(e) => props.input?.onChange(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && props.onOk()}
                aria-label={props.input.label}
                style={css('flex: 1; min-width: 0; border: 0; outline: none; background: transparent; color: var(--text); font-size: 14.5px; font-family: inherit')}
              />
            </div>
          ) : null}
          <div style={css('display: flex; gap: 8px; margin-top: 18px')}>
            <button onClick={props.onClose} className="tp" style={css('flex: 1; height: 44px; border: 0; border-radius: 999px; background: var(--fill2); color: var(--text); font-size: 14.5px; font-weight: 500; cursor: pointer; font-family: inherit')}>
              Отмена
            </button>
            <button onClick={props.onOk} className="tp" style={{ ...css('flex: 1; height: 44px; border: 0; border-radius: 999px; color: #fff; font-size: 14.5px; font-weight: 600; cursor: pointer; font-family: inherit'), background: props.okBg }}>
              {props.okLabel}
            </button>
          </div>
        </div>
      ) : null}
    </>
  );
}

/** «Сменить сервер?» с «Больше не спрашивать». */
export function Confirm(props: { open: boolean; title: string; text: string; okLabel: string; skip: boolean; onSkip: () => void; onOk: () => void; onCancel: () => void }) {
  if (!props.open) return null;
  return (
    <>
      <button aria-label="Отмена" onClick={props.onCancel} style={css('position: absolute; inset: 0; z-index: 60; border: 0; padding: 0; background: rgba(0,0,0,.32); cursor: default; animation: cfIn .2s ease both')} />
      <div style={css('position: absolute; inset: 0; z-index: 61; display: flex; align-items: center; justify-content: center; padding: 24px; pointer-events: none')}>
        <div role="alertdialog" aria-modal="true" aria-label={props.title} className="sq24" style={css('width: 100%; padding: 20px; background: var(--surface); box-shadow: 0 24px 60px rgba(0,0,0,.25), inset 0 0 0 .4px var(--line); pointer-events: auto; animation: popin .24s cubic-bezier(.2,.8,.2,1) both')}>
          <span aria-hidden="true" style={css('width: 44px; height: 44px; border-radius: 50%; background: color-mix(in srgb, var(--accent) 14%, transparent); color: var(--accent); display: flex; align-items: center; justify-content: center')}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M4 8h13l-3-3M20 16H7l3 3" />
            </svg>
          </span>
          <div style={css('margin-top: 14px; font-size: 17px; line-height: 22px; font-weight: 600')}>{props.title}</div>
          <div style={css('margin-top: 6px; font-size: 13.5px; line-height: 19px; color: var(--text2)')}>{props.text}</div>
          <button onClick={props.onSkip} role="checkbox" aria-checked={props.skip ? 'true' : 'false'} style={css('display: flex; align-items: center; gap: 8px; margin-top: 16px; border: 0; padding: 0; background: transparent; color: var(--text2); font-size: 13px; cursor: pointer; font-family: inherit')}>
            <span aria-hidden="true" style={{ ...css('flex: none; width: 18px; height: 18px; border-radius: 6px; color: var(--onAccent); display: flex; align-items: center; justify-content: center; transition: background-color .2s ease'), background: props.skip ? 'var(--accent)' : 'transparent', boxShadow: props.skip ? 'none' : 'inset 0 0 0 1.6px var(--text3)' }}>
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.2" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: props.skip ? 1 : 0 }}>
                <path d="M5 12.5l4.5 4.5L19 7.5" />
              </svg>
            </span>
            Больше не спрашивать
          </button>
          <div style={css('display: flex; gap: 8px; margin-top: 18px')}>
            <button onClick={props.onCancel} className="tp" style={css('flex: 1; height: 44px; border: 0; border-radius: 999px; background: var(--fill2); color: var(--text); font-size: 14.5px; font-weight: 600; cursor: pointer; font-family: inherit')}>
              Отмена
            </button>
            <button onClick={props.onOk} className="tp" style={css('flex: 1; height: 44px; border: 0; border-radius: 999px; background: var(--accent); color: var(--onAccent); font-size: 14.5px; font-weight: 600; cursor: pointer; font-family: inherit')}>
              {props.okLabel}
            </button>
          </div>
        </div>
      </div>
    </>
  );
}

/** Плитка с буквой: сервисы, программы, сайты. */
export function Letter({ name, tint, size = 32, radius = 10, font = 13 }: { name: string; tint: string; size?: number; radius?: number; font?: number }) {
  return (
    <span aria-hidden="true" style={{ ...css('flex: none; color: #fff; display: flex; align-items: center; justify-content: center; font-weight: 700'), width: size, height: size, borderRadius: radius, background: tint, fontSize: font }}>
      {(name.replace(/^\./, '')[0] ?? '?').toUpperCase()}
    </span>
  );
}

export const TINTS = ['#8e8e93', '#5e6ad2', '#d97757', '#2aabee', '#34c759', '#ff9500'];

/** Цвет плитки по имени — для программ, у которых своего цвета нет. */
export function tintOf(name: string): string {
  const KNOWN: Record<string, string> = { chrome: '#4285f4', 'google chrome': '#4285f4', telegram: '#2aabee', discord: '#5865f2', figma: '#a259ff', claude: '#d97757', spotify: '#1db954', steam: '#1b2838', edge: '#0c59a4', firefox: '#ff7139', youtube: '#ff3b30', instagram: '#e1306c', chatgpt: '#10a37f' };
  const k = KNOWN[name.toLowerCase()];
  if (k) return k;
  let h = 0;
  for (const ch of name) h = (h * 31 + (ch.codePointAt(0) ?? 0)) >>> 0;
  return ['#5e6ad2', '#d97757', '#2aabee', '#34c759', '#ff9500', '#af52de', '#ff2d55', '#8e8e93'][h % 8];
}
