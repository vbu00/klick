// Главная по прототипу Димы: скорость и «+» сверху, статус, кнопка, режим, подписки, карточка с серверами.
// Скорость подробно — листом по нажатию на плашку.

import { useEffect, useState, type CSSProperties } from 'react';
import { ProbeMark } from '../components/Controls';
import { Flag, FlagName } from '../components/Flag';
import { Icon } from '../components/Icon';
import { ServerCountries } from '../components/ServerCountries';
import { Sheet, type Tab } from '../components/Chrome';
import { SwitchDiagram } from '../components/SwitchDiagram';
import { buildPath, daysLeft, fmtBytes, fmtDate, fmtMbps, fmtRate, fmtTimer, fmtTraffic, fmtUsed, pingColor, pingText, protocolName } from '../lib/format';
import { errorText, modeName, routingName } from '../lib/i18n';
import { countryName, parseServer } from '../lib/countries';
import { isIos, isMac } from '../lib/platform';
import { probeMark } from '../lib/probe';
import { plural } from '../lib/rules';
import { useStore, useTraffic } from '../lib/store';
import type { Connection, ErrorInfo, ServerView, SpeedView } from '../lib/types';

export function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [active]);
  return now;
}

/** Время подключения: тикает само раз в секунду и не перерисовывает весь экран. */
export function Elapsed({ since, format }: { since: number | null; format: (since: number | null, now: number) => string }) {
  const now = useNow(since != null);
  return <>{format(since, now)}</>;
}

export function Home({ onTab }: { onTab: (t: Tab) => void }) {
  const store = useStore();
  const { state, settings, servers, pinging, isRunning, pending, neighbors, browserProxy } = store;
  const [viewId, setViewId] = useState<string | null>(null);
  const [switchTo, setSwitchTo] = useState<string | null>(null);
  /** Сервер, выбранный в другой подписке под VPN: поставить его после переключения. */
  const [switchServer, setSwitchServer] = useState<string | null>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const [speedOpen, setSpeedOpen] = useState(false);
  const [browserHelp, setBrowserHelp] = useState(false);
  const { nav, consumeNav } = store;

  // Нажали на уведомление: раскрыть серверы или подсказку про соседей.
  // `conn:<id>` — показать это подключение, как если бы его выбрали в списке («Уже добавлено»).
  useEffect(() => {
    if (!nav) return;
    if (nav.target.startsWith('conn:')) {
      const id = nav.target.slice('conn:'.length);
      setViewId(id);
      const st = store.settings;
      if (store.state?.vpn === 'off' && st && st.active_connection !== id && st.connections.some((c) => c.id === id)) void store.selectConnection(id);
    } else if (nav.target === 'servers') document.querySelector('.b-srvs')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    else if (nav.target === 'neighbors') setHelpOpen(true);
    else if (nav.target === 'browser_proxy') setBrowserHelp(true);
    else if (nav.target !== 'card') return;
    consumeNav();
  }, [nav, consumeNav]);

  // Серверы видны всегда: чужую подписку подгружаем, в активной один раз меряем задержку.
  const viewKey = viewId ?? settings?.active_connection ?? null;
  useEffect(() => {
    if (!viewKey || !settings) return;
    if (viewKey !== settings.active_connection) {
      if (!servers[viewKey]) void store.viewServers(viewKey);
      return;
    }
    const l = servers[viewKey] ?? [];
    if (l.length > 1 && l.every((s) => s.delay == null) && !store.tested[viewKey]) void store.testLatency();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewKey, settings?.active_connection, servers[viewKey ?? '']?.length]);

  if (!state || !settings) return null;
  const conns = settings.connections;
  if (conns.length === 0) return <Empty onAdd={() => onTab('add')} />;

  const activeId = settings.active_connection;
  const liveId = isRunning ? activeId : null;
  const view = conns.find((c) => c.id === (viewId ?? activeId)) ?? conns[0];
  const isActiveView = view.id === activeId;
  const viewingLive = isRunning && view.id === liveId;
  const list: ServerView[] = servers[view.id] ?? [];
  // Пока сервер не выбран, ядро при подключении берёт первый — его и показываем выбранным.
  const current = list.find((s) => s.selected) ?? list.find((s) => s.name === view.selected_server) ?? list[0];

  const vpn = state.vpn;
  const statusLabel =
    vpn === 'connecting' || pending === 'on'
      ? 'подключаюсь…'
      : pending === 'off'
        ? 'отключаюсь…'
        : vpn === 'reconnecting'
          ? `переподключаюсь… ${state.attempt ? `${state.attempt[0]} из ${state.attempt[1]}` : ''}`
          : vpn === 'server_down'
            ? 'сервер не отвечает'
            : vpn === 'error'
              ? 'VPN не включился'
              : !isRunning
                ? 'не подключено'
                : viewingLive
                  ? 'время подключения'
                  : 'нажмите, чтобы переключиться сюда';
  const statusTone = pending ? '' : vpn === 'reconnecting' || vpn === 'server_down' ? 'warn' : vpn === 'error' ? 'bad' : '';

  let core = '';
  if (vpn === 'connecting' || pending) core = 'busy';
  else if (vpn === 'reconnecting') core = viewingLive ? 'busy warn' : 'other';
  else if (vpn === 'server_down') core = viewingLive ? 'warn' : 'other';
  else if (vpn === 'error') core = 'bad';
  else if (isRunning) core = viewingLive ? 'live' : 'other';

  const dot = vpn === 'error' ? 'var(--red)' : vpn === 'reconnecting' || vpn === 'server_down' ? 'var(--orange)' : isRunning ? 'var(--accent)' : 'var(--dim)';

  const onPower = async () => {
    // Второе нажатие, пока служба не ответила на первое, — не команда.
    if (vpn === 'connecting' || pending) return;
    if (isRunning) {
      if (!viewingLive) {
        setSwitchTo(view.id);
        return;
      }
      await store.disconnect();
      return;
    }
    if (!isActiveView && !(await store.selectConnection(view.id))) return;
    await store.connect();
  };

  // Нажатие на подписку только показывает её: активной она станет, когда в ней включат VPN или выберут сервер.
  const pickConnection = (c: Connection) => setViewId(c.id);

  // Сервер в подписке, которая сейчас не активна: это и есть переключение на неё. Под VPN — с вопросом.
  const pickOtherServer = async (name: string) => {
    if (isRunning) {
      setSwitchServer(name);
      setSwitchTo(view.id);
      return;
    }
    if (await store.selectConnection(view.id)) await store.selectServer(name);
  };


  return (
    <div className="screen b-home">
      <div className="b-top">
        <SpeedPill onOpen={() => setSpeedOpen(true)} />
        <button className="b-round" aria-label="Добавить подключение" title="Добавить подключение" onClick={() => onTab('add')}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 5v14M5 12h14" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
        </button>
      </div>

      <div className="home-status">
        <div className={`status-label ${statusTone}`}>{statusLabel}</div>
        <div className="timer">
          <Elapsed since={isRunning ? state.since : null} format={fmtTimer} />
        </div>
      </div>

      <div className="power-wrap">
        {viewingLive && vpn === 'connected' && !pending ? (
          <div className="wave">
            <div className="dots" />
            <div className="lit" />
            <div className="ringfx" />
          </div>
        ) : null}
        <button className="power-btn" onClick={onPower} aria-label={isRunning ? 'Отключить VPN' : 'Включить VPN'} aria-busy={!!pending || vpn === 'connecting'}>
          <span className={`power-core ${core}`}>
            <Icon name="power" size={34} />
          </span>
        </button>
      </div>

      <div className="chip-row">
        <button className="mode-chip" onClick={() => onTab('connection')}>
          <span className="dot7" style={{ background: dot }} />
          {routingName[state.routing]}
        </button>
      </div>

      {vpn === 'reconnecting' ? (
        <div className="banner">
          <span className="dot7" />
          <div>
            Переподключаюсь… {state.attempt ? `${state.attempt[0]} из ${state.attempt[1]}` : ''}
            <small>Сервер перестал отвечать, пробую снова</small>
          </div>
        </div>
      ) : vpn === 'server_down' ? (
        <div className="banner">
          <span className="dot7" />
          <div>
            Сервер не отвечает
            <small>Проверяю раз в минуту. Можно выбрать другой сервер ниже</small>
          </div>
        </div>
      ) : vpn === 'error' ? (
        <div className="banner bad">
          <span className="dot7" />
          <div>
            VPN не включился
            <small>{errorText(state.error ?? '')}</small>
          </div>
        </div>
      ) : !isIos && neighbors.length && state.mode === 'tun' ? (
        <button className="banner" onClick={() => setHelpOpen(true)} style={{ cursor: 'pointer' }}>
          <span className="dot7" />
          <div>
            {neighbors.join(', ')} мешает режиму VPN
            <small>Нажмите, чтобы узнать, что сделать</small>
          </div>
        </button>
      ) : !isIos && browserProxy.length && state.mode === 'sys_proxy' ? (
        <button className="banner" onClick={() => setBrowserHelp(true)} style={{ cursor: 'pointer' }}>
          <span className="dot7" />
          <div>
            Браузер идёт мимо kl!ck
            <small>Прокси перехватило расширение: {browserProxy.join(', ')}</small>
          </div>
        </button>
      ) : null}

      {conns.length > 1 ? (
        <div className="pchips">
          {conns.map((c) => (
            <button key={c.id} className={c.id === view.id ? 'pchip on' : 'pchip'} onClick={() => pickConnection(c)}>
              {isRunning && c.id === liveId ? <span className="live" /> : null}
              <span>
                <FlagName name={c.name} />
              </span>
            </button>
          ))}
        </div>
      ) : null}

      <SubCard
        conn={view}
        style={{ marginTop: conns.length > 1 ? 12 : 30 }}
        viewingLive={viewingLive}
        isActive={isActiveView}
        list={list}
        current={current}
        pinging={pinging && isActiveView}
        onPickOther={(name) => void pickOtherServer(name)}
      />

      {speedOpen ? <SpeedSheet onClose={() => setSpeedOpen(false)} /> : null}

      {switchTo ? (
        <Sheet
          onClose={() => {
            setSwitchTo(null);
            setSwitchServer(null);
          }}
        >
          <h3>Сменить подключение?</h3>
          <p>
            Сейчас работает <b>{conns.find((c) => c.id === liveId)?.name}</b>. Текущее соединение разорвётся на пару секунд, и трафик пойдёт через{' '}
            <b>{conns.find((c) => c.id === switchTo)?.name}</b>.
          </p>
          <SwitchDiagram liveName={conns.find((c) => c.id === liveId)?.name ?? ''} targetName={conns.find((c) => c.id === switchTo)?.name ?? ''} />
          <div className="sheet-actions" style={{ marginTop: 16 }}>
            <button
              onClick={() => {
                setSwitchTo(null);
                setSwitchServer(null);
              }}
            >
              Отмена
            </button>
            <button
              className="accent"
              onClick={() => {
                const [id, server] = [switchTo, switchServer];
                setSwitchTo(null);
                setSwitchServer(null);
                void store.selectConnection(id).then((ok) => (ok && server ? store.selectServer(server) : undefined));
              }}
            >
              Переключить
            </button>
          </div>
        </Sheet>
      ) : null}

      {browserHelp ? (
        <Sheet onClose={() => setBrowserHelp(false)}>
          <h3>Браузер идёт мимо kl!ck</h3>
          <p>
            Расширение {browserProxy.join(', ')} управляет прокси браузера. Пока оно установлено и включено, браузер берёт прокси у него, а не системный прокси kl!ck, — и сайты открываются
            мимо VPN или через серверы расширения.
          </p>
          <p>Выключите или удалите это расширение на странице расширений браузера. Или переключитесь на режим VPN (TUN): он ловит трафик всех программ, но браузер всё равно может уходить через серверы расширения.</p>
          <div className="sheet-actions">
            <button onClick={() => setBrowserHelp(false)}>Понятно</button>
            <button
              className="main"
              onClick={() => {
                setBrowserHelp(false);
                void store.transport.call('set_mode', { mode: 'tun' }).catch(() => undefined);
              }}
            >
              VPN (TUN)
            </button>
          </div>
        </Sheet>
      ) : null}

      {helpOpen ? (
        <Sheet onClose={() => setHelpOpen(false)}>
          <h3>{neighbors.join(', ')} мешает режиму VPN</h3>
          <p>Он перехватывает трафик на всех сетевых адаптерах, включая адаптер kl!ck, поэтому часть сайтов через VPN может не открываться.</p>
          {isMac ? (
            <p>Выключите его, пока работает kl!ck: два VPN сразу мешают друг другу. Обход блокировок (zapret, SpoofDPI) можно оставить, если переключиться на системный прокси.</p>
          ) : (
            <p>Остановите обход в Klutz или zapret, пока включён VPN, или переключитесь на системный прокси — с ним zapret уживается.</p>
          )}
          <div className="sheet-actions">
            <button onClick={() => setHelpOpen(false)}>Понятно</button>
            <button
              className="main"
              onClick={() => {
                setHelpOpen(false);
                void store.transport.call('set_mode', { mode: 'sys_proxy' }).catch(() => undefined);
              }}
            >
              Системный прокси
            </button>
          </div>
        </Sheet>
      ) : null}
    </div>
  );
}

/** Скорость в Мбит/с одним числом: «48.2 Мб/с», «512 Кб/с». */
function rate(bytesPerSec: number): string {
  const mbit = (bytesPerSec * 8) / 1e6;
  if (mbit < 1) return `${Math.round(mbit * 1000)} Кб/с`;
  return `${mbit < 100 ? mbit.toFixed(1) : Math.round(mbit)} Мб/с`;
}

/** Объём как в прототипе: «64,8 ГБ», «512 МБ». */
function gb(bytes: number): string {
  // Двоичные, как у панелей подписок и в остальном kl!ck: 200 ГБ лимита — это 200 · 2³⁰ байт.
  if (bytes < 2 ** 30) return `${Math.round(bytes / 2 ** 20)} МБ`;
  const v = bytes / 2 ** 30;
  return `${(v < 100 ? v.toFixed(1) : Math.round(v).toString()).replace('.', ',')} ГБ`;
}

/** Пульс: линия кардиограммы, при наведении пробегает по себе. */
function Pulse() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path className="ecg" d="M2 12h4.2l2.3-5.5 4.6 11 2.4-5.5H22" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" pathLength={100} />
    </svg>
  );
}

/** Карточка подписки: флаг текущего сервера, название, действия; срок и остаток; серверы видны всегда. */
function SubCard(props: {
  conn: Connection;
  style: CSSProperties;
  viewingLive: boolean;
  isActive: boolean;
  list: ServerView[];
  current: ServerView | undefined;
  pinging: boolean;
  /** Выбрать сервер в подписке, которая сейчас не активна. */
  onPickOther: (name: string) => void;
}) {
  const store = useStore();
  // В активной подписке — сменить сервер; в другой — перейти в неё (решает Home).
  const pick = (name: string) => (props.isActive ? void store.selectServer(name) : props.onPickOther(name));
  const { conn, list, current, pinging } = props;
  const [menuOpen, setMenuOpen] = useState(false);
  const [armed, setArmed] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const isSub = conn.kind === 'subscription';
  const info = conn.info;
  const used = info ? info.upload + info.download : 0;
  const pct = info && info.total > 0 ? Math.min(100, (used / info.total) * 100) : 0;
  // Без лимита панель всё равно сообщает, сколько потрачено (total = 0) — показываем без полосы.
  const unlimited = !!info && info.total === 0 && used > 0;
  const days = info?.expire ? daysLeft(info.expire) : null;
  const announce = info?.announce;
  // Ссылки панели; служба уже проверила (только https).
  const links = [
    { label: 'Поддержка', url: info?.support_url },
    { label: 'Страница подписки', url: info?.page_url },
  ].filter((l): l is { label: string; url: string } => !!l.url);
  // «Серверы по странам»: если ни одной страны не узнали, делить нечего — обычный список.
  const countries = new Set(list.map((s) => parseServer(s.name, s.kind).code).filter(Boolean)).size;
  const byCountry = !!store.settings?.servers_by_country && countries > 0;
  const curP = current ? parseServer(current.name, current.kind) : null;
  // Задержку в шапке показываем, только когда её измерили: null до проверки не значит «нет ответа».
  const ping = pinging ? '…' : current && current.delay != null ? pingText(current.delay) : '';
  const subtitle = current
    ? curP?.code
      ? `${countryName(curP.code)} · ${curP.title || protocolName(current.kind)}`
      : current.name
    : conn.selected_server ?? (isSub ? 'Подписка' : 'Одиночная конфигурация');

  const refresh = async () => {
    setRefreshing(true);
    await store.refresh(conn.id);
    setRefreshing(false);
  };

  const closeMenu = () => {
    setMenuOpen(false);
    setArmed(false);
  };

  return (
    <div className="card-wrap" style={props.style}>
      <div className="b-card">
        <div
          className="b-head"
          onContextMenu={(e) => {
            e.preventDefault();
            setMenuOpen(true);
          }}
        >
          <span className="b-flag">{curP?.code ? <Flag code={curP.code} /> : <Icon name={isSub ? 'globe' : 'rules'} size={22} />}</span>
          <span className="b-head-text">
            <span className="b-title">
              <span className="b-title-name">
                <FlagName name={conn.name} />
              </span>
            </span>
            <span className="b-sub">
              <span className="b-sub-name">{subtitle}</span>
              {!pinging && probeMark(current?.probe) ? (
                <ProbeMark probe={current?.probe} />
              ) : ping ? (
                <span className="num" style={{ color: pinging ? 'var(--dim)' : pingColor(current?.delay) }}>
                  {ping}
                </span>
              ) : null}
            </span>
          </span>
          <span className="b-acts">
            {props.isActive && list.length ? (
              <button className={pinging ? 'b-act pl busy' : 'b-act pl'} aria-label="Проверить задержку" title="Проверить задержку" disabled={pinging} onClick={() => void store.testLatency()}>
                <Pulse />
              </button>
            ) : null}
            {isSub ? (
              <button className="b-act rf" aria-label="Обновить подписку" title="Обновить подписку" disabled={refreshing} onClick={() => void refresh()}>
                <Icon name="refresh" size={16} className={refreshing ? 'spin' : undefined} />
              </button>
            ) : null}
            <button className={menuOpen ? 'b-act open' : 'b-act'} aria-label="Ещё" title="Ещё" onClick={() => setMenuOpen(!menuOpen)}>
              <Icon name="dots" size={18} />
            </button>
          </span>
        </div>

        {isSub ? (
          <div className="b-meta">
            <div className="b-kv">
              <span>{info?.expire ? `Действует до ${fmtDate(info.expire)}` : info && (info.total > 0 || unlimited) ? 'Без срока' : 'Срок и трафик панель не сообщает'}</span>
              {days != null ? <b className={days <= 3 ? 'warn' : undefined}>{days === 0 ? 'истекает сегодня' : plural(days, ['день', 'дня', 'дней'])}</b> : null}
            </div>
            {info && info.total > 0 ? (
              <>
                <div className="b-kv">
                  <span>
                    Осталось <b>{gb(Math.max(0, info.total - used))}</b>
                  </span>
                  <span>из {gb(info.total)}</span>
                </div>
                <div className="b-bar">
                  <i style={{ width: `${100 - pct}%`, background: pct > 90 ? 'var(--orange)' : 'var(--accent)' }} />
                </div>
              </>
            ) : unlimited ? (
              <div className="b-kv">
                <span>
                  Потрачено <b>{gb(used)}</b>
                </span>
                <span>без лимита</span>
              </div>
            ) : null}
            {announce ? <div className="b-announce">{announce}</div> : null}
          </div>
        ) : null}

        <div className="b-srvs">
          {!props.isActive && list.length ? <div className="b-note">Выберите сервер — kl!ck перейдёт на это подключение.</div> : null}
          {list.length === 0 ? (
            <div className="b-note">{props.isActive || store.servers[conn.id] ? 'Серверов пока нет. Обновите подписку — список придёт от панели.' : 'Загружаю список серверов…'}</div>
          ) : byCountry ? (
            <ServerCountries list={list} current={current} pinging={pinging} tested={!!store.tested[conn.id]} onPick={pick} />
          ) : (
            list.map((s) => {
              const on = s.name === current?.name;
              return (
                <button key={s.name} className={on ? 'b-srv on' : 'b-srv'} onClick={() => (props.isActive ? !on : true) && pick(s.name)}>
                  <span className="b-radio">
                    <i />
                  </span>
                  <span className="b-srv-text">
                    <span className="b-srv-name">
                      <FlagName name={s.name} />
                    </span>
                    <span className="b-srv-proto">
                      {protocolName(s.kind)}
                      {s.speed != null ? ` · ${fmtMbps(s.speed)}` : ''}
                    </span>
                  </span>
                  <ProbeMark probe={s.probe} />
                  {/* Отметка проверки точнее «нет ответа» — тогда показываем только её. */}
                  {!pinging && s.delay == null && probeMark(s.probe) ? null : pinging ? (
                    <span className="skel" />
                  ) : (
                    <span className="num b-ping" style={{ color: pingColor(s.delay) }}>
                      {pingText(s.delay)}
                    </span>
                  )}
                </button>
              );
            })
          )}
        </div>
      </div>

      {menuOpen ? (
        <>
          <div className="menu-shade" onClick={closeMenu} onContextMenu={(e) => (e.preventDefault(), closeMenu())} />
          <div className="menu" role="menu">
            {links.map((l) => (
              <button
                key={l.label}
                onClick={() => {
                  closeMenu();
                  void store.transport.openUrl(l.url);
                }}
              >
                <Icon name="globe" />
                <span>{l.label}</span>
              </button>
            ))}
            {links.length ? <hr /> : null}
            <button
              className={armed ? 'danger armed' : 'danger'}
              onClick={() => {
                if (!armed) {
                  setArmed(true);
                  return;
                }
                closeMenu();
                void store.remove(conn.id);
              }}
            >
              <Icon name="trash" />
              <span>{armed ? 'Нажмите ещё раз' : isSub ? 'Удалить подписку' : 'Удалить конфигурацию'}</span>
              {armed ? <small>удалить</small> : null}
            </button>
          </div>
        </>
      ) : null}
    </div>
  );
}

/** Плашка скорости над кнопкой: единственное на главной, что меняется раз в секунду от трафика. */
function SpeedPill({ onOpen }: { onOpen: () => void }) {
  const traffic = useTraffic();
  return (
    <button className="b-speed" onClick={onOpen} title="Скорость" aria-label="Скорость">
      <span>
        <Icon name="download" size={16} />
        <b className="num">{rate(traffic.down)}</b>
      </span>
      <i />
      <span>
        <Icon name="upload" size={16} />
        <b className="num">{rate(traffic.up)}</b>
      </span>
    </button>
  );
}

/** Лист «Скорость»: сейчас, всего за сеанс, график минуты и замер через сервер. */
function SpeedSheet({ onClose }: { onClose: () => void }) {
  const traffic = useTraffic();
  const store = useStore();
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<SpeedView | null>(null);
  const connected = store.state?.vpn === 'connected' && !store.pending;
  const server = store.state?.server;

  const test = async () => {
    setTesting(true);
    try {
      setResult(await store.transport.call<SpeedView>('speed_test'));
    } catch (e) {
      const err = e as ErrorInfo;
      store.toast(errorText(err?.code ?? 'unknown', err?.params), undefined, 'bad');
    }
    setTesting(false);
  };

  return (
    <Sheet onClose={onClose}>
      <h3>Скорость</h3>
      <p className="b-sheet-sub">
        {server ? `${server} · ` : ''}
        {connected ? (
          <>
            сеанс <Elapsed since={store.state?.since ?? null} format={fmtTimer} />
          </>
        ) : (
          'VPN выключен'
        )}
      </p>
      <div className="b-tiles">
        <div className="b-tile">
          <span>
            <Icon name="download" size={16} /> Чтение
          </span>
          <b className="num">{rate(traffic.down)}</b>
          <small>всего {gb(traffic.downSum)}</small>
        </div>
        <div className="b-tile">
          <span>
            <Icon name="upload" size={16} /> Загрузка
          </span>
          <b className="num">{rate(traffic.up)}</b>
          <small>всего {gb(traffic.upSum)}</small>
        </div>
      </div>
      <SpeedGraph down={traffic.downHist} up={traffic.upHist} />
      <div className="b-test">
        <div className="b-test-text">
          {testing ? (
            <span>Меряю скорость через сервер… до полуминуты</span>
          ) : result ? (
            <>
              <b>
                ↓ {result.down != null ? fmtMbps(result.down) : '—'} · ↑ {result.up != null ? fmtMbps(result.up) : '—'}
              </b>
              {result.server ? <span>{result.server}</span> : null}
            </>
          ) : (
            <span>{connected ? 'Сколько реально даёт сервер' : 'Замер — когда VPN включён'}</span>
          )}
        </div>
        <button className="btn-elem" onClick={() => void test()} disabled={testing || !connected}>
          <Icon name="refresh" size={13} className={testing ? 'spin' : undefined} />
          {testing ? 'Меряю…' : result ? 'Ещё раз' : 'Проверить'}
        </button>
      </div>
    </Sheet>
  );
}

function SpeedGraph({ down, up }: { down: number[]; up: number[] }) {
  const max = Math.max(50, ...down, ...up) * 1.1;
  const d = buildPath(down, max);
  const u = buildPath(up, max);
  const peak = Math.max(...down);
  return (
    <div className="graph">
      <div className="graph-head">
        <div>Последние 60 секунд</div>
        <div className="legend">
          <span>
            <i style={{ background: 'var(--accent)' }} />
            Чтение
          </span>
          <span>
            <i style={{ background: 'var(--dim)' }} />
            Загрузка
          </span>
        </div>
      </div>
      <svg viewBox="0 0 300 90" preserveAspectRatio="none" aria-hidden="true">
        <line x1="0" y1="89" x2="300" y2="89" stroke="var(--elem)" strokeWidth="1" />
        <line x1="0" y1="45" x2="300" y2="45" stroke="var(--elem)" strokeWidth="1" strokeDasharray="3 4" />
        <path d={d.area} fill="color-mix(in srgb, var(--accent) 12%, transparent)" />
        <path d={d.line} fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
        <path d={u.line} fill="none" stroke="var(--dim)" strokeWidth="1.5" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
      </svg>
      <div className="graph-foot">
        <span>−60 с</span>
        <span>Пик: {rate((peak * 1e6) / 8)}</span>
        <span>сейчас</span>
      </div>
    </div>
  );
}

function Empty({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="screen empty">
      <div className="empty-ico">
        <Icon name="power" size={38} />
      </div>
      <h2>Нет подключений</h2>
      <p>Добавьте ссылку на подписку или одиночную конфигурацию — и можно включать.</p>
      <div className="steps">
        <div className="step">
          <div className="step-n">1</div>
          <div>
            <b>Вставьте ссылку</b>
            <span>Подписка (https://…) или vless://, vmess://, trojan://, ss://</span>
          </div>
        </div>
        <div className="step">
          <div className="step-n">2</div>
          <div>
            <b>{isIos ? 'Выберите, что пускать в VPN' : 'Выберите режим'}</b>
            <span>{isIos ? 'Весь трафик или только нужные сайты и сервисы' : 'По умолчанию VPN (TUN) — работает для всех программ'}</span>
          </div>
        </div>
        <div className="step">
          <div className="step-n">3</div>
          <div>
            <b>Нажмите кнопку питания</b>
            <span>Заблокированные сервисы пойдут через VPN, остальное — напрямую</span>
          </div>
        </div>
      </div>
      <button className="btn-primary" style={{ marginTop: 22 }} onClick={onAdd}>
        Добавить подключение
      </button>
    </div>
  );
}
