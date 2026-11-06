// Главное окно по макету: заголовок, прокручиваемая страница, нижняя навигация, сообщение, листы и окна поверх.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import mark from '../assets/klick-mark.png';
import { isIos, isMac } from '../lib/platform';
import { useStore } from '../lib/store';
import { systemDark } from '../lib/theme';
import { About, KsPage, LogPage, More } from './Extra';
import { Diag } from './Diag';
import { Home } from './Home';
import { Look } from './Look';
import { Routes } from './Routes';
import { SettingsTab } from './SettingsTab';
import { AddSheet, ConnsSheet, MethodSheet, RulesSheet, SpeedSheet } from './Sheets';
import { css } from './css';
import { TABI } from './paths';
import { cacheAppearance, cachedAppearance, varsFor } from './theme';
import { Confirm, Dialog } from './ui';
import { WinCtx, type AddPrefill, type ConfirmSpec, type DialogSpec, type Page, type SheetName, type TabKey, type Win } from './win';
import { setSkipConfirm, skipConfirm } from './model';

/** Нажали на уведомление: куда вести. */
const NAV: Record<string, [TabKey, Page | null]> = {
  servers: ['home', null],
  card: ['home', null],
  neighbors: ['home', null],
  browser_proxy: ['home', null],
  killswitch: ['custom', 'ks'],
  log: ['custom', 'log'],
  settings: ['custom', null],
};

const TABS: [TabKey, string][] = [
  ['home', 'Главная'],
  ['routes', 'Маршруты'],
  ['custom', 'Настройки'],
];

function useSysDark() {
  const [dark, setDark] = useState(systemDark());
  useEffect(() => {
    if (typeof matchMedia !== 'function') return;
    const mq = matchMedia('(prefers-color-scheme: dark)');
    const f = () => setDark(mq.matches);
    mq.addEventListener('change', f);
    return () => mq.removeEventListener('change', f);
  }, []);
  return dark;
}

export function MainWindow() {
  const store = useStore();
  const { transport, toast, nav, consumeNav, settings } = store;
  const [tab, setTab] = useState<TabKey>('home');
  const [page, setPage] = useState<Page | null>(null);
  const [sheet, setSheet] = useState<SheetName | null>(null);
  const [dlg, setDlg] = useState<DialogSpec | null>(null);
  const [dlVal, setDlVal] = useState('');
  const [cf, setCf] = useState<ConfirmSpec | null>(null);
  const [skip, setSkip] = useState(skipConfirm());
  const [prefill, setPrefill] = useState<AddPrefill | null>(null);
  const [viewId, setViewId] = useState<string | null>(null);
  const scRef = useRef<HTMLDivElement>(null);
  const sysDark = useSysDark();

  const appearance = settings?.appearance ?? cachedAppearance();
  useEffect(() => {
    if (settings?.appearance) cacheAppearance(settings.appearance);
  }, [settings?.appearance]);
  const vars = useMemo(() => varsFor(appearance, sysDark), [appearance, sysDark]);
  const cssVars = useMemo(() => Object.fromEntries(Object.entries(vars).map(([k, v]) => [`--${k}`, v])) as React.CSSProperties, [vars]);
  useEffect(() => {
    if (transport.kind === 'tauri') document.body.style.background = vars.bg;
  }, [vars.bg, transport.kind]);

  const openAdd = useCallback((p?: Omit<AddPrefill, 'seq'>) => {
    setPrefill(p ? { ...p, seq: Date.now() } : null);
    setSheet('add');
  }, []);

  // Ссылка klick://add (её проверяет и держит хранилище) или ссылка из буфера по «Добавить» в сообщении:
  // лист «Добавить подключение» с уже вставленной ссылкой. Сама она ничего не добавляет.
  const { incoming, consumeIncoming } = store;
  useEffect(() => {
    if (!incoming) return;
    setPage(null);
    openAdd({ link: incoming.url, name: incoming.name, from: incoming.clipboard ? null : incoming.host || 'ссылка подписки' });
    consumeIncoming();
  }, [incoming, consumeIncoming, openAdd]);

  useEffect(() => {
    if (!nav) return;
    if (nav.target.startsWith('conn:')) {
      setTab('home');
      setPage(null);
      setSheet(null);
      setViewId(nav.target.slice(5));
      consumeNav();
      return;
    }
    const t = NAV[nav.target];
    if (!t) return;
    setTab(t[0]);
    setPage(t[1]);
    setSheet(null);
    consumeNav();
  }, [nav, consumeNav]);

  // Новая вкладка или страница — наверх.
  useEffect(() => {
    scRef.current?.scrollTo({ top: 0 });
  }, [tab, page]);

  // Esc закрывает верхнее: окно, лист.
  useEffect(() => {
    const f = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (cf) setCf(null);
      else if (dlg) setDlg(null);
      else if (sheet) setSheet(null);
    };
    window.addEventListener('keydown', f);
    return () => window.removeEventListener('keydown', f);
  }, [cf, dlg, sheet]);

  const win: Win = {
    tab,
    page,
    go: (t) => {
      setTab(t);
      setPage(null);
    },
    open: setPage,
    back: () => setPage(null),
    sheet,
    openSheet: setSheet,
    closeSheet: () => setSheet(null),
    dialog: (d) => {
      setDlg(d);
      if (d?.input) setDlVal(d.input.value);
    },
    confirm: (c) => {
      if (skip) c.onOk();
      else setCf(c);
    },
    prefill,
    openAdd,
    viewId,
    setViewId,
  };

  const backLabel = page === 'diag' && tab === 'routes' ? 'Маршруты' : page === 'diag' && tab === 'home' ? 'Главная' : 'Настройки';
  const last = store.toasts[store.toasts.length - 1];

  return (
    <WinCtx.Provider value={win}>
      <div className="bm bm-root" style={cssVars}>
        <TitleBar onMin={() => transport.win.minimize()} onClose={() => transport.win.hide()} />

        <div ref={scRef} className="ksc bm-scroll" style={css('position: absolute; left: 0; right: 0; top: 40px; bottom: 0; overflow-y: auto')}>
          <div style={css('padding: 24px 16px 110px')}>
            {page === null && tab === 'home' ? <Home dark={vars.bg !== '#f5f5f7'} /> : null}
            {page === null && tab === 'routes' ? <Routes /> : null}
            {page === null && tab === 'custom' ? <SettingsTab /> : null}
            {page === 'diag' ? <Diag backLabel={backLabel} /> : null}
            {page === 'look' ? <Look /> : null}
            {page === 'ks' ? <KsPage /> : null}
            {page === 'log' ? <LogPage /> : null}
            {page === 'about' ? <About /> : null}
            {page === 'more' ? <More /> : null}
          </div>
        </div>

        <div aria-hidden="true" className="bm-fade" style={css('position: absolute; left: 0; right: 0; bottom: 0; height: 85px; background: linear-gradient(180deg, color-mix(in srgb, var(--fade) 0%, transparent) 0%, var(--fade) 70%); pointer-events: none; z-index: 14')} />
        <nav aria-label="Навигация" className="sf bm-nav" style={css('position: absolute; left: 50%; bottom: 16px; z-index: 15; display: flex; gap: 4px; padding: 4px; transform: translateX(-50%); border-radius: 999px')}>
          {TABS.map(([key, label]) => {
            const a = tab === key;
            const T = TABI[key];
            return (
              <button
                key={key}
                onClick={() => win.go(key)}
                aria-current={a ? 'page' : 'false'}
                aria-label={label}
                title={label}
                className="tp"
                style={{ ...css('width: 56px; height: 44px; display: flex; align-items: center; justify-content: center; border: 0; border-radius: 999px; cursor: pointer'), background: a ? 'var(--fill2)' : 'transparent', color: a ? 'var(--text)' : 'var(--navIcon)' }}
              >
                <svg width="25" height="25" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path fillRule={T[0][0] as 'evenodd' | 'nonzero'} d={T[0][1]} />
                  <path fillRule={T[1][0] as 'evenodd' | 'nonzero'} d={T[1][1]} />
                </svg>
              </button>
            );
          })}
        </nav>

        <ToastView toast={last} onAct={(id) => store.dismiss(id)} />

        <SpeedSheet />
        <AddSheet />
        <RulesSheet />
        <ConnsSheet />
        <MethodSheet />

        <Dialog
          open={!!dlg}
          title={dlg?.title ?? ''}
          text={dlg?.text ?? ''}
          okLabel={dlg?.okLabel ?? ''}
          okBg={dlg?.okBg ?? 'var(--accent)'}
          input={dlg?.input ? { value: dlVal, onChange: setDlVal, label: dlg.input.label } : undefined}
          onClose={() => setDlg(null)}
          onOk={() => {
            if (!dlg) return;
            if (dlg.input && !dlVal.trim()) return;
            const d = dlg;
            setDlg(null);
            d.onOk(dlVal.trim());
          }}
        />
        <Confirm
          open={!!cf}
          title={cf?.title ?? ''}
          text={cf?.text ?? ''}
          okLabel={cf?.okLabel ?? ''}
          skip={skip}
          onSkip={() => {
            setSkip(!skip);
            setSkipConfirm(!skip);
          }}
          onCancel={() => setCf(null)}
          onOk={() => {
            const c = cf;
            setCf(null);
            c?.onOk();
          }}
        />
        <Offline />
      </div>
    </WinCtx.Provider>
  );
}

/** Заголовок окна. На Windows — свои кнопки «свернуть» и «в трей», как в макете; на macOS кнопки окна
 *  системные («светофоры» слева); на iOS окно во весь экран — ни кнопок, ни перетаскивания. */
function TitleBar({ onMin, onClose }: { onMin: () => void; onClose: () => void }) {
  const drag = isIos ? {} : { 'data-tauri-drag-region': true };
  return (
    <div {...drag} className="bm-title" style={css('position: absolute; left: 0; right: 0; top: 0; height: 40px; display: flex; align-items: center; gap: 10px; padding: 0 12px 0 16px; box-shadow: inset 0 -.4px 0 var(--line); background: var(--bg); z-index: 20; transition: background-color .3s ease')}>
      <img src={mark} alt="kl!ck" draggable={false} {...drag} style={css('width: 20px; height: 20px; border-radius: 50%; display: block')} />
      <span {...drag} style={css('flex: 1; align-self: stretch')} />
      {isMac || isIos ? null : (
        <div style={css('display: flex; gap: 4px')}>
          <button className="cap" aria-label="Свернуть" onClick={onMin}>
            <svg width="20" height="20" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
              <path d="M17.5 10a.63.63 0 0 1-.62.63H3.13a.63.63 0 1 1 0-1.25h13.75c.34 0 .62.28.62.62z" />
            </svg>
          </button>
          <button className="cap x" aria-label="Закрыть в трей" onClick={onClose}>
            <svg width="20" height="20" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
              <path d="M16.07 15.18a.63.63 0 1 1-.89.89L10 10.88l-5.18 5.19a.63.63 0 0 1-.89-.89L9.12 10 3.93 4.82a.63.63 0 1 1 .89-.89L10 9.12l5.18-5.19a.63.63 0 1 1 .89.89L10.88 10z" />
            </svg>
          </button>
        </div>
      )}
    </div>
  );
}

const DOT: Record<string, string> = { ok: '#32d74b', warn: '#ff9f0a', bad: '#ff453a', dim: '#8e8e93' };

/** Сообщение снизу, как в макете: тёмная капсула с точкой; держится, пока его не уберёт хранилище. */
function ToastView({ toast, onAct }: { toast: ReturnType<typeof useStore>['toasts'][number] | undefined; onAct: (id: number) => void }) {
  const [shown, setShown] = useState(toast);
  useEffect(() => {
    if (toast) setShown(toast);
  }, [toast]);
  const t = toast ?? shown;
  const on = !!toast;
  return (
    <div
      role="status"
      aria-live="polite"
      style={{
        ...css('position: absolute; left: 50%; bottom: 82px; z-index: 25; display: flex; align-items: center; gap: 8px; padding: 9px 14px; border-radius: 999px; background: var(--accentToast); color: #fff; font-size: 12.5px; font-weight: 500; white-space: nowrap; box-shadow: 0 10px 24px rgba(0,0,0,.18); transition: transform .35s cubic-bezier(.32,.72,0,1), opacity .25s ease'),
        maxWidth: 'calc(100% - 32px)',
        transform: on ? 'translate(-50%, 0)' : 'translate(-50%, 16px)',
        opacity: on ? 1 : 0,
        pointerEvents: on && t?.action ? 'auto' : 'none',
      }}
    >
      <span aria-hidden="true" style={{ ...css('flex: none; width: 6px; height: 6px; border-radius: 50%'), background: DOT[t?.tone ?? 'ok'] }} />
      <span style={css('min-width: 0; overflow: hidden; text-overflow: ellipsis')}>{t?.title ?? ''}</span>
      {t?.action ? (
        <button
          onClick={() => {
            t.action?.run();
            onAct(t.id);
          }}
          style={css('flex: none; margin: -4px -6px -4px 6px; height: 26px; padding: 0 10px; border: 0; border-radius: 999px; background: rgba(255,255,255,.14); color: #fff; font-size: 12.5px; font-weight: 600; cursor: pointer; font-family: inherit')}
        >
          {t.action.label}
        </button>
      ) : null}
    </div>
  );
}

/** Служба не отвечает — в духе макета: карточка поверх всего. */
function Offline() {
  const { serviceUp, transport } = useStore();
  if (serviceUp) return null;
  return (
    <div style={css('position: absolute; inset: 40px 0 0; z-index: 70; display: flex; align-items: center; justify-content: center; padding: 24px; background: color-mix(in srgb, var(--bg) 82%, transparent)')}>
      <div className="sf sq24" role="alert" style={css('width: 100%; padding: 20px')}>
        <span aria-hidden="true" style={css('width: 44px; height: 44px; border-radius: 50%; background: color-mix(in srgb, var(--bad) 14%, transparent); color: var(--bad); display: flex; align-items: center; justify-content: center')}>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 8v5M12 16.5h.01M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z" />
          </svg>
        </span>
        <div style={css('margin-top: 14px; font-size: 17px; line-height: 22px; font-weight: 600')}>Служба kl!ck не отвечает</div>
        <div style={css('margin-top: 6px; font-size: 13.5px; line-height: 19px; color: var(--text2)')}>
          {transport.kind === 'tauri' ? 'Окно работает, но без службы VPN не включить. Переустановите kl!ck или перезагрузите компьютер.' : 'Тестовая служба недоступна.'}
        </div>
      </div>
    </div>
  );
}
