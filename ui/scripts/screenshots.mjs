// Скриншоты для README: `npm run dev` в одном окне, `npm run screenshots` в другом.
// Edge без окна открывает превью с тестовыми данными и снимает само окно kl!ck (без фона превью)
// в двойном разрешении. Результат — docs/screenshots/*.png.

import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const BASE = process.env.PREVIEW ?? 'http://127.0.0.1:5173';
const OUT = fileURLToPath(new URL('../../docs/screenshots/', import.meta.url));
const EDGE = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const PORT = 9351;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Имя файла, страница превью, что нажать перед снимком (aria-label или текст кнопки), какой блок снять.
const SHOTS = [
  ['home-dark', '/?s=connected', [], '.window'],
  ['home-light', '/?s=connected&theme=light', [], '.window'],
  ['connection-dark', '/?s=connected', ['Соединение'], '.window'],
  ['add-light', '/?s=connected&theme=light', ['Добавить'], '.window'],
  ['settings-dark', '/?s=connected', ['Настройки'], '.window'],
  ['killswitch-light', '/?s=connected&theme=light', ['Настройки', 'Kill Switch'], '.window'],
  ['tray-dark', '/?view=tray&s=connected', [], '.tray'],
  ['tray-light', '/?view=tray&s=connected&theme=light', [], '.tray'],
];

const profile = mkdtempSync(join(tmpdir(), 'klick-shots-'));
const edge = spawn(EDGE, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`, '--window-size=1000,1100', '--hide-scrollbars', 'about:blank'], { stdio: 'ignore' });

async function page() {
  for (let i = 0; i < 50; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const p = list.find((t) => t.type === 'page');
      if (p) return p;
    } catch {}
    await sleep(200);
  }
  throw new Error('Edge не открыл порт отладки');
}

const target = await page();
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r, j) => ((ws.onopen = r), (ws.onerror = j)));
let seq = 0;
const pending = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  }
};
const send = (method, params = {}) => new Promise((r) => { const id = ++seq; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
const js = async (expr) => (await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true })).result?.result?.value;

await send('Emulation.setDeviceMetricsOverride', { width: 1000, height: 1100, deviceScaleFactor: 2, mobile: false });
mkdirSync(OUT, { recursive: true });

try {
  for (const [name, path, clicks, sel] of SHOTS) {
    await send('Page.navigate', { url: BASE + path });
    await sleep(2500);
    for (const label of clicks) {
      const ok = await js(`(() => {
        const b = [...document.querySelectorAll('button, [role=button]')].find((e) => e.getAttribute('aria-label') === ${JSON.stringify(label)} || e.textContent.trim().startsWith(${JSON.stringify(label)}));
        if (!b) return false;
        b.click();
        return true;
      })()`);
      if (!ok) throw new Error(`${name}: не нашёл «${label}»`);
      await sleep(900);
    }
    // Всплывающие сообщения превью на снимке не нужны.
    await js(`document.querySelectorAll('.toast, .toasts').forEach((e) => e.remove())`);
    // Окно трея подгоняет высоту под содержимое: снимаем, когда размер перестал меняться.
    const rect = `(() => { scrollTo(0, 0); const b = document.querySelector(${JSON.stringify(sel)}).getBoundingClientRect(); return { x: b.x + scrollX, y: b.y + scrollY, width: b.width, height: b.height }; })()`;
    let r = await js(rect);
    for (let k = 0; k < 20; k++) {
      await sleep(300);
      const next = await js(rect);
      if (JSON.stringify(next) === JSON.stringify(r)) break;
      r = next;
    }
    const shot = await send('Page.captureScreenshot', { format: 'png', clip: { ...r, scale: 1 } });
    writeFileSync(join(OUT, `${name}.png`), Buffer.from(shot.result.data, 'base64'));
    console.log(`${name}.png  ${Math.round(r.width)}×${Math.round(r.height)}`);
  }
} finally {
  ws.close();
  edge.kill();
  await sleep(500);
  rmSync(profile, { recursive: true, force: true });
}
