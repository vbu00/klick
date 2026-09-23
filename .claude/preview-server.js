// Локальный статический сервер только для визуальной проверки вёрстки.
const http = require('http');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const TYPES = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.png': 'image/png', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' };

http
  .createServer((req, res) => {
    const rel = decodeURIComponent(req.url.split('?')[0]);

    // /src/mock отдаёт настоящий index.html, где вместо моста tauri-bridge.js
    // подставлен стаб window.kl: приложение рисуется на правдоподобных
    // данных, без Tauri.
    if (rel === '/src/mock' || rel === '/src/mock/') {
      const html = fs.readFileSync(path.join(ROOT, 'src/index.html'), 'utf8');
      const mockJs = fs.readFileSync(path.join(__dirname, 'mock-kl.js'), 'utf8');
      // Мост именно ЗАМЕНЯЕМ, а не добавляем стаб следом: без window.__TAURI__
      // он падает на первой же строке, и консоль стенда была забита его
      // ошибками — на их фоне настоящие проблемы renderer.js не разглядеть.
      const withMock = html.includes('<script src="tauri-bridge.js"></script>')
        ? html.replace('<script src="tauri-bridge.js"></script>', `<script>${mockJs}</script>`)
        : html.replace(
            '<script src="app.js"></script>',
            `<script>${mockJs}</script>\n<script src="app.js"></script>`
          );
      res.writeHead(200, { 'Content-Type': 'text/html' });
      res.end(withMock);
      return;
    }

    const file = path.join(ROOT, rel === '/' ? 'src/index.html' : rel);
    if (!file.startsWith(ROOT)) {
      res.writeHead(403).end();
      return;
    }
    fs.readFile(file, (err, buf) => {
      if (err) {
        res.writeHead(404).end('not found');
        return;
      }
      res.writeHead(200, { 'Content-Type': TYPES[path.extname(file)] || 'application/octet-stream' });
      res.end(buf);
    });
  })
  .listen(4176, () => console.log('preview on http://localhost:4176/src/mock'));
