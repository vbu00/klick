// «Запущено сейчас»: программы с открытыми соединениями, поиск и отметки.
// Общий для листа «Добавить в список» и выбора программ Kill Switch.

import { useEffect, useMemo, useState } from 'react';
import { shortPath } from '../lib/rules';
import { useStore } from '../lib/store';
import type { ProgramView } from '../lib/types';
import { Tile } from './Controls';

export function ProgramList({ selected, onToggle, taken }: { selected: string[]; onToggle: (path: string) => void; taken: (folder: string) => boolean }) {
  const { transport } = useStore();
  const [running, setRunning] = useState<ProgramView[] | null>(null);
  const [query, setQuery] = useState('');

  useEffect(() => {
    void transport
      .call<ProgramView[]>('programs')
      .then(setRunning)
      .catch(() => setRunning([]));
  }, [transport]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (running ?? []).filter((p) => !q || p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q));
  }, [running, query]);

  return (
    <>
      <input className="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Поиск по названию или .exe" />
      <div className="pick-head">
        <span className="caps">Запущено сейчас · {running ? running.length : '…'}</span>
        <span className="pick-hint">по сетевой активности</span>
      </div>
      {running === null ? <div className="pick-empty">Смотрю, кто в сети…</div> : null}
      {running && shown.length === 0 ? (
        <div className="pick-empty">
          <b>Ничего не найдено</b>
          Программа не запущена? Выберите её .exe вручную через «Обзор…».
        </div>
      ) : null}
      {shown.map((p) => {
        const isTaken = p.folder != null && taken(p.folder);
        const on = selected.includes(p.path);
        return (
          <button key={p.path} className={on ? 'pick on' : 'pick'} disabled={isTaken || p.folder == null} onClick={() => onToggle(p.path)}>
            <span className="check">
              <i />
            </span>
            <Tile label={p.name} />
            <span className="row-text">
              <span className="pick-name">{p.name}</span>
              <span className="pick-sub">{p.folder == null ? 'в общей папке — перенесите в свою' : isTaken ? 'уже в списке' : shortPath(p.folder)}</span>
            </span>
            <span className="pick-act">{p.connections}</span>
          </button>
        );
      })}
    </>
  );
}
