// Навигация главного окна: вкладки, страницы внутри вкладок, листы снизу и окна поверх.

import { createContext, useContext } from 'react';

export type TabKey = 'home' | 'routes' | 'custom';
/** Страницы с кнопкой «назад»: из макета — «Оформление» и «Диагностика»; остальные — функции, которых в макете нет. */
export type Page = 'look' | 'diag' | 'ks' | 'log' | 'about' | 'more';
export type SheetName = 'speed' | 'add' | 'rules' | 'method' | 'conns';

export interface DialogSpec {
  title: string;
  text: string;
  okLabel: string;
  okBg: string;
  input?: { value: string; label: string };
  onOk: (value: string) => void;
}

export interface ConfirmSpec {
  title: string;
  text: string;
  okLabel: string;
  onOk: () => void;
}

/** Что подставить в «Добавить подключение»: из буфера или по ссылке klick://add. */
export interface AddPrefill {
  link: string;
  name: string;
  /** Пришло со страницы подписки: показываем только домен. */
  from: string | null;
  seq: number;
}

export interface Win {
  tab: TabKey;
  page: Page | null;
  go(tab: TabKey): void;
  open(page: Page): void;
  back(): void;
  sheet: SheetName | null;
  openSheet(s: SheetName): void;
  closeSheet(): void;
  dialog(d: DialogSpec | null): void;
  confirm(c: ConfirmSpec): void;
  prefill: AddPrefill | null;
  openAdd(p?: Omit<AddPrefill, 'seq'>): void;
  /** Подключение, открытое на главной. */
  viewId: string | null;
  setViewId(id: string | null): void;
}

export const WinCtx = createContext<Win | null>(null);

export function useWin(): Win {
  const w = useContext(WinCtx);
  if (!w) throw new Error('useWin вне окна');
  return w;
}
