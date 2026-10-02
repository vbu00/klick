import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { StoreProvider } from './lib/store';
import { PLATFORM } from './lib/platform';
import { applyCachedTheme } from './lib/theme';
import { createTransport } from './lib/transport';
import { Preview } from './Preview';
import './styles/theme.css';
import './styles/app.css';
import './styles/connection.css';
import './styles/cx.css';
import './styles/settings.css';
import './styles/tray.css';

// Сразу последняя тема, пока служба не прислала настройки.
applyCachedTheme();

createTransport().then((transport) => {
  if (transport.kind === 'tauri') document.body.classList.add('in-tauri');
  // Окно не в фокусе (сверху игра или другая программа) — бесконечные анимации на паузе:
  // иначе они 60 раз в секунду дёргают видеокарту, которая нужна не нам.
  const idle = () => document.body.classList.toggle('idle', !document.hasFocus());
  window.addEventListener('focus', idle);
  window.addEventListener('blur', idle);
  idle();
  document.body.classList.add(`os-${PLATFORM}`);
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <StoreProvider transport={transport}>
        {transport.kind === 'mock' ? (
          <Preview>
            <App />
          </Preview>
        ) : (
          <App />
        )}
      </StoreProvider>
    </StrictMode>,
  );
});
