import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Окно kl!ck и установщик (setup.html): в Tauri страницы грузятся из dist, для разработки и превью — с этого сервера.
export default defineConfig({
  plugins: [react()],
  // Дата сборки для «О приложении».
  define: { __BUILD_DATE__: JSON.stringify(new Date().toLocaleDateString('ru-RU')) },
  clearScreen: false,
  // Опрос файлов: системные уведомления Windows теряют вторую из двух быстрых правок подряд.
  server: { port: 5173, strictPort: true, host: '127.0.0.1', watch: { usePolling: true, interval: 250 } },
  build: {
    target: 'es2022',
    outDir: 'dist',
    emptyOutDir: true,
    rolldownOptions: { input: { main: 'index.html', setup: 'setup.html' } },
  },
});
