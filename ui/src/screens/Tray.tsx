// Окно трея: вид — как у меню трея kl!ck 0.3 (ключ в шапке, кнопка со свечением на точках, чипы
// подключений, плотные строки), функции — из «Трея v2»: доля VPN, «VPN для всего / для выбранного»,
// «Открыто сейчас», трафик подписки, шестерёнка с короткими настройками, вопрос при выходе.
// Окно подгоняет высоту под содержимое и прячется, когда щёлкнули мимо.

import { useEffect, useLayoutEffect, useMemo, useState, type CSSProperties, type ReactNode } from 'react';
import mark from '../assets/mark.png';
import markFace from '../assets/mark-face.png';
import { Sheet, Toasts } from '../components/Chrome';
import { Tile } from '../components/Controls';
import { Icon } from '../components/Icon';
import { buildPath, fmtRate, pingColor, protocolName } from '../lib/format';
import { errorText, routingTitle } from '../lib/i18n';
import { groupLive, positionVerb, programTarget } from '../lib/live';
import { plural } from '../lib/rules';
import { useStore } from '../lib/store';
import type { Transport } from '../lib/transport';
import type { AboutView, Connection, ConnView, ExitAction, ServerView, VpnState } from '../lib/types';
import { useNow } from './Home';

const STATUS: Record<VpnState, [string, string]> = {
  off: ['Отключено', 'var(--dim)'],
  connecting: ['Подключение…', 'var(--orange)'],
  connected: ['Подключено', 'var(--accent)'],
  reconnecting: ['Переподключение…', 'var(--orange)'],
  server_down: ['Сервер не отвечает', 'var(--orange)'],
  error: ['Ошибка', 'var(--red)'],
};

/** Время подключения «00:12:05». */
function clock(since: number | null, now: number): string {
  const s = since ? Math.max(0, Math.floor(now / 1000 - since)) : 0;
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(Math.floor(s / 3600))}:${p(Math.floor(s / 60) % 60)}:${p(s % 60)}`;
}

/** Страны по названию — когда в имени сервера нет флага. */
const COUNTRIES: [RegExp, string][] = [
  [/нидерланд|netherlands|голланд|amsterdam|амстердам/i, 'NL'],
  [/германи|germany|frankfurt|франкфурт/i, 'DE'],
  [/финлян|finland|helsinki|хельсинки/i, 'FI'],
  [/сша|usa|united states|америк|new york|нью-йорк/i, 'US'],
  [/турци|turkey|türkiye|istanbul|стамбул/i, 'TR'],
  [/япони|japan|tokyo|токио/i, 'JP'],
  [/франци|france|paris|париж/i, 'FR'],
  [/великобритан|англи|united kingdom|london|лондон/i, 'GB'],
  [/швеци|sweden|stockholm/i, 'SE'],
  [/польш|poland|warsaw|варшав/i, 'PL'],
  [/казахстан|kazakhstan|алматы|almaty/i, 'KZ'],
  [/латви|latvia|рига|riga/i, 'LV'],
  [/эстони|estonia|таллин|tallinn/i, 'EE'],
  [/сингапур|singapore/i, 'SG'],
  [/гонконг|hong kong/i, 'HK'],
  [/швейцари|switzerland|zurich|цюрих/i, 'CH'],
  [/канад|canada|toronto/i, 'CA'],
  [/росси|russia|москв|moscow/i, 'RU'],
];

/** Код страны из флага в имени сервера: «🇳🇱 Амстердам» → NL; «NL-2 Amsterdam» → NL; «Нидерланды» → NL. */
function countryCode(name: string): string {
  const flag = [...name].filter((ch) => {
    const c = ch.codePointAt(0) ?? 0;
    return c >= 0x1f1e6 && c <= 0x1f1ff;
  });
  if (flag.length >= 2) return flag.slice(0, 2).map((ch) => String.fromCharCode((ch.codePointAt(0) ?? 0) - 0x1f1e6 + 65)).join('');
  const m = /^([A-Z]{2})(?=[\s\-_·|]|\d)/.exec(name.trim());
  if (m) return m[1];
  return COUNTRIES.find(([re]) => re.test(name))?.[1] ?? '';
}

/** Имя сервера без флага в начале. */
const plainName = (name: string) => name.replace(/^[\u{1F1E6}-\u{1F1FF}]{2}\s*/u, '').trim() || name;

const KIND: Record<Connection['kind'], string> = { subscription: 'Подписка', link: 'Прямое', file: 'Файл' };

/** Соединения раз в 3 секунды, пока VPN работает и окно трея на экране. */
function useTrayConns(running: boolean, transport: Transport): ConnView[] {
  const [conns, setConns] = useState<ConnView[]>([]);
  useEffect(() => {
    if (!running) {
      setConns([]);
      return;
    }
    let alive = true;
    const tick = async () => {
      if (document.visibilityState !== 'visible') return;
      const c = await transport.call<ConnView[]>('connections').catch(() => null);
      if (alive && c) setConns(c);
    };
    void tick();
    const t = setInterval(tick, 3000);
    const onShow = () => void tick();
    document.addEventListener('visibilitychange', onShow);
    return () => {
      alive = false;
      clearInterval(t);
      document.removeEventListener('visibilitychange', onShow);
    };
  }, [running, transport]);
  return conns;
}

export function Tray() {
  const store = useStore();
  const { state, settings, servers, pinging, traffic, isRunning, transport, serviceUp, catalog, programNames } = store;
  const [view, setView] = useState<'main' | 'settings'>('main');
  const [exitAsk, setExitAsk] = useState(false);
  const [about, setAbout] = useState<AboutView | null>(null);
  const [note, setNote] = useState('');
  const now = useNow(isRunning);
  const conns = useTrayConns(isRunning, transport);
  // Содержимое естественной высоты: по нему окно трея подгоняет свою высоту.
  const [box, setBox] = useState<HTMLDivElement | null>(null);

  useEffect(() => transport.onExitRequest(() => setExitAsk(true)), [transport]);
  useEffect(() => {
    void transport
      .call<AboutView>('about')
      .then(setAbout)
      .catch(() => undefined);
  }, [transport]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (exitAsk) setExitAsk(false);
      else if (view === 'settings') setView('main');
      else transport.hideTray();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [exitAsk, view, transport]);
  // Окно трея — по высоте содержимого.
  useLayoutEffect(() => {
    const el = box;
    if (!el) return;
    const fit = () => transport.fitTray(Math.ceil(el.offsetHeight));
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [box, transport]);

  const routing = settings?.routing ?? 'selected';
  const list = useMemo(() => settings?.lists[routing] ?? [], [settings, routing]);
  const apps = useMemo(() => groupLive(conns, list, settings?.kill_switch, catalog, programNames), [conns, list, settings?.kill_switch, catalog, programNames]);

  const exitWith = async (action: Exclude<ExitAction, 'ask'>, remember: boolean) => {
    if (remember) await store.setPrefs({ on_exit: action });
    if (action === 'disconnect') {
      await store.disconnect();
      await transport.exit(true);
    } else {
      await transport.exit(false);
    }
  };

  const onExit = () => {
    const vpnOn = isRunning || state?.vpn === 'connecting';
    if (!vpnOn) return void transport.exit(true);
    const remembered = settings?.on_exit ?? 'ask';
    if (remembered === 'ask') setExitAsk(true);
    else void exitWith(remembered, false);
  };

  const foot = <Versions about={about} />;

  if (!state || !settings || !serviceUp) {
    return (
      <Shell inner={setBox} overlay={exitAsk ? <ExitSheet onPick={exitWith} onClose={() => setExitAsk(false)} /> : null}>
        <div className="tr-hero bad">
          <Glow />
          <Header />
          <div className="tr-status">
          <span className="tr-power bad" aria-hidden="true">
            <Icon name="power" size={26} />
          </span>
          <span className="grow">
            <span className="tr-label" style={{ color: 'var(--red)' }}>
              <i />
              Служба не отвечает
            </span>
            <span className="tr-big sm">Нет связи</span>
            <span className="tr-sub">Переустановите kl!ck или перезагрузите компьютер</span>
          </span>
          </div>
        </div>
        <Footer onOpen={() => transport.openMain()} onExit={onExit} />
        {foot}
      </Shell>
    );
  }

  const vpn = state.vpn;
  const conn = settings.connections.find((c) => c.id === settings.active_connection) ?? null;
  const empty = !conn;
  const srv = conn ? servers[conn.id] ?? [] : [];
  const current = srv.find((s) => s.selected) ?? srv.find((s) => s.name === conn?.selected_server) ?? srv[0];
  const on = vpn === 'connected';
  const busy = vpn === 'connecting' || vpn === 'reconnecting';
  const [label, color] = empty ? ['Нет подключений', 'var(--dim)'] : STATUS[vpn];
  const labelFull = vpn === 'reconnecting' && state.attempt ? `${label} ${state.attempt[0]} из ${state.attempt[1]}` : label;
  const bigText = empty ? 'Добавьте сервер' : vpn === 'error' ? 'Нет ответа' : clock(isRunning ? state.since : null, now);
  const sub = empty ? 'Подписка или прямая ссылка' : srv.length > 1 && current ? `${conn.name} · ${plainName(current.name)}` : `${conn.name}${current ? ` · ${protocolName(current.kind)}` : ''}`;
  const ks = settings.kill_switch;
  const ksOn = ks.programs.filter((p) => p.enabled).length;
  const total = apps.reduce((a, x) => a + x.conns, 0);
  const vpnPct = total ? Math.round((apps.reduce((a, x) => a + x.vpnConns, 0) / total) * 100) : null;

  const power = async () => {
    if (empty) return;
    if (isRunning || busy) await store.disconnect();
    else await store.connect();
  };

  const powerClass = on ? 'tr-power on' : busy ? 'tr-power busy' : vpn === 'server_down' ? 'tr-power warn' : vpn === 'error' ? 'tr-power bad' : empty ? 'tr-power empty' : 'tr-power';
  const look = on ? 'ok' : busy ? 'busy' : vpn === 'server_down' ? 'warn' : vpn === 'error' ? 'bad' : '';

  if (view === 'settings') {
    return (
      <Shell inner={setBox} overlay={<Toasts />}>
        <TraySettings onBack={() => setView('main')} />
        {foot}
      </Shell>
    );
  }

  return (
    <Shell
      inner={setBox}
      overlay={
        <>
          <Toasts />
          {exitAsk ? <ExitSheet onPick={exitWith} onClose={() => setExitAsk(false)} /> : null}
        </>
      }
    >
      <div className={`tr-hero ${look}`}>
      <Glow />
      <Header ping={isRunning ? current?.delay ?? null : undefined} onSettings={() => setView('settings')} />

      <div className="tr-status">
        <button className={powerClass} disabled={empty} title={isRunning ? 'Отключить' : busy ? 'Отменить' : 'Подключить'} onClick={() => void power()}>
          <Icon name="power" size={26} />
          {busy ? <span className="tr-spin" /> : null}
        </button>
        <span className="grow">
          <span className="tr-label" style={{ color }}>
            <i />
            {labelFull}
          </span>
          <span className={empty || vpn === 'error' ? 'tr-big sm' : isRunning ? 'tr-big' : 'tr-big dim'}>{bigText}</span>
          <span className="tr-sub">{sub}</span>
        </span>
      </div>
      </div>

      {vpn === 'error' ? (
        <div className="tr-error">
          <span className="grow">
            <span className="t">Не удалось подключиться</span>
            <span className="s">{state.error ? errorText(state.error) : 'Сервер не ответил'}</span>
          </span>
          <button onClick={() => void store.connect()}>Повторить</button>
        </div>
      ) : null}

      {empty ? (
        <PasteCard />
      ) : (
        <>
          {isRunning ? <Traffic down={traffic.down} up={traffic.up} downHist={traffic.downHist} upHist={traffic.upHist} /> : null}
          {isRunning && vpnPct != null ? (
            <div className="tr-share">
              <span className="tr-share-bar">
                <i style={{ width: `${vpnPct}%` }} />
                <em />
              </span>
              <span className="tr-share-key">
                <i className="vpn" />
                VPN {vpnPct}%
              </span>
              <span className="tr-share-key">
                <i />
                напрямую
              </span>
            </div>
          ) : null}
          <div className="tr-seg">
            {(
              [
                ['all_vpn', routingTitle.all_vpn],
                ['selected', routingTitle.selected],
              ] as const
            ).map(([k, t]) => (
              <button key={k} className={routing === k ? 'on' : ''} onClick={() => routing !== k && void store.setRouting(k)}>
                {t}
              </button>
            ))}
          </div>

          {isRunning && apps.length ? (
            <div className="tr-block">
              <div className="tr-caps">
                <span className="grow">Открыто сейчас</span>
                <small>тумблер = через VPN</small>
              </div>
              <div className="tr-card">
                {apps.slice(0, 3).map((a) => (
                  <div key={a.key} className="tr-app">
                    <Tile label={a.name} size={22} />
                    <span className="grow">
                      <span className="t">{a.name}</span>
                      <span className={a.vpn ? 's vpn' : 's'}>{a.why === 'killswitch' ? 'Kill Switch' : a.vpn ? 'через VPN' : 'напрямую'}</span>
                    </span>
                    <MiniToggle
                      on={a.vpn}
                      disabled={a.why === 'killswitch' || !a.path}
                      label={`${a.name} через VPN`}
                      onChange={async (v) => {
                        if (!a.path) return;
                        if (await store.routeTarget(programTarget(a.path), v, a.name, true)) {
                          setNote(`${a.name} → ${v ? 'через VPN' : 'напрямую'}. Правило добавлено в «${positionVerb(routing)} · мой список».`);
                        }
                      }}
                    />
                  </div>
                ))}
              </div>
              {note ? <div className="tr-note">{note}</div> : null}
            </div>
          ) : null}

          {settings.connections.length > 1 ? (
            <div className="tr-block">
              <div className="tr-caps">Подключение</div>
              <div className="tr-chips">
                {settings.connections.map((c) => {
                  const active = c.id === conn.id;
                  return (
                    <button key={c.id} className={active ? 'tr-chip on' : 'tr-chip'} title={profileMeta(c, servers[c.id])} onClick={() => !active && void store.selectConnection(c.id)}>
                      {active && isRunning ? <i /> : null}
                      {c.name}
                    </button>
                  );
                })}
              </div>
            </div>
          ) : null}

          <div className="tr-block">
            <div className="tr-caps">
              <span className="grow">{srv.length > 1 ? `Сервер · ${srv.length}` : 'Сервер'}</span>
              <button className="tr-ping" onClick={() => void store.testLatency()} disabled={pinging}>
                <Icon name="refresh" size={13} className={pinging ? 'spin' : undefined} />
                {pinging ? 'Проверяем…' : 'Задержка'}
              </button>
            </div>
            {srv.length > 1 ? (
              <>
                {conn.info ? <Usage info={conn.info} /> : null}
                <div className="tr-card tr-servers">
                  {srv.map((s) => (
                    <ServerRow key={s.name} s={s} active={s.name === current?.name} pinging={pinging} onPick={() => s.name !== current?.name && void store.selectServer(s.name)} />
                  ))}
                </div>
              </>
            ) : (
              <div className="tr-card tr-direct">
                <span className="tr-radio on">
                  <i />
                </span>
                <span className="grow">
                  <span className="t">{current ? plainName(current.name) : conn.name}</span>
                  <span className="s mono">{current ? protocolName(current.kind) : 'список появится после обновления'}</span>
                </span>
                <span className="tr-ms" style={{ color: pinging ? 'var(--dim)' : pingColor(current?.delay) }}>
                  {pinging ? '…' : current?.delay != null ? `${current.delay} мс` : 'нет ответа'}
                </span>
              </div>
            )}
          </div>

          <div className="tr-ks">
            <span className="tr-ks-ico" style={{ color: ks.enabled ? 'var(--accent)' : 'var(--dim)' }}>
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true">
                <path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z" />
              </svg>
            </span>
            <span className="grow">
              <span className="t">Kill Switch</span>
              <span className="s">{ks.enabled ? (ksOn ? `Включён · ${plural(ksOn, ['программа', 'программы', 'программ'])}` : 'Включён · список пуст') : 'Выключен'}</span>
            </span>
            <MiniToggle on={ks.enabled} label="Kill Switch" big onChange={(v) => void store.ksSet(v)} />
          </div>
        </>
      )}

      <Footer onOpen={() => transport.openMain()} onExit={onExit} />
      {foot}
    </Shell>
  );
}

/** Окно трея: прокрутка снаружи, внутри — содержимое естественной высоты. */
function Shell({ inner, overlay, children }: { inner: (el: HTMLDivElement | null) => void; overlay?: ReactNode; children: ReactNode }) {
  return (
    <div className="tray">
      <div className="tray-in" ref={inner}>
        {children}
      </div>
      {overlay}
    </div>
  );
}

function profileMeta(c: Connection, list: ServerView[] | undefined): string {
  const kind = KIND[c.kind];
  if (c.kind === 'subscription') {
    const n = list?.length ?? 0;
    const until = c.info?.expire ? ` · до ${new Date(c.info.expire * 1000).toLocaleDateString('ru-RU', { day: '2-digit', month: '2-digit' })}` : '';
    return `${kind}${n ? ` · ${plural(n, ['сервер', 'сервера', 'серверов'])}` : ''}${until}`;
  }
  const first = list?.[0];
  return `${kind}${first ? ` · ${protocolName(first.kind)}` : ''}`;
}

function Header({ ping, onSettings }: { ping?: number | null; onSettings?: () => void }) {
  return (
    <div className="tr-head">
      <span className="tr-key" aria-hidden="true">
        <span style={{ '--m': `url("${markFace}")` } as CSSProperties} />
        <span style={{ '--m': `url("${mark}")` } as CSSProperties} />
      </span>
      <span className="tr-name">
        kl<b>!</b>ck
      </span>
      {ping !== undefined ? (
        <span className="tr-pingpill">
          <i style={{ background: pingColor(ping) }} />
          {ping == null ? '—' : `${ping} мс`}
        </span>
      ) : null}
      {onSettings ? (
        <button className="tr-gear" title="Настройки" aria-label="Настройки" onClick={onSettings}>
          <Icon name="settings" size={18} />
        </button>
      ) : null}
    </div>
  );
}

/** Точки и свечение под кнопкой: цвет — по состоянию блока. */
function Glow() {
  return (
    <>
      <span className="tr-dots" aria-hidden="true" />
      <span className="tr-glow" aria-hidden="true" />
    </>
  );
}

function MiniToggle({ on, onChange, label, disabled, big }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean; big?: boolean }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} disabled={disabled} className={`tr-toggle${on ? ' on' : ''}${big ? ' big' : ''}`} onClick={() => onChange(!on)}>
      <i />
    </button>
  );
}

/** Скорость — только пока VPN работает: нули при выключенном VPN ничего не говорят. */
function Traffic({ down, up, downHist, upHist }: { down: number; up: number; downHist: number[]; upHist: number[] }) {
  const d = buildPath(downHist.slice(-32), Math.max(1.2, ...downHist.slice(-32)) * 1.1);
  const u = buildPath(upHist.slice(-32), Math.max(3, ...upHist.slice(-32)) * 1.1);
  const split = (bytes: number) => {
    const [v, unit] = fmtRate(bytes).split(' ');
    return { v, unit };
  };
  const rd = split(down);
  const wr = split(up);
  return (
    <div className="tr-traffic">
      <div className="tr-rate">
        <span className="k">
          <Icon name="download" size={13} />
          Чтение
        </span>
        <span className="v">
          {rd.v} <small>{rd.unit}</small>
        </span>
        <svg viewBox="0 0 300 90" preserveAspectRatio="none" aria-hidden="true">
          <path d={d.area} fill="color-mix(in srgb, var(--accent) 12%, transparent)" />
          <path d={d.line} fill="none" stroke="var(--accent)" strokeWidth="1.4" vectorEffect="non-scaling-stroke" />
        </svg>
      </div>
      <div className="tr-rate">
        <span className="k">
          <Icon name="upload" size={13} />
          Загрузка
        </span>
        <span className="v">
          {wr.v} <small>{wr.unit}</small>
        </span>
        <svg viewBox="0 0 300 90" preserveAspectRatio="none" aria-hidden="true">
          <path d={u.area} fill="color-mix(in srgb, var(--dim) 10%, transparent)" />
          <path d={u.line} fill="none" stroke="var(--dim2)" strokeWidth="1.4" vectorEffect="non-scaling-stroke" />
        </svg>
      </div>
    </div>
  );
}

function Usage({ info }: { info: NonNullable<Connection['info']> }) {
  const GB = 1024 ** 3;
  const used = (info.upload + info.download) / GB;
  const pct = info.total ? Math.min(100, ((info.upload + info.download) / info.total) * 100) : 0;
  const until = info.expire ? ` · до ${new Date(info.expire * 1000).toLocaleDateString('ru-RU', { day: '2-digit', month: '2-digit' })}` : '';
  const text = info.total ? `${used.toFixed(1)} / ${Math.round(info.total / GB)} ГБ${until}` : `${used.toFixed(1)} ГБ · без лимита${until}`;
  return (
    <div className="tr-usage">
      <span className="bar">
        <i style={{ width: `${pct}%` }} />
      </span>
      <span className="num">{text}</span>
    </div>
  );
}

function ServerRow({ s, active, pinging, onPick }: { s: ServerView; active: boolean; pinging: boolean; onPick: () => void }) {
  const code = countryCode(s.name);
  return (
    <button className={active ? 'tr-server on' : 'tr-server'} onClick={onPick}>
      <span className={active ? 'tr-radio on' : 'tr-radio'}>
        <i />
      </span>
      {code ? <span className="tr-code">{code}</span> : null}
      <span className="tr-server-name">{plainName(s.name)}</span>
      <span className="tr-ms" style={{ color: pinging ? 'var(--dim)' : pingColor(s.delay) }}>
        {pinging ? '…' : s.delay == null ? 'нет ответа' : `${s.delay} мс`}
      </span>
    </button>
  );
}

function PasteCard() {
  const store = useStore();
  const [busy, setBusy] = useState(false);
  const paste = async () => {
    setBusy(true);
    const text = (await store.transport.clipboardText().catch(() => '')).trim();
    if (!/^(https?|vless|vmess|trojan|ss|ssr|hysteria2?|hy2|tuic|anytls|wireguard):\/\//i.test(text)) {
      store.toast('В буфере нет ссылки', 'Скопируйте ссылку на подписку или конфигурацию', 'warn');
    } else {
      await store.addLink(text);
    }
    setBusy(false);
  };
  return (
    <div className="tr-empty">
      <div className="tr-empty-text">
        Вставьте ссылку на подписку или конфигурацию — <code>https://</code>, <code>vless://</code>, <code>trojan://</code>.
      </div>
      <button className="tr-paste" disabled={busy} onClick={() => void paste()}>
        <Icon name="plus" size={16} />
        {busy ? 'Добавляю…' : 'Вставить из буфера'}
      </button>
    </div>
  );
}

function Footer({ onOpen, onExit }: { onOpen: () => void; onExit: () => void }) {
  return (
    <div className="tr-foot">
      <button className="tr-open" onClick={onOpen}>
        <Icon name="home" size={16} />
        Открыть kl!ck
      </button>
      <button className="tr-exit" title="Выйти из kl!ck" aria-label="Выйти из kl!ck" onClick={onExit}>
        <Icon name="winClose" size={16} />
      </button>
    </div>
  );
}

function Versions({ about }: { about: AboutView | null }) {
  return (
    <div className="tr-ver">
      <span>kl!ck {about?.version ?? '…'}</span>
      <span>mihomo {about?.core_version ?? '…'}</span>
    </div>
  );
}

function TraySettings({ onBack }: { onBack: () => void }) {
  const store = useStore();
  const { settings, transport } = store;
  const [autostart, setAutostart] = useState<boolean | null>(null);
  useEffect(() => {
    void transport.autostart
      .get()
      .then(setAutostart)
      .catch(() => setAutostart(false));
  }, [transport]);
  if (!settings) return null;
  const mode = settings.mode;
  const rows: [string, string, boolean, (v: boolean) => void][] = [
    ['Kill Switch', 'Программы из списка — только VPN', settings.kill_switch.enabled, (v) => void store.ksSet(v)],
    [
      'Запуск с системой',
      'Свёрнутым в трей',
      !!autostart,
      (v) => {
        setAutostart(v);
        void transport.autostart.set(v).catch(() => setAutostart(!v));
      },
    ],
    ['Обновлять подписки', 'Каждые 12 часов', settings.auto_update, (v) => void store.setPrefs({ auto_update: v })],
    ['Уведомления', 'О разрыве и переподключении', settings.notify, (v) => void store.setPrefs({ notify: v })],
  ];
  return (
    <>
      <div className="tr-sethead">
        <button className="tr-back" title="Назад" aria-label="Назад" onClick={onBack}>
          <Icon name="chevron" size={14} className="flip" />
        </button>
        <span>Настройки</span>
      </div>
      <div className="tr-caps">Режим</div>
      <div className="tr-seg">
        {(
          [
            ['tun', 'VPN (TUN)'],
            ['sys_proxy', 'Системный прокси'],
          ] as const
        ).map(([k, t]) => (
          <button key={k} className={mode === k ? 'on' : ''} onClick={() => mode !== k && void store.setMode(k)}>
            {t}
          </button>
        ))}
      </div>
      <div className="tr-desc">{mode === 'tun' ? 'Все программы, включая игры и UDP.' : 'Браузеры и программы, которые используют системный прокси.'}</div>
      <div className="tr-card">
        {rows.map(([title, desc, on, set]) => (
          <div key={title} className="tr-setrow">
            <span className="grow">
              <span className="t">{title}</span>
              <span className="s">{desc}</span>
            </span>
            <MiniToggle big on={on} label={title} onChange={set} />
          </div>
        ))}
      </div>
      <button className="tr-all" onClick={() => transport.openMain('settings')}>
        Все настройки в kl!ck
      </button>
    </>
  );
}

function ExitSheet({ onPick, onClose }: { onPick: (a: Exclude<ExitAction, 'ask'>, remember: boolean) => Promise<void>; onClose: () => void }) {
  const [remember, setRemember] = useState(false);
  const [busy, setBusy] = useState(false);
  const pick = async (a: Exclude<ExitAction, 'ask'>) => {
    setBusy(true);
    await onPick(a, remember);
    setBusy(false);
  };
  return (
    <Sheet onClose={onClose}>
      <h3>Выйти из kl!ck?</h3>
      <p>VPN сейчас включён. Служба может держать его и без окна — тогда вернуться к kl!ck можно из меню «Пуск».</p>
      <div className="exit-choices">
        <button className="exit-choice main" disabled={busy} onClick={() => void pick('disconnect')}>
          Отключить VPN и выйти
        </button>
        <button className="exit-choice" disabled={busy} onClick={() => void pick('keep')}>
          Оставить VPN работать
        </button>
      </div>
      <button className={remember ? 'check-row on' : 'check-row'} onClick={() => setRemember(!remember)}>
        <span className="check">
          <i />
        </span>
        Запомнить выбор
      </button>
      <div className="sheet-actions" style={{ marginTop: 12 }}>
        <button onClick={onClose}>Отмена</button>
      </div>
    </Sheet>
  );
}
