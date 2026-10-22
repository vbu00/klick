// Серверы по странам: строка страны (флаг, название, сколько серверов, протоколы, лучшая задержка),
// по нажатию — её серверы. Включается в настройках («Серверы по странам»).
// Порядок стран не зависит от задержек: замер приходит разом и не должен переставлять строки под курсором.

import { useMemo, useState, type KeyboardEvent } from 'react';
import { groupByCountry, groupTags, parseServer, type ParsedServer } from '../lib/countries';
import { pingColor, protocolName } from '../lib/format';
import { plural } from '../lib/rules';
import type { ServerView } from '../lib/types';
import { Flag } from './Flag';
import { Icon } from './Icon';

/** Раскрытые страны живут, пока открыто окно: закрыли и открыли список — всё как было. */
let remembered: Set<string> | null = null;

const serverTitle = (s: ServerView, p: ParsedServer) => p.title || protocolName(s.kind);

/** Задержка: «…» во время замера, пусто до первого замера — null тогда не значит «нет ответа». */
function Ping({ delay, pinging, tested }: { delay: number | null | undefined; pinging: boolean; tested: boolean }) {
  if (pinging) return <span className="num cg-ping dim">…</span>;
  if (delay != null)
    return (
      <span className="num cg-ping" style={{ color: pingColor(delay) }}>
        {delay} мс
      </span>
    );
  return tested ? <span className="cg-nr">нет ответа</span> : <span className="num cg-ping" />;
}

export function ServerCountries(props: { list: ServerView[]; current: ServerView | undefined; pinging: boolean; tested: boolean; onPick: (name: string) => void }) {
  const { list, current, pinging, tested } = props;
  const cur = current ? parseServer(current.name, current.kind) : null;
  const curKey = cur ? (cur.code ?? '') : null;
  // Страна выбранного сервера — сверху, но только на момент открытия: выбор в другой стране строки не двигает.
  const [pin] = useState(curKey);
  const [open, setOpen] = useState<Set<string>>(() => (remembered ??= curKey !== null ? new Set([curKey]) : null) ?? new Set());
  const groups = useMemo(() => groupByCountry(list, pin), [list, pin]);

  const toggle = (key: string) => {
    const next = new Set(open);
    if (!next.delete(key)) next.add(key);
    remembered = next;
    setOpen(next);
  };

  // Стрелки: вверх-вниз — по строкам, вправо — раскрыть страну, влево — свернуть или вернуться к стране.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const items = [...e.currentTarget.querySelectorAll<HTMLButtonElement>('[data-nav]')].filter((b) => !b.closest('[inert]'));
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    const el = items[i];
    const key = el.dataset.key ?? '';
    const isCountry = el.dataset.nav === 'country';
    const focus = (b: HTMLButtonElement | undefined) => b?.focus();
    if (e.key === 'ArrowDown') focus(items[i + 1]);
    else if (e.key === 'ArrowUp') focus(items[i - 1]);
    else if (e.key === 'Home') focus(items[0]);
    else if (e.key === 'End') focus(items[items.length - 1]);
    else if (e.key === 'ArrowRight' && isCountry) {
      if (open.has(key)) focus(items[i + 1]);
      else toggle(key);
    } else if (e.key === 'ArrowLeft' && isCountry) {
      if (open.has(key)) toggle(key);
    } else if (e.key === 'ArrowLeft') focus(items.find((b) => b.dataset.nav === 'country' && b.dataset.key === key));
    else return;
    e.preventDefault();
  };

  return (
    <div className="cg-list" onKeyDown={onKey}>
      {groups.map((g) => {
        const isOpen = open.has(g.key);
        const isCur = g.key === curKey;
        const delays = g.items.map((i) => i.server.delay).filter((d): d is number => d != null);
        const best = delays.length ? Math.min(...delays) : null;
        const n = g.items.length;
        const id = `cg-${g.key || 'other'}`;
        return (
          <div key={g.key} className={`cg${isOpen ? ' open' : ''}${isCur ? ' cur' : ''}`}>
            <button
              className="cg-head"
              data-nav="country"
              data-key={g.key}
              aria-expanded={isOpen}
              aria-controls={id}
              aria-label={`${g.name}: ${plural(n, ['сервер', 'сервера', 'серверов'])}${isCur && current ? `, выбран ${serverTitle(current, cur!)}` : ''}${best != null && !pinging ? `, лучшая задержка ${best} мс` : ''}`}
              onClick={() => toggle(g.key)}
            >
              <Flag code={g.code} />
              <span className="cg-text">
                <span className="cg-title">
                  <span className="cg-name">{g.name}</span>
                  <span className="cg-n">{n}</span>
                </span>
                <span className="cg-sub">
                  {isCur && current && cur ? (
                    <span className="cg-cur">
                      <i />
                      {serverTitle(current, cur)}
                    </span>
                  ) : (
                    groupTags(g).map((t) => (
                      <span key={t} className="cg-tag">
                        {t}
                      </span>
                    ))
                  )}
                </span>
              </span>
              <Ping delay={best} pinging={pinging} tested={tested} />
              <Icon name="chevron" size={14} className="cg-chev" />
            </button>
            <div className="cg-body" id={id} role="group" aria-label={g.name} inert={!isOpen}>
              <div className="cg-inner">
                {g.items.map(({ server: s, p }) => {
                  const on = s.name === current?.name;
                  const title = serverTitle(s, p);
                  const proto = p.title && !p.named ? protocolName(s.kind) : null;
                  return (
                    <button
                      key={s.name}
                      className={`cs${on ? ' on' : ''}${tested && !pinging && s.delay == null ? ' mute' : ''}`}
                      data-nav="server"
                      data-key={g.key}
                      aria-current={on || undefined}
                      title={s.name}
                      onClick={() => !on && props.onPick(s.name)}
                    >
                      <span className="radio">
                        <i />
                      </span>
                      <span className="cs-title">{title}</span>
                      {proto ? <span className="cg-tag">{proto}</span> : null}
                      <Ping delay={s.delay} pinging={pinging} tested={tested} />
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
