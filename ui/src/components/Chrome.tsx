// Рамка окна: заголовок с кнопками, нижняя панель, всплывающие сообщения, лист снизу, «служба не отвечает».

import { useState, type ReactNode } from 'react';
import statusDefault from '../assets/status/default.svg';
import statusError from '../assets/status/error.svg';
import statusEmpty from '../assets/status/no-connection.svg';
import statusSuccess from '../assets/status/success.svg';
import statusWarning from '../assets/status/warning.svg';
import { isMac } from '../lib/platform';
import { useStore } from '../lib/store';
import type { VpnState } from '../lib/types';
import { Icon, type IconName } from './Icon';

export type Tab = 'home' | 'connection' | 'add' | 'settings';

/** Значок в заголовке — тот же, что в трее: цвет по состоянию подключения. */
function statusIcon(vpn: VpnState | undefined, empty: boolean, serviceUp: boolean): string {
  if (!serviceUp) return statusError;
  switch (vpn) {
    case 'connected':
      return statusSuccess;
    case 'connecting':
    case 'reconnecting':
    case 'server_down':
      return statusWarning;
    case 'error':
      return statusError;
    default:
      return empty ? statusEmpty : statusDefault;
  }
}

/** Окно фиксированного размера: кнопки «развернуть» нет. На macOS кнопки окна — системные «светофоры»
 *  слева (заголовок окна прозрачный), свои кнопки не рисуем. */
export function TitleBar() {
  const { transport, state, serviceUp } = useStore();
  const icon = statusIcon(state?.vpn, !!state && !state.connection, serviceUp);
  return (
    <header className="titlebar" data-tauri-drag-region>
      <img className="logo" src={icon} alt="" draggable={false} data-tauri-drag-region />
      <span className="app-name" data-tauri-drag-region>
        kl!ck
      </span>
      <div className="drag" data-tauri-drag-region />
      {isMac ? null : (
        <>
          <button className="tb-btn" aria-label="Свернуть" onClick={() => transport.win.minimize()}>
            <Icon name="winMin" size={16} />
          </button>
          <button className="tb-btn close" aria-label="Свернуть в трей" onClick={() => transport.win.hide()}>
            <Icon name="winClose" size={16} />
          </button>
        </>
      )}
    </header>
  );
}

const TABS: [Tab, IconName, string][] = [
  ['home', 'home', 'Главная'],
  ['connection', 'rules', 'Соединение'],
  ['add', 'plus', 'Добавить'],
  ['settings', 'settings', 'Настройки'],
];

export function BottomNav({ tab, onTab }: { tab: Tab; onTab: (t: Tab) => void }) {
  return (
    <>
      <div className="nav-fade" />
      <nav className="nav" aria-label="Разделы">
        <div className="nav-inner">
          {TABS.map(([key, icon, title]) => (
            <button key={key} className={key === tab ? 'nav-btn on' : 'nav-btn'} aria-label={title} title={title} aria-current={key === tab ? 'page' : undefined} onClick={() => onTab(key)}>
              <Icon name={icon} size={24} />
            </button>
          ))}
        </div>
      </nav>
    </>
  );
}

const TONE: Record<string, string> = { ok: 'var(--accent)', warn: 'var(--orange)', bad: 'var(--red)', dim: 'var(--dim)' };

export function Toasts() {
  const { toasts, dismiss } = useStore();
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className="toast">
          <div className="dot8" style={{ background: TONE[t.tone] }} />
          <div className="toast-body">
            <b>{t.title}</b>
            {t.text ? <span>{t.text}</span> : null}
          </div>
          {t.action ? (
            <button
              onClick={() => {
                t.action?.run();
                dismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          ) : null}
        </div>
      ))}
    </div>
  );
}

export function Sheet({ onClose, children, tall }: { onClose: () => void; children: ReactNode; tall?: boolean }) {
  return (
    <div className="sheet-root" role="dialog" aria-modal="true">
      <div className="sheet-shade" onClick={onClose} />
      <div className={tall ? 'sheet tall' : 'sheet'}>
        <div className="sheet-grip" />
        {children}
      </div>
    </div>
  );
}

export function Offline() {
  const { serviceUp, transport, state, toast } = useStore();
  const [busy, setBusy] = useState(false);
  if (serviceUp) return null;
  // macOS: службу можно поднять прямо отсюда — система спросит пароль администратора.
  const canRepair = isMac;
  const repair = async () => {
    setBusy(true);
    try {
      await transport.repairService();
      toast('Служба перезапущена', 'VPN выключен — включите его, когда будете готовы', 'ok');
    } catch (e) {
      if (String(e) !== 'cancelled') toast('Службу не удалось перезапустить', String(e).slice(0, 160), 'bad');
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="offline">
      <b>Служба kl!ck не отвечает</b>
      <span>
        {transport.kind !== 'tauri' && !canRepair
          ? 'Тестовая служба недоступна.'
          : canRepair
            ? 'Окно работает, но без службы VPN не включить. Перезапустите её — macOS спросит пароль администратора.'
            : 'Окно работает, но без службы VPN не включить. Переустановите kl!ck или перезагрузите компьютер.'}
        {isMac && state?.kill_switch ? ' Если в Kill Switch есть программы, прямые подключения закрыты, пока служба не вернётся.' : null}
      </span>
      {canRepair ? (
        <button className="offline-action" disabled={busy} onClick={() => void repair()}>
          {busy ? 'Перезапускаю…' : 'Перезапустить службу'}
        </button>
      ) : null}
    </div>
  );
}
