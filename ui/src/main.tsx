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
  // иначе они 60 раз в секунду дёргают видеокарту, которая нужна не нам. Разовые (появление
  // экрана, всплывающие уведомления) не трогаем: остановленные на первом кадре, они прозрачны.
  const syncLoops = () => {
    const idle = !document.hasFocus();
    for (const a of document.getAnimations()) {
      if (a.effect?.getTiming().iterations !== Infinity) continue;
      if (idle) a.pause();
      else if (a.playState === 'paused') a.play();
    }
  };
  window.addEventListener('focus', syncLoops);
  window.addEventListener('blur', syncLoops);
  document.addEventListener('animationstart', syncLoops);
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
