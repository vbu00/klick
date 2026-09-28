// Связь страницы установщика с его Rust-частью. В браузере (превью, скриншоты) — макет с
// подставными данными: `setup.html?s=old`, `?s=maintain`, `?s=same`, `?s=newer`, `?s=uninstall`, `?s=fail`.

export type Kind = 'install' | 'update' | 'reinstall' | 'uninstall';
export type TaskId = 'stop' | 'files' | 'core' | 'old' | 'service' | 'shortcuts' | 'stop_service' | 'unhook' | 'driver' | 'remove' | 'data';

export interface Installed {
  version: string;
  path: string;
  relation: 'older' | 'same' | 'newer';
}

export interface Old {
  version: string | null;
  dirs: string[];
  uninstaller: string | null;
  data: string | null;
  task: boolean;
}

export interface Info {
  version: string;
  core_version: string;
  default_path: string;
  size: number;
  installed: Installed | null;
  old: Old | null;
  data_path: string;
}

export interface Hello {
  start: 'install' | 'maintain' | 'uninstall';
  info: Info;
}

export interface Progress {
  tasks: TaskId[];
  active: number;
  pct: number;
  ceil: number;
  cancellable: boolean;
}

export interface Outcome {
  ok: boolean;
  cancelled: boolean;
  error: string | null;
  detail: string | null;
  rolled_back: boolean;
  notes: string[];
  path: string;
}

export interface Request {
  kind: Kind;
  path: string;
  desktop: boolean;
  autostart: boolean;
  wipe: boolean;
}

export type PathCheck = { ok: true; path: string } | { ok: false; code: string };

export interface Api {
  inTauri: boolean;
  hello(): Promise<Hello>;
  checkPath(path: string): Promise<PathCheck>;
  freeSpace(path: string): Promise<number | null>;
  licenses(): Promise<{ klick: string; notices: string }>;
  start(req: Request): Promise<void>;
  cancel(): Promise<void>;
  launch(path: string): Promise<void>;
  quit(): Promise<void>;
  minimize(): void;
  pickFolder(): Promise<string | null>;
  onProgress(cb: (p: Progress) => void): void;
  onDone(cb: (o: Outcome) => void): void;
  onClose(cb: () => void): void;
}

/** Какие задачи будут — как в Rust (`steps::tasks`): чтобы список был на экране сразу. */
export function expectedTasks(req: Request, info: Info): TaskId[] {
  if (req.kind === 'uninstall') return ['stop_service', 'unhook', 'driver', 'remove', ...(req.wipe ? (['data'] as TaskId[]) : [])];
  return [...(req.kind === 'install' ? [] : (['stop'] as TaskId[])), 'files', 'core', ...(info.old ? (['old'] as TaskId[]) : []), 'service', 'shortcuts'];
}

export async function createApi(): Promise<Api> {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) return tauriApi();
  return mockApi(new URLSearchParams(location.search).get('s') ?? 'install');
}

async function tauriApi(): Promise<Api> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const { open } = await import('@tauri-apps/plugin-dialog');
  return {
    inTauri: true,
    hello: () => invoke<Hello>('setup_info'),
    checkPath: (path) =>
      invoke<string>('check_path', { path }).then(
        (p): PathCheck => ({ ok: true, path: p }),
        (code): PathCheck => ({ ok: false, code: String(code) }),
      ),
    freeSpace: (path) => invoke<number | null>('free_space', { path }),
    licenses: () => invoke('licenses'),
    start: (req) => invoke('start', { req }),
    cancel: () => invoke('cancel'),
    launch: (path) => invoke('launch_app', { path }),
    quit: () => invoke('quit'),
    minimize: () => void getCurrentWindow().minimize(),
    pickFolder: async () => {
      const p = await open({ title: 'Папка для kl!ck', directory: true, multiple: false });
      return typeof p === 'string' ? p : null;
    },
    onProgress: (cb) => void listen<Progress>('setup://progress', (e) => cb(e.payload)),
    onDone: (cb) => void listen<Outcome>('setup://done', (e) => cb(e.payload)),
    onClose: (cb) => void listen('setup://close', () => cb()),
  };
}

// ── Макет для браузера ─────────────────────────────────────────────────

const GB = 1024 ** 3;

function mockApi(scenario: string): Api {
  const installed: Installed | null =
    scenario === 'maintain' || scenario === 'uninstall'
      ? { version: '0.3.0', path: 'C:\\Program Files\\klick', relation: 'older' }
      : scenario === 'same'
        ? { version: '0.4.0', path: 'C:\\Program Files\\klick', relation: 'same' }
        : scenario === 'newer'
          ? { version: '1.0.0', path: 'C:\\Program Files\\klick', relation: 'newer' }
          : null;
  const old: Old | null =
    scenario === 'old'
      ? { version: '0.2.1', dirs: ['C:\\Program Files\\kl!ck'], uninstaller: 'C:\\Program Files\\kl!ck\\uninstall.exe', data: 'C:\\Users\\user\\AppData\\Local\\com.vbu00.klick', task: true }
      : null;
  const info: Info = {
    version: '0.4.0',
    core_version: 'v1.19.31',
    default_path: installed?.path ?? 'C:\\Program Files\\klick',
    size: 142 * 1024 * 1024,
    installed,
    old,
    data_path: 'C:\\ProgramData\\klick',
  };
  const start = scenario === 'uninstall' ? 'uninstall' : installed ? 'maintain' : 'install';
  let progress: (p: Progress) => void = () => {};
  let done: (o: Outcome) => void = () => {};
  let timer = 0;
  let cancelled = false;

  const tasksFor = (req: Request) => expectedTasks(req, info);

  return {
    inTauri: false,
    hello: async () => ({ start, info }),
    checkPath: async (path) => {
      const p = path.trim().replace(/\//g, '\\').replace(/\\+$/, '');
      if (!/^[a-zA-Z]:\\./.test(p)) return { ok: false, code: 'path.absolute' };
      if (/[<>:"|?*]/.test(p.slice(3))) return { ok: false, code: 'path.chars' };
      if (/^c:\\users(\\|$)/i.test(p)) return { ok: false, code: 'path.profile' };
      if (/^c:\\windows(\\|$)/i.test(p)) return { ok: false, code: 'path.system' };
      if (/^[a-z]:\\(program files|games)$/i.test(p)) return { ok: false, code: 'path.not_empty' };
      return { ok: true, path: p };
    },
    freeSpace: async (path) => (/^d:/i.test(path) ? 1.4 * GB : 212 * GB),
    licenses: async () => ({ klick: 'MIT License\n\nCopyright (c) 2026 vbu00\n\nPermission is hereby granted, free of charge, to any person obtaining a copy…', notices: '' }),
    start: async (req) => {
      cancelled = false;
      const tasks = tasksFor(req);
      const fail = scenario === 'fail';
      let pct = 0;
      clearInterval(timer);
      timer = window.setInterval(() => {
        if (cancelled) {
          clearInterval(timer);
          done({ ok: false, cancelled: true, error: null, detail: null, rolled_back: true, notes: [], path: req.path });
          return;
        }
        pct = Math.min(100, pct + 1.2);
        const each = 100 / tasks.length;
        const active = Math.min(tasks.length - 1, Math.floor(pct / each));
        const cancellable = req.kind === 'uninstall' ? active === 0 : tasks[active] === 'stop' || tasks[active] === 'files' || tasks[active] === 'core';
        progress({ tasks, active: pct >= 100 ? tasks.length : active, pct, ceil: Math.min(100, (active + 1) * each), cancellable: cancellable && pct < 100 });
        if (fail && pct >= 38) {
          clearInterval(timer);
          done({ ok: false, cancelled: false, error: 'files.write', detail: 'klick-service.exe: Отказано в доступе. (os error 5)', rolled_back: true, notes: [], path: req.path });
        } else if (pct >= 100) {
          clearInterval(timer);
          setTimeout(() => done({ ok: true, cancelled: false, error: null, detail: null, rolled_back: false, notes: old ? ['note.reboot'] : [], path: req.path }), 300);
        }
      }, 60);
    },
    cancel: async () => {
      cancelled = true;
    },
    launch: async () => {},
    quit: async () => {
      location.reload();
    },
    minimize: () => {},
    pickFolder: async () => 'D:\\Games',
    onProgress: (cb) => (progress = cb),
    onDone: (cb) => (done = cb),
    onClose: () => {},
  };
}
