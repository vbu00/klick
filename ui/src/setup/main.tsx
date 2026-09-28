import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createApi } from './api';
import { Setup } from './Setup';
import './setup.css';

createApi().then(async (api) => {
  if (!api.inTauri) document.body.classList.add('preview');
  const hello = await api.hello();
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <Setup api={api} hello={hello} />
    </StrictMode>,
  );
});
