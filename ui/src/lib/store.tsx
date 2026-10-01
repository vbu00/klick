// Состояние окна: зеркало службы плюс то, что нужно только интерфейсу (всплывающие сообщения, график).

import { createContext, useCallback, useContext, useEffect, useMemo, useReducer, useRef, type ReactNode } from 'react';
import { DROP_NOTICES, errorText, noticeTarget, noticeText } from './i18n';
import { planToggle, positionVerb } from './live';
import type { Transport } from './transport';
import { applyTheme, resolveTheme, systemDark } from './theme';
import type { ConnectionView, ErrorInfo, KEvent, KillSwitch, Mode, Preferences, Route, Routing, Rule, ServerView, Service, Settings, StateView, Target } from './types';

export type Tone = 'ok' | 'warn' | 'bad' | 'dim';

export interface Toast {
  id: number;
  title: string;
  text?: string;
  tone: Tone;
  action?: { label: string; run: () => void };
}

/** Подписка из ссылки `klick://add`, прошедшей проверки: экран «Добавить» с уже вставленной ссылкой. */
export interface IncomingLink {
  url: string;
  name: string;
  host: string;
  seq: number;
}

interface Traffic {
  up: number;
  down: number;
  upHist: number[];
  downHist: number[];
  upSum: number;
  downSum: number;
}

interface S {
  serviceUp: boolean;
  loaded: boolean;
  state: StateView | null;
  settings: Settings | null;
  servers: Record<string, ServerView[]>;
  pinging: boolean;
  /** Подключения, чьи серверы уже проверяли на задержку: только у них null значит «нет ответа». */
  tested: Record<string, boolean>;
  traffic: Traffic;
  toasts: Toast[];
  /** Кто мешает режиму VPN: zapret, другой VPN. Узнаём из уведомления службы при подключении. */
  neighbors: string[];
  /** Расширения браузеров, перехватившие прокси: мешают режиму «Системный прокси». */
  browserProxy: string[];
  /** Каталог сервисов для списков. */
  catalog: Service[];
  /** Понятные названия программ по папкам правил и Kill Switch — от службы. */
  programNames: Record<string, string>;
  /** Куда перейти: нажали на уведомление Windows; `conn:<id>` — показать это подключение. */
  nav: { target: string; seq: number } | null;
  /** Ссылка `klick://add`, которую ещё не показал экран «Добавить». */
  incoming: IncomingLink | null;
}

type A =
  | { t: 'service'; up: boolean }
  | { t: 'state'; state: StateView }
  | { t: 'settings'; settings: Settings }
  | { t: 'servers'; id: string; list: ServerView[] }
  | { t: 'pinging'; on: boolean }
  | { t: 'tested'; id: string }
  | { t: 'traffic'; up: number; down: number }
  | { t: 'toast'; toast: Toast }
  | { t: 'dismiss'; id: number }
  | { t: 'neighbors'; names: string[] }
  | { t: 'browserProxy'; names: string[] }
  | { t: 'catalog'; list: Service[] }
  | { t: 'killswitch'; ks: KillSwitch }
  | { t: 'names'; names: Record<string, string> }
  | { t: 'nav'; target: string | null }
  | { t: 'incoming'; link: IncomingLink | null }
  | { t: 'list'; position: Routing; list: Rule[] };

const HIST = 60;
const zeros = () => Array<number>(HIST).fill(0);
const emptyTraffic = (): Traffic => ({ up: 0, down: 0, upHist: zeros(), downHist: zeros(), upSum: 0, downSum: 0 });

const initial: S = {
  serviceUp: true,
  loaded: false,
  state: null,
  settings: null,
  servers: {},
  pinging: false,
  tested: {},
  traffic: emptyTraffic(),
  toasts: [],
  neighbors: [],
  browserProxy: [],
  catalog: [],
  programNames: {},
  nav: null,
  incoming: null,
};

const running = (st: StateView | null) => !!st && (st.vpn === 'connected' || st.vpn === 'reconnecting' || st.vpn === 'server_down');

function reducer(s: S, a: A): S {
  switch (a.t) {
    case 'service':
      return { ...s, serviceUp: a.up };
    case 'state':
      return { ...s, state: a.state, loaded: true, traffic: running(a.state) ? s.traffic : emptyTraffic(), neighbors: running(a.state) ? s.neighbors : [], browserProxy: running(a.state) ? s.browserProxy : [] };
    case 'settings':
      return { ...s, settings: a.settings };
    case 'servers':
      return { ...s, servers: { ...s.servers, [a.id]: a.list } };
    case 'pinging':
      return { ...s, pinging: a.on };
    case 'tested':
      return { ...s, tested: { ...s.tested, [a.id]: true } };
    case 'traffic': {
      const t = s.traffic;
      const mb = (b: number) => (b * 8) / 1e6;
      return {
        ...s,
        traffic: {
          up: a.up,
          down: a.down,
          upHist: [...t.upHist.slice(1), mb(a.up)],
          downHist: [...t.downHist.slice(1), mb(a.down)],
          upSum: t.upSum + a.up,
          downSum: t.downSum + a.down,
        },
      };
    }
    case 'toast':
      return { ...s, toasts: [...s.toasts.slice(-1), a.toast] };
    case 'dismiss':
      return { ...s, toasts: s.toasts.filter((x) => x.id !== a.id) };
    case 'neighbors':
      return { ...s, neighbors: a.names };
    case 'browserProxy':
      return { ...s, browserProxy: a.names };
    case 'catalog':
      return { ...s, catalog: a.list };
    case 'nav':
      return { ...s, nav: a.target ? { target: a.target, seq: (s.nav?.seq ?? 0) + 1 } : null };
    case 'incoming':
      return { ...s, incoming: a.link };
    case 'names':
      return { ...s, programNames: { ...s.programNames, ...a.names } };
    case 'killswitch':
      return s.settings ? { ...s, settings: { ...s.settings, kill_switch: a.ks } } : s;
    case 'list':
      return s.settings ? { ...s, settings: { ...s.settings, lists: { ...s.settings.lists, [a.position]: a.list } } } : s;
  }
}

export interface Store extends S {
  transport: Transport;
  isRunning: boolean;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  selectConnection(id: string): Promise<boolean>;
  selectServer(name: string): Promise<void>;
  testLatency(): Promise<void>;
  refresh(id: string): Promise<void>;
  remove(id: string): Promise<void>;
  addLink(source: string, name?: string): Promise<boolean>;
  importFile(file: File): Promise<boolean>;
  setMode(mode: Mode): Promise<void>;
  setRouting(routing: Routing): Promise<void>;
  setRussia(key: 'ru_domains' | 'ru_ips', on: boolean): Promise<void>;
  /** Экран обработал переход — забыть о нём. */
  consumeNav(): void;
  /** Экран «Добавить» взял ссылку `klick://add` — забыть о ней. */
  consumeIncoming(): void;
  /** Настройки поведения и оформления: передаются только изменённые поля. */
  setPrefs(p: Preferences): Promise<boolean>;
  ksSet(enabled: boolean): Promise<void>;
  /** Добавить программы в Kill Switch по пути к exe или папке; возвращает, сколько добавилось. */
  ksAdd(paths: string[]): Promise<number>;
  ksRemove(folder: string): Promise<void>;
  ksProgram(folder: string, enabled: boolean): Promise<void>;
  /** Добавить правила в список положения; возвращает, сколько добавилось. */
  addRules(position: Routing, rules: Rule[]): Promise<number>;
  removeRule(position: Routing, index: number): Promise<void>;
  setRuleRoute(position: Routing, index: number, route: Route): Promise<void>;
  /** Тумблер правила: выключенное остаётся в списке. */
  setRuleEnabled(position: Routing, index: number, enabled: boolean): Promise<void>;
  /** Тумблер «через VPN» из «Сейчас в сети», трея и «Не открывается?»: правило в мой список текущего положения. */
  routeTarget(target: Target, vpn: boolean, label: string, quiet?: boolean): Promise<boolean>;
  toast(title: string, text?: string, tone?: Tone, action?: Toast['action']): void;
  dismiss(id: number): void;
  reload(): Promise<void>;
}

const Ctx = createContext<Store | null>(null);

/** «resume» — один раз за жизнь окна. */
let resumed = false;

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error('useStore вне StoreProvider');
  return s;
}

export function StoreProvider({ transport, children }: { transport: Transport; children: ReactNode }) {
  const [s, dispatch] = useReducer(reducer, initial);
  const ref = useRef(s);
  ref.current = s;
  const toastId = useRef(0);

  const toast = useCallback((title: string, text?: string, tone: Tone = 'dim', action?: Toast['action']) => {
    const id = ++toastId.current;
    dispatch({ t: 'toast', toast: { id, title, text, tone, action } });
    setTimeout(() => dispatch({ t: 'dismiss', id }), action ? 5000 : 2800);
  }, []);

  const failed = useCallback(
    (e: unknown) => {
      const err = e as ErrorInfo;
      toast(errorText(err?.code ?? 'unknown', err?.params), undefined, 'bad');
    },
    [toast],
  );

  // id передаём явно: сразу после dispatch ref ещё хранит старые настройки.
  const loadServers = useCallback(async (connId?: string | null) => {
    const id = connId ?? ref.current.settings?.active_connection;
    if (!id) return;
    try {
      const list = await transport.call<ServerView[]>('servers');
      dispatch({ t: 'servers', id, list });
    } catch {
      /* подписка без серверов — список останется пустым */
    }
  }, [transport]);

  const reloadSettings = useCallback(async () => {
    const settings = await transport.call<Settings>('settings').catch(() => null);
    if (settings) dispatch({ t: 'settings', settings });
  }, [transport]);

  const reload = useCallback(async () => {
    try {
      const [state, settings] = await Promise.all([transport.call<StateView>('status'), transport.call<Settings>('settings')]);
      dispatch({ t: 'settings', settings });
      dispatch({ t: 'state', state });
      // После входа в Windows служба сама решит, вернуть ли VPN («Восстанавливать подключение»).
      if (!resumed) {
        resumed = true;
        void transport.call('resume').catch(() => undefined);
      }
      if (!ref.current.catalog.length) {
        const c = await transport.call<{ services: Service[] }>('catalog').catch(() => null);
        if (c) dispatch({ t: 'catalog', list: c.services });
      }
      if (settings.active_connection) {
        const list = await transport.call<ServerView[]>('servers').catch(() => []);
        dispatch({ t: 'servers', id: settings.active_connection, list });
      }
    } catch {
      dispatch({ t: 'service', up: false });
    }
  }, [transport]);

  // Названия программ: у папок правил и Kill Switch, которых ещё не знаем.
  const asked = useRef(new Set<string>());
  useEffect(() => {
    if (!s.settings) return;
    const folders = [
      ...s.settings.kill_switch.programs.map((p) => p.folder),
      ...[...s.settings.lists.all_vpn, ...s.settings.lists.selected].filter((r) => r.target.kind === 'program').map((r) => r.target.value),
    ].filter((f) => !asked.current.has(f));
    if (!folders.length) return;
    folders.forEach((f) => asked.current.add(f));
    void transport
      .call<Record<string, string>>('program_names', { folders })
      .then((names) => dispatch({ t: 'names', names }))
      .catch(() => undefined);
  }, [s.settings, transport]);

  // Тема: из настроек службы, «Системная» — вслед за Windows.
  const appearance = s.settings?.appearance;
  useEffect(() => {
    if (!appearance) return;
    const apply = () => applyTheme(resolveTheme(appearance, systemDark()));
    apply();
    const mq = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null;
    mq?.addEventListener('change', apply);
    return () => mq?.removeEventListener('change', apply);
  }, [appearance]);

  useEffect(() => {
    const offEvent = transport.onEvent((e: KEvent) => {
      if (e.ev === 'state') {
        const prev = ref.current.state;
        dispatch({ t: 'state', state: e.state });
        if (prev && prev.vpn !== 'connected' && e.state.vpn === 'connected' && prev.vpn === 'connecting') {
          toast(`Подключено к ${e.state.connection?.name ?? 'серверу'}`, e.state.server ?? undefined, 'ok');
        }
        if (e.state.connection?.id !== ref.current.settings?.active_connection || prev?.server !== e.state.server) {
          void transport.call<Settings>('settings').then((settings) => {
            dispatch({ t: 'settings', settings });
            void loadServers(settings.active_connection);
          });
        }
      } else if (e.ev === 'servers') {
        // Замерили в другом окне (или в этом): свежие задержки и «нет ответа» — после замера.
        dispatch({ t: 'tested', id: e.connection });
        void loadServers(e.connection);
      } else if (e.ev === 'settings') {
        // Главное окно и окно трея — два отдельных окна: что поменяли в одном, должно быть видно в другом.
        void transport.call<Settings>('settings').then((settings) => dispatch({ t: 'settings', settings })).catch(() => undefined);
      } else if (e.ev === 'traffic') {
        dispatch({ t: 'traffic', up: e.up, down: e.down });
      } else if (e.ev === 'notice') {
        if (e.code === 'neighbors.conflict' && Array.isArray(e.params?.names)) dispatch({ t: 'neighbors', names: e.params.names as string[] });
        if (e.code === 'neighbors.browser_proxy' && Array.isArray(e.params?.names)) dispatch({ t: 'browserProxy', names: e.params.names as string[] });
        const n = noticeText(e.code, e.params);
        if (!n) return;
        // Уведомления Windows показывает только главное окно — и только когда его не видно.
        if (transport.window !== 'main') {
          toast(n.title, n.text, n.tone);
          return;
        }
        const code = e.code;
        void transport.isActive().then((active) => {
          if (active) {
            toast(n.title, n.text, n.tone);
            return;
          }
          const allowed = !DROP_NOTICES.includes(code) || ref.current.settings?.notify !== false;
          if (allowed) void transport.notify(n.title, n.text ?? '', noticeTarget(code)).catch(() => undefined);
        });
      }
    });
    const offNav = transport.onNavigate((target) => dispatch({ t: 'nav', target }));
    const offService = transport.onService((up) => {
      dispatch({ t: 'service', up });
      if (up) void reload();
    });
    void reload();
    return () => {
      offEvent();
      offNav();
      offService();
    };
  }, [transport, reload, loadServers, toast]);

  // Ссылка klick://add со страницы подписки. Окно забирает её само: при запуске по ссылке она пришла
  // раньше, чем страница начала слушать события. Сама ссылка ничего не добавляет — только открывает экран.
  const linkSeq = useRef(0);
  useEffect(() => {
    if (transport.window !== 'main') return;
    const take = async () => {
      const p = await transport.takePendingLink().catch(() => null);
      if (!p) return;
      if (!p.url) {
        toast('Ссылка не подходит', 'Подписка не добавлена', 'bad');
        return;
      }
      // Эта подписка уже есть — открыть её, а не заводить вторую.
      const found = await transport.call<ConnectionView | null>('find_connection', { source: p.url }).catch(() => null);
      if (found) {
        toast('Уже добавлено', found.name, 'dim');
        dispatch({ t: 'nav', target: `conn:${found.id}` });
        return;
      }
      dispatch({ t: 'incoming', link: { url: p.url, name: p.name ?? '', host: p.host ?? '', seq: ++linkSeq.current } });
    };
    const off = transport.onAddLink(() => void take());
    void take();
    return off;
  }, [transport, toast]);

  const call = useCallback(
    async <T,>(cmd: string, args?: unknown): Promise<T | undefined> => {
      try {
        return await transport.call<T>(cmd, args);
      } catch (e) {
        failed(e);
        return undefined;
      }
    },
    [transport, failed],
  );

  const store = useMemo<Store>(
    () => ({
      ...s,
      transport,
      isRunning: running(s.state),
      connect: async () => {
        await call('connect');
      },
      disconnect: async () => {
        await call('disconnect');
      },
      selectConnection: async (id) => {
        const r = await call('select_connection', { id });
        await reload();
        return r !== undefined;
      },
      selectServer: async (name) => {
        const id = ref.current.settings?.active_connection;
        if (id) {
          const list = (ref.current.servers[id] ?? []).map((x) => ({ ...x, selected: x.name === name }));
          dispatch({ t: 'servers', id, list });
        }
        await call('select_server', { name });
      },
      testLatency: async () => {
        const id = ref.current.settings?.active_connection;
        if (!id || ref.current.pinging) return;
        dispatch({ t: 'pinging', on: true });
        const list = await call<ServerView[]>('test_latency');
        if (list) {
          dispatch({ t: 'servers', id, list });
          dispatch({ t: 'tested', id });
        }
        dispatch({ t: 'pinging', on: false });
      },
      refresh: async (id) => {
        const r = await call('refresh_connection', { id });
        if (r !== undefined) {
          toast('Подписка обновлена', 'Список серверов и лимиты актуальны', 'ok');
          await reload();
        }
      },
      remove: async (id) => {
        const name = ref.current.settings?.connections.find((c) => c.id === id)?.name ?? '';
        const r = await call('remove_connection', { id });
        if (r !== undefined) {
          toast('Подключение удалено', name, 'dim');
          await reload();
        }
      },
      addLink: async (source, name) => {
        try {
          await transport.call('add_connection', { source, name: name || null });
        } catch (e) {
          const err = e as ErrorInfo;
          // Эта ссылка уже добавлена: открыть то подключение вместо второго такого же.
          if (err?.code === 'conn.exists' && typeof err.params?.id === 'string') {
            toast('Уже добавлено', typeof err.params.name === 'string' ? err.params.name : undefined, 'dim');
            dispatch({ t: 'nav', target: `conn:${err.params.id}` });
            return true;
          }
          failed(e);
          return false;
        }
        toast('Подключение добавлено', 'Проверьте задержку серверов', 'ok');
        await reload();
        return true;
      },
      importFile: async (file) => {
        const content = await file.text();
        const r = await call('import_file', { file_name: file.name, content });
        if (r === undefined) return false;
        toast('Файл импортирован', file.name, 'ok');
        await reload();
        return true;
      },
      setMode: async (mode) => {
        if (await call('set_mode', { mode }) !== undefined) await reloadSettings();
      },
      setRouting: async (routing) => {
        if (await call('set_routing', { routing }) !== undefined) await reloadSettings();
      },
      setRussia: async (key, on) => {
        const settings = await call<Settings>('set_preferences', { prefs: { [key]: on } });
        if (settings) dispatch({ t: 'settings', settings });
      },
      consumeNav: () => dispatch({ t: 'nav', target: null }),
      consumeIncoming: () => dispatch({ t: 'incoming', link: null }),
      setPrefs: async (prefs) => {
        const settings = await call<Settings>('set_preferences', { prefs });
        if (settings) dispatch({ t: 'settings', settings });
        return settings !== undefined;
      },
      ksSet: async (enabled) => {
        const ks = await call<KillSwitch>('kill_switch_set', { enabled });
        if (!ks) return;
        dispatch({ t: 'killswitch', ks });
        if (!enabled) {
          toast('Kill Switch выключен', 'Программы из списка пойдут напрямую без VPN', 'warn', {
            label: 'Отменить',
            run: () => void call<KillSwitch>('kill_switch_set', { enabled: true }).then((k) => k && dispatch({ t: 'killswitch', ks: k })),
          });
        }
      },
      ksAdd: async (paths) => {
        let added = 0;
        let firstError: unknown;
        for (const folder of paths) {
          try {
            const ks = await transport.call<KillSwitch>('kill_switch_add', { folder });
            dispatch({ t: 'killswitch', ks });
            added++;
          } catch (e) {
            firstError ??= e;
          }
        }
        if (firstError) failed(firstError);
        return added;
      },
      ksRemove: async (folder) => {
        const was = ref.current.settings?.kill_switch.programs.find((p) => p.folder === folder);
        const ks = await call<KillSwitch>('kill_switch_remove', { folder });
        if (!ks) return;
        dispatch({ t: 'killswitch', ks });
        toast('Программа убрана из Kill Switch', undefined, 'dim', {
          label: 'Отменить',
          run: () =>
            void call<KillSwitch>('kill_switch_add', { folder }).then(async (k) => {
              if (!k) return;
              const back = was && !was.enabled ? await call<KillSwitch>('kill_switch_program', { folder, enabled: false }) : k;
              if (back) dispatch({ t: 'killswitch', ks: back });
            }),
        });
      },
      ksProgram: async (folder, enabled) => {
        const ks = await call<KillSwitch>('kill_switch_program', { folder, enabled });
        if (ks) dispatch({ t: 'killswitch', ks });
      },
      addRules: async (position, rules) => {
        let added = 0;
        let firstError: unknown;
        for (const rule of rules) {
          try {
            const list = await transport.call<Rule[]>('list_add', { position, rule });
            dispatch({ t: 'list', position, list });
            added++;
          } catch (e) {
            firstError ??= e;
          }
        }
        if (firstError) failed(firstError);
        return added;
      },
      removeRule: async (position, index) => {
        const rule = ref.current.settings?.lists[position][index];
        const list = await call<Rule[]>('list_remove', { position, index });
        if (!list || !rule) return;
        dispatch({ t: 'list', position, list });
        toast('Правило удалено', undefined, 'dim', {
          label: 'Отменить',
          run: () => {
            void call<Rule[]>('list_add', { position, rule }).then((l) => l && dispatch({ t: 'list', position, list: l }));
          },
        });
      },
      setRuleRoute: async (position, index, route) => {
        const list = await call<Rule[]>('list_set_route', { position, index, route });
        if (list) dispatch({ t: 'list', position, list });
      },
      setRuleEnabled: async (position, index, enabled) => {
        const list = await call<Rule[]>('list_set_enabled', { position, index, enabled });
        if (list) dispatch({ t: 'list', position, list });
      },
      routeTarget: async (target, vpn, label, quiet) => {
        const st = ref.current.settings;
        if (!st) return false;
        const position = st.routing;
        const plan = planToggle(st.lists[position], target, vpn);
        if (plan.op === 'none') return true;
        let list: Rule[] | undefined;
        if (plan.op === 'add') {
          try {
            list = await transport.call<Rule[]>('list_add', { position, rule: plan.rule });
          } catch (e) {
            failed(e);
          }
        } else {
          const was = st.lists[position][plan.index];
          list = was.route === plan.route ? st.lists[position] : await call<Rule[]>('list_set_route', { position, index: plan.index, route: plan.route });
          if (list && list[plan.index]?.enabled === false) list = await call<Rule[]>('list_set_enabled', { position, index: plan.index, enabled: true });
        }
        if (!list) return false;
        dispatch({ t: 'list', position, list });
        if (!quiet) toast(`${label} → ${vpn ? 'через VPN' : 'напрямую'}`, `Правило в «${positionVerb(position)} · мой список»`, 'ok');
        return true;
      },
      toast,
      dismiss: (id) => dispatch({ t: 'dismiss', id }),
      reload,
    }),
    [s, transport, call, reload, reloadSettings, toast, failed],
  );

  return <Ctx.Provider value={store}>{children}</Ctx.Provider>;
}
