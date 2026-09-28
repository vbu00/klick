// Окно и трей kl!ck через протокол отладки WebView2 (порт 9341 — ui-check.ps1 задаёт его
// переменной WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS): снимки страниц, размеры, и трей открывается
// той же командой, что и щелчок по значку.

import { writeFileSync } from 'node:fs';

const PORT = 9341;
const OUT = 'C:\\klick\\results';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function targets() {
  for (let i = 0; i < 60; i++) {
    try {
      const list = (await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()).filter((t) => t.type === 'page');
      if (list.length) return list;
    } catch {}
    await sleep(500);
  }
  throw new Error('WebView2 не открыл порт отладки');
}

async function attach(target) {
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
  const evalJs = async (expr) => (await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true })).result?.result?.value;
  const shot = async (name) => {
    const r = await send('Page.captureScreenshot', { format: 'png' });
    if (r.result?.data) writeFileSync(`${OUT}\\${name}.png`, Buffer.from(r.result.data, 'base64'));
    return !!r.result?.data;
  };
  return { send, evalJs, shot, close: () => ws.close() };
}

const geometry = `JSON.stringify({ x: window.screenX, y: window.screenY, w: window.outerWidth, h: window.outerHeight, inner: [innerWidth, innerHeight], scroll: document.documentElement.scrollHeight, dpr: devicePixelRatio, screen: [screen.availWidth, screen.availHeight] })`;

// Окна — одна страница; какое где, знает Tauri: имя окна (main / tray).
async function byLabel(label) {
  for (const t of await targets()) {
    const c = await attach(t);
    const l = await c.evalJs(`window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? ''`);
    if (l === label) return c;
    c.close();
  }
  return null;
}

const m = await byLabel('main');
if (!m) throw new Error('нет страницы главного окна');
console.log('главное окно: ' + (await m.evalJs(geometry)));
console.log('кнопки заголовка: ' + (await m.evalJs(`[...document.querySelectorAll('header button, .tb-btn')].map(b => b.getAttribute('aria-label')).join(', ')`)));
console.log('снимок главного окна: ' + (await m.shot('page-main')));

// Трей — как по щелчку на значке.
await m.evalJs(`window.__TAURI_INTERNALS__.invoke('open_tray').then(() => 'ok', e => String(e))`);
await sleep(2500);
const t = await byLabel('tray');
if (!t) {
  console.log('FAIL окно трея не нашлось');
} else {
  console.log('трей: ' + (await t.evalJs(geometry)));
  console.log('снимок трея: ' + (await t.shot('page-tray')));
  t.close();
}
m.close();
