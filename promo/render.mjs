// Рендер роликов: страница в безголовом Edge → WebCodecs (H.264 + AAC) → MP4.
//   node render.mjs                         → out/klick-tiktok.mp4 + out/cover.png        («POV: катка», story.html)
//   node render.mjs flythrough              → out/klick-flythrough.mp4 + out/cover-flythrough.png
//   node render.mjs [flythrough] --stills 1,5.5,9   → out/still-*.png для проверки кадров
// Страница открывается с локального HTTP-сервера, а не с file://: с file://
// картинки «грязнят» canvas, и из него нельзя сделать VideoFrame.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer-core';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const OUT = path.join(HERE, 'out');
const EDGE = 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.png': 'image/png', '.svg': 'image/svg+xml', '.css': 'text/css' };
const VIDEOS = {
  story: { page: 'story.html', mp4: 'klick-tiktok.mp4', cover: 'cover.png' },
  flythrough: { page: 'flythrough.html', mp4: 'klick-flythrough.mp4', cover: 'cover-flythrough.png' },
};
const video = VIDEOS[process.argv[2]] || VIDEOS.story;

const server = http.createServer((req, res) => {
  const rel = decodeURIComponent(req.url.split('?')[0]);
  const file = path.join(ROOT, rel);
  if (!file.startsWith(ROOT)) return res.writeHead(403).end();
  fs.readFile(file, (err, buf) => {
    if (err) return res.writeHead(404).end();
    res.writeHead(200, { 'Content-Type': TYPES[path.extname(file)] || 'application/octet-stream' });
    res.end(buf);
  });
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const url = `http://127.0.0.1:${server.address().port}/promo/${video.page}?render=1`;

fs.mkdirSync(OUT, { recursive: true });
const browser = await puppeteer.launch({ executablePath: EDGE, headless: true, args: ['--hide-scrollbars', '--force-device-scale-factor=1'] });
try {
  const page = await browser.newPage();
  page.on('console', (m) => console.log('  [страница]', m.text()));
  page.on('pageerror', (e) => console.error('  [ошибка]', e.message));
  await page.setViewport({ width: 1080, height: 1920 });
  await page.goto(url);
  await page.waitForFunction('window.storyReady === true', { timeout: 30000 });

  const i = process.argv.indexOf('--stills');
  if (i > 0) {
    for (const t of process.argv[i + 1].split(',')) {
      const b64 = await page.evaluate((t) => window.frameAt(t), Number(t));
      fs.writeFileSync(path.join(OUT, `still-${t}.png`), Buffer.from(b64, 'base64'));
      console.log('кадр', t);
    }
  } else {
    await page.exposeFunction('reportProgress', (i, n) => process.stdout.write(`\r  кадр ${i}/${n}`));
    const started = Date.now();
    const { b64, audioCodec } = await page.evaluate(() => window.encodeStory((i, n) => window.reportProgress(i, n)));
    const file = path.join(OUT, video.mp4);
    fs.writeFileSync(file, Buffer.from(b64, 'base64'));
    // Обложка — кадр с логотипом (момент задаёт страница, по умолчанию 11 с).
    fs.writeFileSync(path.join(OUT, video.cover), Buffer.from(await page.evaluate(() => window.frameAt(window.COVER_T ?? 11.0)), 'base64'));
    console.log(`\nготово за ${((Date.now() - started) / 1000).toFixed(0)} с: ${file} (${(fs.statSync(file).size / 1e6).toFixed(1)} МБ, звук ${audioCodec})`);
  }
} finally {
  await browser.close();
  server.close();
}
