// «Маршруты» (бета): что пускать через VPN — двумя карточками, российское напрямую — одним тумблером,
// мой список, «Сейчас» с программами и метками VPN / Напрямую / Частично, «Что-то не открывается?».
// Подэкраны: «Мой список», «Сейчас в сети», «Не открывается?», «Диагностика» (проверка серверов и
// «Как вас видят сайты»). Способ подключения (TUN / прокси) — в настройках.

import { useEffect, useMemo, useRef, useState } from 'react';
import { AddRule } from '../components/AddRule';
import { Back, Row, Seg, Tile, Toggle } from '../components/Controls';
import { Icon } from '../components/Icon';
import { IpCard } from '../components/IpCard';
import { ProbeCard } from '../components/ProbeCard';
import { failureReason, routeName, routingTitle } from '../lib/i18n';
import { groupLive, positionVerb, programTarget, siteRule, siteTarget, type AppRow, type SiteRow } from '../lib/live';
import { isIos } from '../lib/platform';
import { defaultRoute, plural, ruleTitle, unicodeDomain } from '../lib/rules';
import { useStore, useVisible } from '../lib/store';
import type { Transport } from '../lib/transport';
import type { ConnView, FailureView, Mode, Routing, Rule, Service, Target } from '../lib/types';

type Sub = null | 'list' | 'live' | 'failures' | 'diag';

/** Соединения и неудачи раз в 2 секунды, пока открыта вкладка, работает VPN и окно видно. Спрятанное
 *  в трей окно службу не опрашивает; показали — сразу свежие данные. */
function useLive(running: boolean, transport: Transport) {
  const [conns, setConns] = useState<ConnView[]>([]);
  const [fails, setFails] = useState<FailureView[]>([]);
  const visible = useVisible();
  useEffect(() => {
    if (!running) {
      setConns([]);
      setFails([]);
      return;
    }
    if (!visible) return;
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
  }, [running, visible, transport]);
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
      {sub === null ? <Main routing={routing} list={list} apps={apps} fails={fails} onSub={setSub} /> : null}
      {sub === 'list' ? <MyList routing={routing} list={list} onBack={() => setSub(null)} onAdd={() => setAdding(true)} /> : null}
      {sub === 'live' ? <Live apps={apps} onBack={() => setSub(null)} /> : null}
      {sub === 'failures' ? <Failures fails={fails} list={list} onBack={() => setSub('diag')} /> : null}
      {sub === 'diag' ? <Diagnostics back="Маршруты" onBack={() => setSub(null)} fails={fails} onFailures={() => setSub('failures')} /> : null}

      {adding ? <AddRule position={routing} onClose={() => setAdding(false)} /> : null}
    </div>
  );
}

/** «Диагностика»: недавние неудачи, проверка серверов в этой сети и «Как вас видят сайты». */
export function Diagnostics({ back, onBack, fails, onFailures }: { back: string; onBack: () => void; fails?: FailureView[]; onFailures?: () => void }) {
  return (
    <>
      <Back label={back} onClick={onBack} />
      <div className="sub-title">Диагностика</div>
      <div className="sub-lead">Что мешает открыть сайт: сервер, блокировка в этой сети или утечка мимо VPN.</div>
      {fails && onFailures ? (
        <div className="card-list mt14">
          <Row
            title="Не открывается сейчас"
            sub={fails.length ? 'Недавние неудачные соединения — можно пустить через VPN или мимо' : 'Неудачных соединений не было'}
            right={fails.length ? <span className="cx-count warn">{fails.length}</span> : undefined}
            onClick={onFailures}
            chevron
          />
        </div>
      ) : null}
      <ProbeCard />
      <IpCard />
    </>
  );
}

function Main({ routing, list, apps, fails, onSub }: { routing: Routing; list: Rule[]; apps: AppRow[]; fails: FailureView[]; onSub: (s: Sub) => void }) {
  const store = useStore();
  const { settings, catalog, isRunning, programNames } = store;
  if (!settings) return null;
  const russia = settings.russia_direct;
  const ruOn = russia.domains || russia.ips;
  // Один тумблер на оба набора: сайты .ru/.рф и российские адреса.
  const setRu = async (v: boolean) => {
    await store.setRussia('ru_domains', v);
    await store.setRussia('ru_ips', v);
  };

  const count = (kinds: Target['kind'][]) => list.filter((r) => kinds.includes(r.target.kind)).length;
  const parts = [
    [count(['service']), ['сервис', 'сервиса', 'сервисов']],
    [isIos ? 0 : count(['program']), ['программа', 'программы', 'программ']],
    [count(['domain', 'ip']), ['сайт', 'сайта', 'сайтов']],
  ] as const;
  const summary = parts.filter(([n]) => n > 0).map(([n, forms]) => plural(n, [...forms] as [string, string, string])).join(', ');
  const listRow = (title: string, empty: string) => (
    <Row title={title} sub={list.length ? summary : empty} onClick={() => onSub('list')} chevron left={list.length ? <Tile label={ruleTitle(list[0], catalog, programNames)} size={30} /> : undefined} />
  );

  return (
    <>
      <div className="screen-title xl">Маршруты</div>

      <span className="b-cap">Что пускать через VPN</span>
      <div className="b-choices">
        <Choice on={routing === 'all_vpn'} title="Всё, кроме исключений" sub="Российские сайты и ваши исключения — напрямую" onClick={() => void store.setRouting('all_vpn')} />
        <Choice on={routing === 'selected'} title="Только выбранное" sub="Заблокированное в РФ и то, что добавите сами" onClick={() => void store.setRouting('selected')} />
      </div>

      {routing === 'all_vpn' ? (
        <>
          <span className="b-cap">Напрямую, без VPN</span>
          <div className="card-list">
            <Row wrap title="Российские сайты и сервисы" sub="Госуслуги, банки, маркетплейсы, VK — без VPN" right={<Toggle label="Российские сайты и сервисы" on={ruOn} onChange={(v) => void setRu(v)} />} />
            {listRow('Мои исключения', isIos ? 'Сайты и сервисы мимо VPN' : 'Сайты, сервисы и программы мимо VPN')}
          </div>
        </>
      ) : (
        <>
          <span className="b-cap">Через VPN</span>
          <div className="card-list">
            <Row wrap title="Заблокированное в РФ" sub="Готовый набор, обновляется сам" right={<Toggle label="Заблокированное в РФ" on={settings.blocked_preset !== false} onChange={(v) => void store.setPrefs({ blocked_preset: v })} />} />
            {listRow('Мой список', isIos ? 'Добавьте сервис или сайт' : 'Добавьте сервис, программу или сайт')}
          </div>
        </>
      )}
      <div className="b-foot">Локальная сеть — роутер, принтер, NAS — всегда идёт напрямую.</div>

      <div className="b-cap-row">
        <span className="b-cap">Сейчас</span>
        {isRunning ? (
          <button className="b-link" onClick={() => onSub('live')}>
            {isIos ? 'Все сайты' : 'Все программы'}
          </button>
        ) : null}
      </div>
      <LiveSummary apps={apps} running={isRunning} onOpen={() => onSub('live')} />

      <div className="card-list mt12">
        <Row
          title="Что-то не открывается?"
          sub="Диагностика: серверы, блокировки, утечки"
          right={fails.length ? <span className="cx-count warn">{fails.length}</span> : undefined}
          onClick={() => onSub('diag')}
          chevron
        />
      </div>
    </>
  );
}

/** Карточка выбора с кружком: «Всё, кроме исключений» / «Только выбранное». */
function Choice({ on, title, sub, onClick }: { on: boolean; title: string; sub: string; onClick: () => void }) {
  return (
    <button className={on ? 'b-choice on' : 'b-choice'} role="radio" aria-checked={on} onClick={onClick}>
      <span className="b-radio">
        <i />
      </span>
      <span className="b-choice-text">
        <span className="b-choice-title">{title}</span>
        <span className="b-choice-sub">{sub}</span>
      </span>
    </button>
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
  // iOS: вместо программ — самые активные сайты.
  const rows = isIos
    ? apps
        .flatMap((a) => a.sites)
        .sort((x, y) => y.bytes - x.bytes)
        .map((s) => ({ key: s.host, name: s.host, conns: s.conns, vpnConns: s.vpn ? s.conns : 0, bytes: s.bytes }))
    : apps;
  const total = apps.reduce((a, x) => a + x.conns, 0);
  const vpn = apps.reduce((a, x) => a + x.vpnConns, 0);
  const pct = total ? (vpn / total) * 100 : 0;
  return (
    <button className="cx-card cx-live" onClick={onOpen}>
      <span className="cx-live-top">
        <span className="cx-live-count">
          <b>{plural(total, ['соединение', 'соединения', 'соединений'])}</b>
          {isIos ? null : <span>{plural(apps.length, ['программа', 'программы', 'программ'])}</span>}
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
      {rows.slice(0, 3).map((a) => (
        <span key={a.key} className="cx-live-app">
          <Tile label={a.name} size={28} />
          <span className="grow">
            <span className="t">{a.name}</span>
            <span className="s">
              {a.conns} соед. · {(a.bytes / 1e6).toFixed(1).replace('.', ',')} МБ
            </span>
          </span>
          {a.vpnConns === a.conns ? <span className="b-tag vpn">VPN</span> : a.vpnConns === 0 ? <span className="b-tag direct">Напрямую</span> : <span className="b-tag mixed">Частично</span>}
        </span>
      ))}
    </button>
  );
}

// ── Мой список ───────────────────────────────────────────────────────────

type ListTab = 'all' | 'service' | 'program' | 'site';

const GROUPS: [Exclude<ListTab, 'all'>, string][] = [
  ['service', 'Сервисы'],
  ...(isIos ? [] : [['program', 'Программы'] as [Exclude<ListTab, 'all'>, string]]),
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
      <Back label="Маршруты" onClick={onBack} />
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
          ...(isIos ? [] : [['program', 'Программы'] as [ListTab, string]]),
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
        <div className="cx-empty">{routing === 'selected'
            ? isIos
              ? 'Добавьте сервис или сайт — они пойдут через VPN.'
              : 'Добавьте сервис, программу или сайт — они пойдут через VPN.'
            : isIos
              ? 'Добавьте то, что должно идти напрямую: банк, локальный сервис.'
              : 'Добавьте то, что должно идти напрямую: банк, игру, локальный сервис.'}</div>
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
  // iOS не говорит, какая программа открыла соединение, — только сайты.
  const [view, setView] = useState<LiveView>(isIos ? 'sites' : 'apps');
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
      <Back label="Маршруты" onClick={onBack} />
      <div className="cx-live-head">
        <div className="sub-title">Сейчас в сети</div>
        <button className={paused ? 'cx-pill' : 'cx-pill live'} onClick={() => setPaused(!paused)} disabled={!isRunning}>
          <i />
          {paused ? 'Пауза' : 'Live'}
        </button>
      </div>
      <div className="sub-lead">Тумблер включён — идёт через VPN. Правило сохранится в мой список.</div>
      {isIos ? null : (
        <Seg
          className="mt14"
          value={view}
          options={[
            ['apps', 'Программы'],
            ['sites', 'Сайты'],
          ]}
          onChange={setView}
        />
      )}
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
                          {isIos ? null : `${s.app} · `}
                          {locked ? 'по правилу программы' : s.vpn ? 'через VPN' : 'напрямую'}
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
      <Back label="Диагностика" onClick={onBack} />
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
