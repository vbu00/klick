import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { isMac } from './platform';
import type { Transport } from './transport';
import type { KEvent, PendingLink } from './types';

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
    window: win.label === 'tray' ? 'tray' : 'main',
    call: <T>(cmd: string, args?: unknown) => invoke<T>('service_call', { request: args === undefined ? { cmd } : { cmd, args } }),
    onEvent: (cb) => subscribe<KEvent>('klick://event', cb),
    onService: (cb) => {
      invoke<boolean>('service_up').then(cb).catch(() => cb(false));
      return subscribe<{ up: boolean }>('klick://service', (p) => cb(p.up));
    },
    win: {
      minimize: () => void win.minimize(),
      hide: () => void win.hide(),
    },
    pickExe: async () => {
      // macOS: программа — пакет .app, по умолчанию из папки «Программы».
      const path = isMac
        ? await open({ title: 'Выберите программу', multiple: false, directory: false, defaultPath: '/Applications', filters: [{ name: 'Программы', extensions: ['app'] }] })
        : await open({ title: 'Выберите программу', multiple: false, directory: false, filters: [{ name: 'Программы', extensions: ['exe'] }] });
      return typeof path === 'string' ? path : null;
    },
    openUrl: (url) => openUrl(url),
    openMain: (target) => void invoke('open_main', { target: target ?? null }),
    hideTray: () => void invoke('hide_tray'),
    fitTray: (height) => void invoke('fit_tray', { height }),
    clipboardText: () => invoke<string>('clipboard_text'),
    repairService: () => invoke('repair_service'),
    exit: (clearProxy) => invoke('app_exit', { clearProxy }),
    isActive: async () => (await win.isVisible()) && (await win.isFocused()) && !(await win.isMinimized()),
    notify: (title, text, target) => invoke('notify', { title, text, target: target ?? null }),
    onNavigate: (cb) => subscribe<string>('klick://navigate', cb),
    onExitRequest: (cb) => subscribe<null>('klick://exit-request', () => cb()),
    takePendingLink: () => invoke<PendingLink | null>('take_pending_link'),
    onAddLink: (cb) => subscribe<null>('klick://add-link', () => cb()),
    autostart: {
      get: () => isEnabled(),
      set: (on) => (on ? enable() : disable()),
    },
  };
}
