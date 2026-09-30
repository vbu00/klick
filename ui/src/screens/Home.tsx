// Главная: как в макете — статус, кнопка, чип режима, подключения с серверами, скорость.

import { useEffect, useState, type CSSProperties } from 'react';
import { Icon } from '../components/Icon';
import { Sheet, type Tab } from '../components/Chrome';
import { SwitchDiagram } from '../components/SwitchDiagram';
import { buildPath, daysLeft, fmtBytes, fmtDate, fmtRate, fmtTimer, fmtTraffic, pingColor, pingText, protocolName } from '../lib/format';
import { errorText, modeName, routingName } from '../lib/i18n';
import { isMac } from '../lib/platform';
import { useStore } from '../lib/store';
import type { Connection, ServerView } from '../lib/types';

export function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [active]);
  return now;
}

export function Home({ onTab }: { onTab: (t: Tab) => void }) {
  const store = useStore();
  const { state, settings, servers, pinging, traffic, isRunning, neighbors } = store;
  const [viewId, setViewId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [switchTo, setSwitchTo] = useState<string | null>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const now = useNow(isRunning);
  const { nav, consumeNav } = store;

  // Нажали на уведомление: раскрыть серверы или подсказку про соседей.
  // `conn:<id>` — показать это подключение, как если бы его выбрали в списке («Уже добавлено»).
  useEffect(() => {
    if (!nav) return;
    if (nav.target.startsWith('conn:')) {
      const id = nav.target.slice('conn:'.length);
      setViewId(id);
      setExpanded(false);
      const st = store.settings;
      if (store.state?.vpn === 'off' && st && st.active_connection !== id && st.connections.some((c) => c.id === id)) void store.selectConnection(id);
    } else if (nav.target === 'servers') setExpanded(true);
    else if (nav.target === 'neighbors') setHelpOpen(true);
    else if (nav.target !== 'card') return;
    consumeNav();
  }, [nav, consumeNav]);

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
    vpn === 'connecting'
      ? 'подключаюсь…'
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
  const statusTone = vpn === 'reconnecting' || vpn === 'server_down' ? 'warn' : vpn === 'error' ? 'bad' : '';

  let core = '';
  if (vpn === 'connecting') core = 'busy';
  else if (vpn === 'reconnecting') core = viewingLive ? 'busy warn' : 'other';
  else if (vpn === 'server_down') core = viewingLive ? 'warn' : 'other';
  else if (vpn === 'error') core = 'bad';
  else if (isRunning) core = viewingLive ? 'live' : 'other';

  const dot = vpn === 'error' ? 'var(--red)' : vpn === 'reconnecting' || vpn === 'server_down' ? 'var(--orange)' : isRunning ? 'var(--accent)' : 'var(--dim)';

  const onPower = async () => {
    if (vpn === 'connecting') return;
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

  const pickConnection = async (c: Connection) => {
    setExpanded(false);
    setViewId(c.id);
    if (!isRunning && c.id !== activeId) await store.selectConnection(c.id);
  };

  const toggleExpand = () => {
    const next = !expanded;
    setExpanded(next);
    if (next && isActiveView && list.length > 1 && list.every((s) => s.delay == null)) void store.testLatency();
  };

  return (
    <div className="screen">
      <div className="home-status">
        <div className={`status-label ${statusTone}`}>{statusLabel}</div>
        <div className="timer">{isRunning ? fmtTimer(state.since, now) : '0:00:00'}</div>
      </div>

      <div className="power-wrap">
        {viewingLive && vpn === 'connected' ? (
          <div className="wave">
            <div className="dots" />
            <div className="lit" />
            <div className="ringfx" />
          </div>
        ) : null}
        <button className="power-btn" onClick={onPower} aria-label={isRunning ? 'Отключить VPN' : 'Включить VPN'}>
          <span className={`power-core ${core}`}>
            <Icon name="power" size={34} />
          </span>
        </button>
      </div>

      <div className="chip-row">
        <button className="mode-chip" onClick={() => onTab('connection')}>
          <span className="dot7" style={{ background: dot }} />
          {modeName[state.mode]} · {routingName[state.routing]}
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
        <button className="banner" onClick={() => setExpanded(true)} style={{ cursor: 'pointer' }}>
          <span className="dot7" />
          <div>
            Сервер не отвечает
            <small>Проверяю раз в минуту. Можно выбрать другой сервер</small>
          </div>
        </button>
      ) : vpn === 'error' ? (
        <div className="banner bad">
          <span className="dot7" />
          <div>
            VPN не включился
            <small>{errorText(state.error ?? '')}</small>
          </div>
        </div>
      ) : neighbors.length && state.mode === 'tun' ? (
        <button className="banner" onClick={() => setHelpOpen(true)} style={{ cursor: 'pointer' }}>
          <span className="dot7" />
          <div>
            {neighbors.join(', ')} мешает режиму VPN
            <small>Нажмите, чтобы узнать, что сделать</small>
          </div>
        </button>
      ) : null}

      {conns.length > 1 ? (
        <div className="pchips">
          {conns.map((c) => (
            <button key={c.id} className={c.id === view.id ? 'pchip on' : 'pchip'} onClick={() => void pickConnection(c)}>
              {isRunning && c.id === liveId ? <span className="live" /> : null}
              <span>{c.name}</span>
            </button>
          ))}
        </div>
      ) : null}

      <ProfileCard
        conn={view}
        style={{ marginTop: conns.length > 1 ? 10 : 34 }}
        viewingLive={viewingLive}
        live={vpn === 'connected'}
        isActive={isActiveView}
        list={list}
        current={current}
        expanded={expanded}
        pinging={pinging}
        onToggle={toggleExpand}
        onExpand={() => setExpanded(true)}
      />

      <div className="tiles">
        <div className="tile">
          <div className="tile-head">
            Чтение
            <Icon name="download" size={22} />
          </div>
          <div className="tile-val">{fmtRate(traffic.down)}</div>
          <div className="tile-sub">всего {fmtBytes(traffic.downSum)}</div>
        </div>
        <div className="tile">
          <div className="tile-head">
            Загрузка
            <Icon name="upload" size={22} />
          </div>
          <div className="tile-val">{fmtRate(traffic.up)}</div>
          <div className="tile-sub">всего {fmtBytes(traffic.upSum)}</div>
        </div>
      </div>
      <SpeedGraph down={traffic.downHist} up={traffic.upHist} />

      {switchTo ? (
        <Sheet onClose={() => setSwitchTo(null)}>
          <h3>Сменить подключение?</h3>
          <p>
            Сейчас работает <b>{conns.find((c) => c.id === liveId)?.name}</b>. Текущее соединение разорвётся на пару секунд, и трафик пойдёт через{' '}
            <b>{conns.find((c) => c.id === switchTo)?.name}</b>.
          </p>
          <SwitchDiagram liveName={conns.find((c) => c.id === liveId)?.name ?? ''} targetName={conns.find((c) => c.id === switchTo)?.name ?? ''} />
          <div className="sheet-actions" style={{ marginTop: 16 }}>
            <button onClick={() => setSwitchTo(null)}>Отмена</button>
            <button
              className="accent"
              onClick={() => {
                const id = switchTo;
                setSwitchTo(null);
                void store.selectConnection(id);
              }}
            >
              Переключить
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

function ProfileCard(props: {
  conn: Connection;
  style: CSSProperties;
  viewingLive: boolean;
  /** VPN работает без сбоев: показываем «Работает». */
  live: boolean;
  isActive: boolean;
  list: ServerView[];
  current: ServerView | undefined;
  expanded: boolean;
  pinging: boolean;
  onToggle: () => void;
  onExpand: () => void;
}) {
  const store = useStore();
  const { conn, list, current, pinging } = props;
  const [menuOpen, setMenuOpen] = useState(false);
  const [armed, setArmed] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const isSub = conn.kind === 'subscription';
  const info = conn.info;
  const used = info ? info.upload + info.download : 0;
  const pct = info && info.total > 0 ? Math.min(100, (used / info.total) * 100) : 0;
  // Задержку в строке показываем, только когда её измерили: null до проверки не значит «нет ответа».
  const ping = pinging ? '…' : current && current.delay != null ? pingText(current.delay) : '';

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
      <div className="pcard">
        <div
          className="pcard-head"
          onClick={props.onToggle}
          onContextMenu={(e) => {
            e.preventDefault();
            setMenuOpen(true);
          }}
        >
          <div className="pcard-icon">
            <Icon name={isSub ? 'globe' : 'rules'} size={24} />
          </div>
          <div className="pcard-body">
            <div className="pcard-title-row">
              <div className="pcard-title">{conn.name}</div>
              {props.viewingLive && props.live ? <span className="badge-live">Работает</span> : null}
            </div>
            <div className="pcard-sub">
              <span className="name">{current?.name ?? conn.selected_server ?? (isSub ? 'Подписка' : 'Одиночная конфигурация')}</span>
              {ping ? (
                <>
                  <span className="sep" />
                  <span className="num" style={{ color: pinging ? 'var(--dim)' : pingColor(current?.delay) }}>
                    {ping}
                  </span>
                </>
              ) : null}
            </div>
          </div>
          <button
            className={menuOpen ? 'menu-btn open' : 'menu-btn'}
            aria-label="Действия"
            title="Действия"
            onClick={(e) => {
              e.stopPropagation();
              setMenuOpen(!menuOpen);
            }}
          >
            <Icon name="dots" size={20} />
          </button>
        </div>

        {isSub ? (
          <div className="traffic">
            {info && info.total > 0 ? (
              <>
                <div className="traffic-row">
                  <span>Трафик</span>
                  <b>{fmtTraffic(used, info.total)}</b>
                </div>
                <div className="bar">
                  <i style={{ width: `${pct}%`, background: pct > 90 ? 'var(--orange)' : 'var(--text)' }} />
                </div>
              </>
            ) : null}
            <div className="expiry" style={info && info.total > 0 ? undefined : { marginTop: 0 }}>
              <span>
                {info?.expire ? (
                  <>
                    Действует до <b>{fmtDate(info.expire)}</b> · {daysLeft(info.expire) === 0 ? 'истекает' : `${daysLeft(info.expire)} дн.`}
                  </>
                ) : (
                  'Срок и трафик панель не сообщает'
                )}
              </span>
              <button className="btn-elem" onClick={refresh} disabled={refreshing}>
                <Icon name="refresh" size={13} className={refreshing ? 'spin' : undefined} />
                {refreshing ? 'Обновляем…' : 'Обновить'}
              </button>
            </div>
          </div>
        ) : null}

        {props.expanded ? (
          <div className="servers">
            {!props.isActive ? (
              <div className="srv-empty">Серверы этого подключения появятся, когда вы переключитесь на него.</div>
            ) : isSub || list.length > 1 ? (
              <>
                <div className="servers-head">
                  <div className="caps">Серверы · {list.length}</div>
                  <button className="btn-elem" onClick={() => void store.testLatency()} disabled={pinging}>
                    {pinging ? 'Проверяем…' : 'Проверить задержку'}
                  </button>
                </div>
                {list.map((s) => (
                  <button key={s.name} className={s.name === current?.name ? 'srv on' : 'srv'} onClick={() => s.name !== current?.name && void store.selectServer(s.name)}>
                    <span className="radio">
                      <i />
                    </span>
                    <span style={{ flex: 1, minWidth: 0 }}>
                      <span className="srv-name" style={{ display: 'block' }}>
                        {s.name}
                      </span>
                      <span className="srv-proto" style={{ display: 'block' }}>
                        {protocolName(s.kind)}
                      </span>
                    </span>
                    <span className="num" style={{ color: pinging ? 'var(--dim)' : pingColor(s.delay) }}>
                      {pinging ? '…' : pingText(s.delay)}
                    </span>
                  </button>
                ))}
              </>
            ) : (
              <div className="kv">
                <div>
                  <span>Протокол</span>
                  <span>{current ? protocolName(current.kind) : '—'}</span>
                </div>
                <div>
                  <span>Сервер</span>
                  <span>{current?.name ?? '—'}</span>
                </div>
                <div>
                  <span>Задержка</span>
                  <span style={{ color: pingColor(current?.delay) }}>{pinging ? '…' : pingText(current?.delay)}</span>
                </div>
                <button className="btn-elem" style={{ height: 34, borderRadius: 10, justifyContent: 'center' }} onClick={() => void store.testLatency()} disabled={pinging}>
                  {pinging ? 'Проверяем…' : 'Проверить задержку'}
                </button>
              </div>
            )}
          </div>
        ) : null}
      </div>

      {menuOpen ? (
        <>
          <div className="menu-shade" onClick={closeMenu} onContextMenu={(e) => (e.preventDefault(), closeMenu())} />
          <div className="menu" role="menu">
            {isSub ? (
              <button
                onClick={() => {
                  closeMenu();
                  void refresh();
                }}
              >
                <Icon name="refresh" />
                <span>Обновить подписку</span>
              </button>
            ) : null}
            {props.isActive ? (
              <button
                onClick={() => {
                  closeMenu();
                  props.onExpand();
                  void store.testLatency();
                }}
              >
                <Icon name="server" />
                <span>Проверить задержку</span>
              </button>
            ) : null}
            <hr />
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
        <span>Пик: {fmtRate((peak * 1e6) / 8)}</span>
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
            <b>Выберите режим</b>
            <span>По умолчанию VPN (TUN) — работает для всех программ</span>
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
