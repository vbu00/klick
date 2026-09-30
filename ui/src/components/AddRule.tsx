// Лист «Добавить в список» по макету v4: Сервисы · Программы · Сайт или IP. Правила встают в список
// текущего положения тумблера: в «Только выбранное» — через VPN, во «Всё через VPN» — напрямую.

import { useEffect, useState } from 'react';
import { routeName } from '../lib/i18n';
import { isMac } from '../lib/platform';
import { defaultRoute } from '../lib/rules';
import { useStore } from '../lib/store';
import type { ProgramView, Routing, Rule, Target } from '../lib/types';
import { Sheet } from './Chrome';
import { Seg, Tile } from './Controls';
import { Icon } from './Icon';

type Tab = 'services' | 'programs' | 'site';

const same = (a: Target, kind: Target['kind'], value: string) => a.kind === kind && a.value.toLowerCase() === value.toLowerCase();

function Check({ on }: { on: boolean }) {
  return (
    <span className={on ? 'ax-check on' : 'ax-check'}>
      {on ? (
        <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M2.5 6.2l2.3 2.3 4.7-5" />
        </svg>
      ) : null}
    </span>
  );
}

export function AddRule({ position, onClose }: { position: Routing; onClose: () => void }) {
  const store = useStore();
  const { catalog, settings, transport } = store;
  const list = settings?.lists[position] ?? [];
  const route = defaultRoute[position];
  const [tab, setTab] = useState<Tab>('services');
  const [services, setServices] = useState<string[]>([]);
  const [programs, setPrograms] = useState<string[]>([]);
  const [running, setRunning] = useState<ProgramView[] | null>(null);
  const [site, setSite] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void transport
      .call<ProgramView[]>('programs')
      .then(setRunning)
      .catch(() => setRunning([]));
  }, [transport]);

  const inList = (kind: Target['kind'], value: string) => list.some((r) => same(r.target, kind, value));
  const freeServices = catalog.filter((s) => !inList('service', s.id));
  const freePrograms = (running ?? []).filter((p) => !(p.folder && inList('program', p.folder)));

  const siteItems = site
    .split(/[\s,;]+/)
    .map((s) => s.trim())
    .filter(Boolean);
  const count = services.length + programs.length + siteItems.length;

  const add = async (rules: Rule[]) => {
    if (!rules.length || busy) return;
    setBusy(true);
    const added = await store.addRules(position, rules);
    setBusy(false);
    if (added > 0) {
      store.toast(added === 1 ? 'Добавлено в список' : `Добавлено правил: ${added}`, `Пойдёт ${routeName[route]}`, 'ok');
      onClose();
    }
  };

  const submit = () =>
    void add([
      ...services.map((id): Rule => ({ target: { kind: 'service', value: id }, route })),
      ...programs.map((path): Rule => ({ target: { kind: 'program', value: path }, route })),
      ...siteItems.map((v): Rule => ({ target: /^[\d.]+(\/\d+)?$|:/.test(v) ? { kind: 'ip', value: v } : { kind: 'domain', value: v }, route })),
    ]);

  const browse = async () => {
    const path = await transport.pickExe();
    if (path) void add([{ target: { kind: 'program', value: path }, route }]);
  };

  const toggle = (set: (f: (prev: string[]) => string[]) => void, v: string) => set((prev) => (prev.includes(v) ? prev.filter((x) => x !== v) : [...prev, v]));

  return (
    <Sheet onClose={onClose} tall>
      <div className="ax-head">
        <div className="grow">
          <h3>Добавить в «{position === 'selected' ? 'Через VPN' : 'Напрямую'}»</h3>
          <p>{position === 'selected' ? 'Выбранное пойдёт через VPN, остальное — напрямую.' : 'Выбранное пойдёт мимо VPN: банк, игра, локальный сервис.'}</p>
        </div>
        <button className="ax-close" aria-label="Закрыть" onClick={onClose}>
          <Icon name="winClose" size={12} />
        </button>
      </div>
      <Seg
        className="ax-tabs"
        value={tab}
        options={[
          ['services', 'Сервисы'],
          ['programs', 'Программы'],
          ['site', 'Сайт или IP'],
        ]}
        onChange={setTab}
      />

      <div className="ax-body">
        {tab === 'services' ? (
          <div className="ax-grid">
            {freeServices.map((s) => {
              const on = services.includes(s.id);
              return (
                <button key={s.id} className={on ? 'ax-svc on' : 'ax-svc'} onClick={() => toggle(setServices, s.id)}>
                  <Tile label={s.name} size={26} />
                  <span className="ax-name">{s.name}</span>
                  <Check on={on} />
                </button>
              );
            })}
            {!freeServices.length ? <div className="ax-empty">Все сервисы каталога уже в списке</div> : null}
          </div>
        ) : null}

        {tab === 'programs' ? (
          <>
            <div className="ax-caps">Запущены сейчас</div>
            <div className="ax-card">
              {running === null ? <div className="ax-empty">Смотрю, кто в сети…</div> : null}
              {running && !freePrograms.length ? <div className="ax-empty">Все запущенные программы уже в списке</div> : null}
              {freePrograms.map((p) => {
                const on = programs.includes(p.path);
                const shared = p.folder == null;
                return (
                  <button key={p.path} className="ax-app" disabled={shared} title={shared ? 'Программа лежит в общей папке — перенесите её в свою' : p.folder ?? undefined} onClick={() => toggle(setPrograms, p.path)}>
                    <Tile label={p.name} size={28} />
                    <span className="ax-name">
                      {p.name}
                      {shared ? <small>в общей папке</small> : null}
                    </span>
                    <Check on={on} />
                  </button>
                );
              })}
            </div>
            <button className="ax-browse" onClick={() => void browse()}>
              {isMac ? 'Выбрать программу…' : 'Выбрать .exe…'}
            </button>
          </>
        ) : null}

        {tab === 'site' ? (
          <>
            <input className="ax-field" autoFocus value={site} onChange={(e) => setSite(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && submit()} placeholder="example.com или 1.2.3.4" spellCheck={false} />
            <div className="ax-note">Поддомены включаются автоматически.</div>
          </>
        ) : null}
      </div>

      <div className="ax-foot">
        <button className={count ? 'ax-go on' : 'ax-go'} disabled={count === 0 || busy} onClick={submit}>
          {busy ? 'Добавляю…' : count ? `Добавить · ${count}` : 'Добавить'}
        </button>
      </div>
    </Sheet>
  );
}
