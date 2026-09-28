// «Соединение» по макету v4: режим, «Куда направлять» с готовыми наборами, «Мой список» карточкой,
// «Сейчас в сети» сводкой; ниже — живая схема и «Как вас видят сайты».
// Подэкраны: «Мой список», «Сейчас в сети», «Не открывается?». Везде один тумблер — «через VPN»:
// в списке он включает и выключает правило, в «Сейчас в сети» сам пишет правило в мой список.

import { useEffect, useMemo, useRef, useState } from 'react';
import { AddRule } from '../components/AddRule';
import { Back, Seg, Tile, Toggle } from '../components/Controls';
import { Icon } from '../components/Icon';
import { IpCard } from '../components/IpCard';
import { LiveSchema } from '../components/LiveSchema';
import { ModeInfo } from '../components/ModeInfo';
import { failureReason, routeName, routingTitle } from '../lib/i18n';
import { groupLive, positionVerb, programTarget, siteRule, siteTarget, type AppRow, type SiteRow } from '../lib/live';
import { defaultRoute, plural, ruleTitle, unicodeDomain } from '../lib/rules';
import { useStore } from '../lib/store';
import type { Transport } from '../lib/transport';
import type { ConnView, FailureView, Mode, Routing, Rule, Service, Target } from '../lib/types';

type Sub = null | 'list' | 'live' | 'failures';

/** Соединения и неудачи раз в 2 секунды, пока открыта вкладка и работает VPN. */
function useLive(running: boolean, transport: Transport) {
  const [conns, setConns] = useState<ConnView[]>([]);
  const [fails, setFails] = useState<FailureView[]>([]);
  useEffect(() => {
    if (!running) {
      setConns([]);
      setFails([]);
      return;
    }
    let alive = true;
    const tick = async () => {
      const [c, f] = await Promise.all([
        transport.call<ConnView[]>('connections').catch(() => null),
        transport.call<FailureView[]>('failures').catch(() => null),
      ]);
      if (!alive) return;
      if (c) setConns(c);
      if (f) setFails(f);
    };
    void tick();
    const t = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [running, transport]);
  return { conns, fails };
}

/** Имя цели для сообщения: сервис по каталогу, сайт буквами. */
function targetLabel(t: Target, catalog: Service[]): string {
  if (t.kind === 'service') return catalog.find((s) => s.id === t.value)?.name ?? t.value;
  return unicodeDomain(t.value);
}

export function Connection() {
  const store = useStore();
  const { state, settings, catalog, transport, isRunning, programNames } = store;
  const [sub, setSub] = useState<Sub>(null);
  const [adding, setAdding] = useState(false);
  const [modeInfo, setModeInfo] = useState<Mode | null>(null);
  const { conns, fails } = useLive(isRunning, transport);

  useEffect(() => {
    document.querySelector('main.content')?.scrollTo({ top: 0 });
  }, [sub]);

  const routing = settings?.routing ?? 'selected';
  const list = useMemo(() => settings?.lists[routing] ?? [], [settings, routing]);
  const apps = useMemo(() => groupLive(conns, list, settings?.kill_switch, catalog, programNames), [conns, list, settings?.kill_switch, catalog, programNames]);

  if (!state || !settings) return null;

  return (
    <div className="screen">
      {sub === null ? (
        <Main routing={routing} list={list} apps={apps} fails={fails} conns={conns} onSub={setSub} onAdd={() => setAdding(true)} onModeInfo={() => setModeInfo(settings.mode)} />
      ) : null}
      {sub === 'list' ? <MyList routing={routing} list={list} onBack={() => setSub(null)} onAdd={() => setAdding(true)} /> : null}
      {sub === 'live' ? <Live apps={apps} onBack={() => setSub(null)} /> : null}
      {sub === 'failures' ? <Failures fails={fails} list={list} onBack={() => setSub(null)} /> : null}

      {adding ? <AddRule position={routing} onClose={() => setAdding(false)} /> : null}
      {modeInfo ? (
        <ModeInfo
          initial={modeInfo}
          current={settings.mode}
          onClose={() => setModeInfo(null)}
          onPick={(m) => {
            setModeInfo(null);
            void store.setMode(m);
          }}
        />
      ) : null}
    </div>
  );
}

function Main({
  routing,
  list,
  apps,
  fails,
  conns,
  onSub,
  onAdd,
  onModeInfo,
}: {
  routing: Routing;
  list: Rule[];
  apps: AppRow[];
  fails: FailureView[];
  conns: ConnView[];
  onSub: (s: Sub) => void;
  onAdd: () => void;
  onModeInfo: () => void;
}) {
  const store = useStore();
  const { state, settings, catalog, isRunning, programNames } = store;
  if (!state || !settings) return null;
  const mode = settings.mode;
  const russia = settings.russia_direct;
  const verb = positionVerb(routing);

  const count = (kinds: Target['kind'][]) => list.filter((r) => kinds.includes(r.target.kind)).length;
  const parts = [
    [count(['service']), ['сервис', 'сервиса', 'сервисов']],
    [count(['program']), ['программа', 'программы', 'программ']],
    [count(['domain', 'ip']), ['сайт', 'сайта', 'сайтов']],
  ] as const;
  const summary = parts.filter(([n]) => n > 0).map(([n, forms]) => plural(n, [...forms] as [string, string, string]));
  const enabled = list.filter((r) => r.enabled !== false).length;

  return (
    <>
      <div className="screen-title xl">Соединение</div>

      <div className="cx-head">
        <span>Режим</span>
        <button className="info-btn" aria-label="Как это работает" title="Как это работает" onClick={onModeInfo}>
          <Icon name="info" size={16} />
        </button>
      </div>
      <Seg
        className="cx-seg"
        value={mode}
        options={[
          ['tun', 'VPN (TUN)'],
          ['sys_proxy', 'Системный прокси'],
        ]}
        onChange={(m) => void store.setMode(m)}
      />
      <div className="cx-desc">
        {mode === 'tun' ? 'Все программы, включая игры и UDP.' : 'Браузеры и программы, которые используют системный прокси.'}
        {isRunning ? ' Смена режима переподключит VPN на пару секунд.' : ''}
      </div>

      <div className="cx-head">
        <span>Куда направлять</span>
      </div>
      <Seg
        className="cx-seg"
        value={routing}
        options={[
          ['all_vpn', routingTitle.all_vpn],
          ['selected', routingTitle.selected],
        ]}
        onChange={(r) => void store.setRouting(r)}
      />
      <div className="cx-card mt8">
        {routing === 'selected' ? (
          <Preset title="Заблокированное в РФ — через VPN" desc="Готовый набор, обновляется сам. Остальное — напрямую." on={settings.blocked_preset !== false} onChange={(v) => void store.setPrefs({ blocked_preset: v })} />
        ) : (
          <>
            <Preset title=".ru и .рф напрямую" desc="Госуслуги, банки, Яндекс — без VPN" on={russia.domains} onChange={(v) => void store.setRussia('ru_domains', v)} />
            <Preset title="Российские IP напрямую" desc="По базе стран, даже без .ru в адресе" on={russia.ips} onChange={(v) => void store.setRussia('ru_ips', v)} />
            <Preset title="Локальная сеть — напрямую" desc="Роутер, принтер, игры по локалке" />
          </>
        )}
      </div>

      <div className="cx-head wide">
        <span>Мой список</span>
      </div>
      <button className="cx-card cx-list" onClick={() => onSub('list')}>
        <span className="grow">
          <span className="cx-list-title">{verb} · мой список</span>
          <span className="cx-list-sum">{list.length ? summary.join(' · ') : 'Пока пусто — добавьте сервис, программу или сайт'}</span>
          {list.length ? (
            <span className="cx-stack">
              {list.slice(0, 6).map((r, i) => (
                <span key={i} className={r.enabled === false ? 'cx-stack-tile off' : 'cx-stack-tile'}>
                  <Tile label={ruleTitle(r, catalog, programNames)} size={28} />
                </span>
              ))}
              <span className="cx-stack-note">
                включено {enabled} из {list.length}
              </span>
            </span>
          ) : null}
        </span>
        <Icon name="chevron" size={14} className="cx-chev" />
      </button>
      <button className="cx-add" onClick={onAdd}>
        <Icon name="plus" size={16} />
        Добавить
      </button>

      <div className="cx-head wide">
        <span>Сейчас в сети</span>
      </div>
      <LiveSummary apps={apps} running={isRunning} onOpen={() => onSub('live')} />
      {fails.length ? (
        <button className="cx-card cx-fails" onClick={() => onSub('failures')}>
          <span className="grow">
            <span className="cx-list-title sm">Не открывается?</span>
            <span className="cx-list-sum">Недавние неудачные соединения</span>
          </span>
          <span className="cx-count warn">{fails.length}</span>
          <Icon name="chevron" size={14} className="cx-chev" />
        </button>
      ) : null}

      <div className="cx-head wide">
        <span>Схема · сейчас</span>
      </div>
      <LiveSchema running={isRunning} mode={mode} server={state.server} conns={conns} catalog={catalog} />

      <IpCard />
    </>
  );
}

function Preset({ title, desc, on, onChange }: { title: string; desc: string; on?: boolean; onChange?: (v: boolean) => void }) {
  return (
    <div className="cx-row">
      <div className="grow">
        <div className="cx-row-title">{title}</div>
        <div className="cx-row-sub">{desc}</div>
      </div>
      {onChange ? <Toggle label={title} on={!!on} onChange={onChange} /> : <span className="cx-always">всегда</span>}
    </div>
  );
}

function LiveSummary({ apps, running, onOpen }: { apps: AppRow[]; running: boolean; onOpen: () => void }) {
  const total = apps.reduce((a, x) => a + x.conns, 0);
  const vpn = apps.reduce((a, x) => a + x.vpnConns, 0);
  const pct = total ? (vpn / total) * 100 : 0;
  return (
    <button className="cx-card cx-live" onClick={onOpen}>
      <span className="cx-live-top">
        <span className="cx-live-count">
          <b>{plural(total, ['соединение', 'соединения', 'соединений'])}</b>
          <span>{plural(apps.length, ['программа', 'программы', 'программ'])}</span>
        </span>
        <span className="cx-bar">
          <i style={{ width: `${pct}%` }} />
          {total > vpn ? <em /> : null}
        </span>
        <span className="cx-legend">
          <span>
            <i className="vpn" />
            VPN <b>{vpn}</b>
          </span>
          <span>
            <i className="direct" />
            Напрямую <b>{total - vpn}</b>
          </span>
        </span>
        {!running ? <span className="cx-live-off">Соединения появятся после подключения</span> : null}
      </span>
      {apps.slice(0, 3).map((a) => (
        <span key={a.key} className="cx-live-app">
          <Tile label={a.name} size={28} />
          <span className="grow">
            <span className="t">{a.name}</span>
            <span className="s">
              {a.conns} соед. · {(a.bytes / 1e6).toFixed(1).replace('.', ',')} МБ
            </span>
          </span>
          <span className="cx-mini">
            <i style={{ width: `${a.conns ? (a.vpnConns / a.conns) * 100 : 0}%` }} />
          </span>
        </span>
      ))}
    </button>
  );
}

// ── Мой список ───────────────────────────────────────────────────────────

type ListTab = 'all' | 'service' | 'program' | 'site';

const GROUPS: [Exclude<ListTab, 'all'>, string][] = [
  ['service', 'Сервисы'],
  ['program', 'Программы'],
  ['site', 'Сайты и IP'],
];

function MyList({ routing, list, onBack, onAdd }: { routing: Routing; list: Rule[]; onBack: () => void; onAdd: () => void }) {
  const store = useStore();
  const [tab, setTab] = useState<ListTab>('all');
  const verb = positionVerb(routing);
  const enabled = list.filter((r) => r.enabled !== false).length;
  const kindOf = (r: Rule): Exclude<ListTab, 'all'> => (r.target.kind === 'domain' || r.target.kind === 'ip' ? 'site' : r.target.kind);
  const groups = GROUPS.filter(([k]) => tab === 'all' || tab === k)
    .map(([k, label]) => ({ k, label, rows: list.map((rule, index) => ({ rule, index })).filter(({ rule }) => kindOf(rule) === k) }))
    .filter((g) => g.rows.length);

  return (
    <>
      <Back label="Соединение" onClick={onBack} />
      <div className="sub-title">{verb} · мой список</div>
      <div className="sub-lead">
        Положение «{routingTitle[routing]}». Включено {enabled} из {list.length}. Выключенное правило не работает, но остаётся в списке.
      </div>
      <Seg
        className="mt14 cx-seg-sm"
        value={tab}
        options={[
          ['all', 'Все'],
          ['service', 'Сервисы'],
          ['program', 'Программы'],
          ['site', 'Сайты'],
        ]}
        onChange={setTab}
      />
      {groups.map((g) => (
        <div key={g.k}>
          <div className="cx-head sm">
            <span>{g.label}</span>
          </div>
          <div className="cx-card">
            {g.rows.map(({ rule, index }) => {
              const title = ruleTitle(rule, store.catalog, store.programNames);
              const on = rule.enabled !== false;
              const exception = rule.route !== defaultRoute[routing];
              return (
                <div key={`${rule.target.kind}:${rule.target.value}`} className="cx-rule" title={rule.target.kind === 'program' ? rule.target.value : undefined}>
                  <span className={on ? 'cx-rule-tile' : 'cx-rule-tile off'}>
                    <Tile label={title} size={30} />
                  </span>
                  <span className={on ? 'cx-rule-name' : 'cx-rule-name off'}>{title}</span>
                  {exception ? <span className={`route-badge ${rule.route}`}>{routeName[rule.route]}</span> : null}
                  <Toggle label={title} on={on} onChange={(v) => void store.setRuleEnabled(routing, index, v)} />
                  <button className="cx-x" aria-label="Удалить правило" title="Удалить" onClick={() => void store.removeRule(routing, index)}>
                    <Icon name="winClose" size={11} />
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      ))}
      {!groups.length ? (
        <div className="cx-empty">{routing === 'selected' ? 'Добавьте сервис, программу или сайт — они пойдут через VPN.' : 'Добавьте то, что должно идти напрямую: банк, игру, локальный сервис.'}</div>
      ) : null}
      <button className="cx-add mt12" onClick={onAdd}>
        <Icon name="plus" size={16} />
        Добавить
      </button>
    </>
  );
}

// ── Сейчас в сети ────────────────────────────────────────────────────────

type LiveView = 'apps' | 'sites';
type LiveFilter = 'all' | 'vpn' | 'direct';

function Live({ apps, onBack }: { apps: AppRow[]; onBack: () => void }) {
  const store = useStore();
  const { isRunning, settings, catalog } = store;
  const [view, setView] = useState<LiveView>('apps');
  const [filter, setFilter] = useState<LiveFilter>('all');
  const [paused, setPaused] = useState(false);
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const frozen = useRef<AppRow[]>(apps);
  if (!paused) frozen.current = apps;
  const shown = paused ? frozen.current : apps;
  // Пока открыта первая программа — так видно, что у программ есть сайты.
  const first = shown[0]?.key;
  const isOpen = (key: string) => open[key] ?? key === first;

  const sites = shown.flatMap((a) => a.sites);
  const nVpn = sites.filter((s) => s.vpn).length;
  const ok = (vpn: boolean) => filter === 'all' || (filter === 'vpn') === vpn;

  const routeApp = (a: AppRow, vpn: boolean) => {
    if (!a.path) {
      store.toast('Путь программы неизвестен', 'Добавьте её через «Добавить» → «Программы»', 'warn');
      return;
    }
    void store.routeTarget(programTarget(a.path), vpn, a.name);
  };
  const routeSite = (s: SiteRow, vpn: boolean) => void store.routeTarget(s.target, vpn, targetLabel(s.target, catalog));

  return (
    <>
      <Back label="Соединение" onClick={onBack} />
      <div className="cx-live-head">
        <div className="sub-title">Сейчас в сети</div>
        <button className={paused ? 'cx-pill' : 'cx-pill live'} onClick={() => setPaused(!paused)} disabled={!isRunning}>
          <i />
          {paused ? 'Пауза' : 'Live'}
        </button>
      </div>
      <div className="sub-lead">Тумблер включён — идёт через VPN. Правило сохранится в мой список.</div>
      <Seg
        className="mt14"
        value={view}
        options={[
          ['apps', 'Программы'],
          ['sites', 'Сайты'],
        ]}
        onChange={setView}
      />
      <div className="cx-filters">
        {(
          [
            ['all', 'Все', sites.length, 'dim'],
            ['vpn', 'VPN', nVpn, 'vpn'],
            ['direct', 'Прямо', sites.length - nVpn, 'direct'],
          ] as const
        ).map(([k, label, n, dot]) => (
          <button key={k} className={filter === k ? 'cx-chip on' : 'cx-chip'} onClick={() => setFilter(k)}>
            <i className={dot} />
            {label} <span>{n}</span>
          </button>
        ))}
      </div>

      {!isRunning ? (
        <div className="cx-empty">VPN выключен. Соединения появятся после подключения.</div>
      ) : (
        <div className="cx-card mt10">
          {view === 'apps'
            ? shown
                .filter((a) => (a.sites.length ? a.sites.some((s) => ok(s.vpn)) : ok(a.vpn)))
                .map((a) => {
                  const nV = a.sites.filter((s) => s.vpn).length;
                  const expanded = isOpen(a.key) && a.sites.length > 0;
                  const sub =
                    a.why === 'killswitch'
                      ? 'Kill Switch · всегда через VPN'
                      : a.why === 'program'
                        ? `правило программы · ${a.vpn ? 'через VPN' : 'напрямую'}`
                        : `${plural(a.sites.length, ['сайт', 'сайта', 'сайтов'])} · ${nV ? `через VPN ${nV}` : 'все напрямую'}`;
                  return (
                    <div key={a.key} className="cx-app">
                      <div className="cx-app-row">
                        <button className="cx-app-main" onClick={() => setOpen({ ...open, [a.key]: !expanded })}>
                          <Tile label={a.name} size={30} />
                          <span className="grow">
                            <span className="t">
                              {a.name}
                              {a.sites.length ? <Icon name="chevron" size={10} className={expanded ? 'cx-caret open' : 'cx-caret'} /> : null}
                            </span>
                            <span className={a.vpn || nV ? 's vpn' : 's'}>{sub}</span>
                          </span>
                        </button>
                        <Toggle label={`${a.name} через VPN`} on={a.vpn} disabled={a.why === 'killswitch'} onChange={(v) => routeApp(a, v)} />
                      </div>
                      {expanded
                        ? a.sites
                            .filter((s) => ok(s.vpn))
                            .map((s) => (
                              <div key={s.host} className="cx-site">
                                <span className="grow">
                                  <span className="t">{s.host}</span>
                                  <span className={s.vpn ? 's vpn' : 's'}>{a.locked ? (a.why === 'killswitch' ? 'Kill Switch' : 'по правилу программы') : s.vpn ? 'через VPN' : 'напрямую'}</span>
                                </span>
                                <Toggle label={`${s.host} через VPN`} on={s.vpn} disabled={a.locked} onChange={(v) => routeSite(s, v)} />
                              </div>
                            ))
                        : null}
                    </div>
                  );
                })
            : sites
                .filter((s) => ok(s.vpn))
                .map((s) => {
                  const app = shown.find((a) => a.key === s.appKey);
                  const locked = !!app?.locked;
                  return (
                    <div key={`${s.appKey}-${s.host}`} className="cx-rule">
                      <Tile label={s.host} size={30} />
                      <span className="grow">
                        <span className="cx-rule-name">{s.host}</span>
                        <span className={s.vpn ? 'cx-rule-sub vpn' : 'cx-rule-sub'}>
                          {s.app} · {locked ? 'по правилу программы' : s.vpn ? 'через VPN' : 'напрямую'}
                        </span>
                      </span>
                      <Toggle label={`${s.host} через VPN`} on={s.vpn} disabled={locked} onChange={(v) => routeSite(s, v)} />
                    </div>
                  );
                })}
          {(view === 'apps' ? !shown.length : !sites.filter((s) => ok(s.vpn)).length) ? <div className="cx-none">Ничего нет</div> : null}
        </div>
      )}
      {settings?.mode === 'sys_proxy' ? <div className="foot-note">В режиме системного прокси видны только программы, которые идут через прокси.</div> : null}
    </>
  );
}

// ── Не открывается? ──────────────────────────────────────────────────────

function Failures({ fails, list, onBack }: { fails: FailureView[]; list: Rule[]; onBack: () => void }) {
  const store = useStore();
  const { catalog } = store;
  const now = Date.now() / 1000;
  const ago = (at: number) => {
    const m = Math.max(0, Math.round((now - at) / 60));
    return m < 1 ? 'только что' : m < 60 ? `${m} мин назад` : `${Math.round(m / 60)} ч назад`;
  };
  return (
    <>
      <Back label="Соединение" onClick={onBack} />
      <div className="sub-title">Не открывается?</div>
      <div className="sub-lead">Недавние неудачные соединения. Если сайт не открывается напрямую, пустите его через VPN — и наоборот.</div>
      {fails.length === 0 ? (
        <div className="cx-empty">Неудачных соединений нет</div>
      ) : (
        <div className="cx-card mt14">
          {fails.map((f, i) => {
            const host = unicodeDomain(f.host);
            const target = siteTarget(f.host, catalog);
            const rule = siteRule(list, f.host, catalog);
            const vpn = rule ? rule.route === 'vpn' : f.route === 'vpn';
            return (
              <div key={`${f.host}-${f.at}-${i}`} className="cx-rule">
                <Tile label={host} size={30} />
                <span className="grow">
                  <span className="cx-rule-name">{host}</span>
                  <span className="cx-rule-sub">
                    {routeName[f.route]} · {failureReason(f.error)} · {ago(f.at)}
                  </span>
                </span>
                <Toggle label={`${host} через VPN`} on={vpn} onChange={(v) => void store.routeTarget(target, v, targetLabel(target, catalog))} />
              </div>
            );
          })}
        </div>
      )}
      <div className="foot-note">Причина обрыва известна не всегда. Список хранится только в памяти и пропадает при выходе.</div>
    </>
  );
}
