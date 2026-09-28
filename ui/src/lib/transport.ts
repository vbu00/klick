import type { KEvent } from './types';

/** Как окно говорит со службой. В Tauri — через канал управления, в браузере — с тестовой службой. */
export interface Transport {
  kind: 'tauri' | 'mock';
  /** Какое это окно: главное или окно трея. */
  window: 'main' | 'tray';
  call<T = unknown>(cmd: string, args?: unknown): Promise<T>;
  onEvent(cb: (e: KEvent) => void): () => void;
  onService(cb: (up: boolean) => void): () => void;
  win: { minimize(): void; hide(): void };
  /** «Обзор…»: выбрать exe программы; null — окно выбора закрыли. */
  pickExe(): Promise<string | null>;
  /** Открыть ссылку в браузере по умолчанию. */
  openUrl(url: string): Promise<void>;
  /** «Запускать с Windows»: запись автозапуска окна у текущего пользователя. */
  autostart: { get(): Promise<boolean>; set(on: boolean): Promise<void> };
  /** Показать главное окно (из трея); `target` — какой экран открыть. */
  openMain(target?: string): void;
  hideTray(): void;
  /** Окно трея по высоте содержимого. */
  fitTray(height: number): void;
  /** Текст из буфера обмена: «Вставить из буфера». */
  clipboardText(): Promise<string>;
  /** Закрыть kl!ck; `clearProxy` — VPN выключили, снять и системный прокси. */
  exit(clearProxy: boolean): Promise<void>;
  /** Главное окно сейчас перед глазами: тогда вместо уведомления Windows — сообщение в окне. */
  isActive(): Promise<boolean>;
  /** Уведомление Windows; `target` — куда вести по нажатию. */
  notify(title: string, text: string, target?: string): Promise<void>;
  /** Нажали на уведомление: открыть нужный экран. */
  onNavigate(cb: (target: string) => void): () => void;
  /** «Выход» из меню значка: окно трея спрашивает, что делать с VPN. */
  onExitRequest(cb: () => void): () => void;
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function createTransport(): Promise<Transport> {
  if (isTauri()) {
    const m = await import('./tauri');
    return m.createTauriTransport();
  }
  const m = await import('./mock');
  return m.createMockTransport();
}
