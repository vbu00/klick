import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { isIos, isMac } from './platform';
import type { Transport } from './transport';
import type { KEvent, PendingLink } from './types';

/** iOS: Rust-часть знает только `service_call`, `service_up`, `clipboard_text`, `take_pending_link` и opener —
 *  остальное ничего не делает, а не падает. */
const none = () => undefined;

/** Окно внутри Tauri: команды и события идут через Rust-часть к службе. */
export function createTauriTransport(): Transport {
  const win = getCurrentWindow();
  const subscribe = <T>(name: string, cb: (payload: T) => void) => {
    let off: (() => void) | undefined;
    let dead = false;
    listen<T>(name, (e) => cb(e.payload)).then((u) => (dead ? u() : (off = u)));
    return () => {
      dead = true;
      off?.();
    };
  };
  return {
    kind: 'tauri',
    window: !isIos && win.label === 'tray' ? 'tray' : 'main',
    call: <T>(cmd: string, args?: unknown) => invoke<T>('service_call', { request: args === undefined ? { cmd } : { cmd, args } }),
    onEvent: (cb) => subscribe<KEvent>('klick://event', cb),
    onService: (cb) => {
      invoke<boolean>('service_up').then(cb).catch(() => cb(false));
      return subscribe<{ up: boolean }>('klick://service', (p) => cb(p.up));
    },
    win: {
      minimize: () => (isIos ? undefined : void win.minimize()),
      // Через Rust: заодно перестаёт рисоваться страница скрытого окна.
      hide: () => (isIos ? undefined : void invoke('hide_self')),
    },
    pickExe: async () => {
      if (isIos) return null;
      // macOS: программа — пакет .app, по умолчанию из папки «Программы».
      const path = isMac
        ? await open({ title: 'Выберите программу', multiple: false, directory: false, defaultPath: '/Applications', filters: [{ name: 'Программы', extensions: ['app'] }] })
        : await open({ title: 'Выберите программу', multiple: false, directory: false, filters: [{ name: 'Программы', extensions: ['exe'] }] });
      return typeof path === 'string' ? path : null;
    },
    openUrl: (url) => openUrl(url),
    openMain: (target) => (isIos ? undefined : void invoke('open_main', { target: target ?? null })),
    hideTray: () => (isIos ? undefined : void invoke('hide_tray')),
    fitTray: (height) => (isIos ? undefined : void invoke('fit_tray', { height })),
    clipboardText: () => invoke<string>('clipboard_text'),
    repairService: async () => (isIos ? undefined : invoke('repair_service')),
    exit: async (clearProxy) => (isIos ? undefined : invoke('app_exit', { clearProxy })),
    // iOS: окно одно, на экране оно, пока страница видна.
    isActive: async () => (isIos ? !document.hidden : (await win.isVisible()) && (await win.isFocused()) && !(await win.isMinimized())),
    notify: async (title, text, target) => (isIos ? undefined : invoke('notify', { title, text, target: target ?? null })),
    onNavigate: (cb) => (isIos ? none : subscribe<string>('klick://navigate', cb)),
    onExitRequest: (cb) => (isIos ? none : subscribe<null>('klick://exit-request', () => cb())),
    takePendingLink: () => invoke<PendingLink | null>('take_pending_link'),
    onAddLink: (cb) => subscribe<null>('klick://add-link', () => cb()),
    autostart: {
      get: async () => (isIos ? false : isEnabled()),
      set: async (on) => (isIos ? undefined : on ? enable() : disable()),
    },
  };
}
