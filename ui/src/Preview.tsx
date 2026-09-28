// Превью в браузере: окно посередине и переключатель состояний тестовой службы под ним.

import { useState, type ReactNode } from 'react';
import type { Scenario } from './lib/mock';

const SCENARIOS: [Scenario, string][] = [
  ['empty', 'Нет подключений'],
  ['off', 'Выключен'],
  ['connected', 'Подключён'],
  ['reconnecting', 'Переподключение'],
  ['down', 'Сервер не отвечает'],
  ['error', 'Ошибка'],
  ['neighbors', 'Мешает zapret'],
];

export function Preview({ children }: { children: ReactNode }) {
  const initial = (new URLSearchParams(location.search).get('s') as Scenario) || 'off';
  const [current, setCurrent] = useState<Scenario>(initial);
  const pick = (s: Scenario) => {
    setCurrent(s);
    (window as unknown as { __klickMock?: { setScenario(s: Scenario): void } }).__klickMock?.setScenario(s);
    const url = new URL(location.href);
    url.searchParams.set('s', s);
    history.replaceState(null, '', url);
  };
  const tray = new URLSearchParams(location.search).get('view') === 'tray';
  // Окно выбирается при запуске транспорта, поэтому переключение — перезагрузкой.
  const switchView = () => {
    const url = new URL(location.href);
    if (tray) url.searchParams.delete('view');
    else url.searchParams.set('view', 'tray');
    location.assign(url);
  };
  return (
    <div className="preview">
      {children}
      <div className="preview-bar">
        <span>Превью с тестовыми данными — служба не нужна</span>
        {SCENARIOS.map(([key, label]) => (
          <button key={key} className={key === current ? 'on' : ''} onClick={() => pick(key)}>
            {label}
          </button>
        ))}
        <button className="view" onClick={switchView}>
          {tray ? 'Главное окно' : 'Окно трея'}
        </button>
      </div>
    </div>
  );
}
