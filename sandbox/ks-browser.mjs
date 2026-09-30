// Edge в Kill Switch: много раз включаем и выключаем VPN в режиме «Системный прокси» и каждый раз
// открываем страницу в одном и том же, всё время работающем Edge. Рядом — что видит Windows:
// значения в реестре (их пишет kl!ck) и итоговая запись DefaultConnectionSettings (её читают
// Chrome и Edge через WinHTTP), а также что видит .NET (читает ту же запись).

import { spawn, execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const CLI = 'C:\\Program Files\\klick\\klick-cli.exe';
const EDGE = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const URL = 'https://www.gstatic.com/generate_204';
const PORT = 9333;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const rows = [];

function cli(...args) {
  try {
    return execFileSync(CLI, ['--prod', ...args], { encoding: 'utf8', timeout: 60000 }).trim();
  } catch (e) {
    return `ошибка: ${(e.stdout || '') + (e.stderr || '')}`.trim();
  }
}

function regValue(key, name) {
  try {
    const out = execFileSync('reg.exe', ['query', key, '/v', name], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    const m = out.match(new RegExp(`${name}\\s+REG_\\w+\\s+(.*)`));
    return m ? m[1].trim() : null;
  } catch {
    return null;
  }
}

// DefaultConnectionSettings: версия, счётчик, флаги (1 — напрямую, 2 — прокси, 4 — PAC, 8 — автообнаружение),
// длина и строка прокси.
function blob() {
  const hex = regValue('HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\\Connections', 'DefaultConnectionSettings');
  if (!hex) return { flags: null, proxy: null, counter: null };
  const b = Buffer.from(hex, 'hex');
  const counter = b.readUInt32LE(4);
  const flags = b.readUInt32LE(8);
  const len = b.readUInt32LE(12);
  const proxy = b.subarray(16, 16 + len).toString('latin1');
  return { flags, proxy, counter };
}

function state() {
  const k = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings';
  const enable = regValue(k, 'ProxyEnable');
  const server = regValue(k, 'ProxyServer');
  const bl = blob();
  let net = null;
  try {
    net = execFileSync('powershell.exe', ['-NoProfile', '-Command', "[System.Net.WebRequest]::GetSystemWebProxy().GetProxy([uri]'https://www.gstatic.com/').Authority"], { encoding: 'utf8', timeout: 30000 }).trim();
  } catch {}
  return { reg: `${enable === '0x1' ? 'вкл' : 'выкл'} ${server ?? '-'}`, blob: `флаги ${bl.flags} ${bl.flags & 2 ? bl.proxy : 'напрямую'} (№${bl.counter})`, net };
}

async function cdp() {
  let target;
  for (let i = 0; i < 100 && !target; i++) {
    await sleep(300);
    try {
      target = (await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()).find((t) => t.type === 'page');
    } catch {}
  }
  if (!target) throw new Error('Edge не открыл порт отладки');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r, j) => ((ws.onopen = r), (ws.onerror = j)));
  let seq = 0;
  const pending = new Map();
  const listeners = [];
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    } else if (msg.method) listeners.forEach((l) => l(msg));
  };
  // Не ждать Edge вечно: зависший ответ по протоколу отладки — ошибка шага, а не зависший сценарий.
  const send = (method, params = {}, ms = 30000) =>
    new Promise((r, j) => {
      const id = ++seq;
      const timer = setTimeout(() => { pending.delete(id); j(new Error(`${method}: Edge не ответил за ${ms / 1000} с`)); }, ms);
      pending.set(id, (msg) => { clearTimeout(timer); r(msg); });
      ws.send(JSON.stringify({ id, method, params }));
    });
  await send('Page.enable');
  await send('Network.enable');
  await send('Network.setCacheDisabled', { cacheDisabled: true });
  return {
    // Открыть адрес; вернуть «ok <код ответа>» или текст ошибки сети. Ответ 204 браузер не
    // показывает и помечает переход как net::ERR_ABORTED — поэтому смотрим, пришёл ли ответ.
    async open(url) {
      let failed = null;
      let status = null;
      const on = (msg) => {
        if (msg.method === 'Network.responseReceived' && msg.params.type === 'Document') status = msg.params.response.status;
        if (msg.method === 'Network.loadingFailed' && msg.params.type === 'Document' && msg.params.errorText !== 'net::ERR_ABORTED') failed = msg.params.errorText;
      };
      listeners.push(on);
      const sep = url.includes('?') ? '&' : '?';
      let r;
      try {
        r = await send('Page.navigate', { url: `${url}${sep}t=${Date.now()}` });
      } catch (e) {
        listeners.splice(listeners.indexOf(on), 1);
        return e.message;
      }
      const navErr = r.result?.errorText && r.result.errorText !== 'net::ERR_ABORTED' ? r.result.errorText : null;
      const deadline = Date.now() + 20000;
      while (!status && !failed && !navErr && Date.now() < deadline) await sleep(200);
      listeners.splice(listeners.indexOf(on), 1);
      if (status) return `ok ${status}`;
      return navErr || failed || 'нет ответа за 20 с';
    },
  };
}

async function step(name, action, expect, url = URL) {
  console.log(`… ${name}`);
  if (action) action();
  await sleep(2500);
  const st = state();
  const res = await page.open(url);
  const good = expect === 'ok' ? res.startsWith('ok') : !res.startsWith('ok');
  const row = { name, res, expect, good, ...st };
  rows.push(row);
  console.log(`${good ? 'OK  ' : 'FAIL'} ${name}: ${res} | реестр: ${st.reg} | запись Windows: ${st.blob} | .NET: ${st.net}`);
}

const edge = spawn(EDGE, ['--headless=new', `--remote-debugging-port=${PORT}`, '--user-data-dir=C:\\edgeprof', '--no-first-run', '--no-default-browser-check', 'about:blank'], { stdio: 'ignore' });
let page;
try {
  page = await cdp();
  console.log('Edge работает; режим «Системный прокси», Edge в Kill Switch');
  await step('VPN выкл', null, 'blocked');
  // А. Включить и выключить много раз
  for (let i = 1; i <= 8; i++) {
    await step(`${i}: VPN вкл (прокси)`, () => cli('connect'), 'ok');
    await step(`${i}: VPN выкл`, () => cli('disconnect'), 'blocked');
  }
  // Б. Как у друга: TUN, потом на ходу «Системный прокси»
  for (let i = 1; i <= 4; i++) {
    cli('mode', 'tun');
    await step(`Б${i}: VPN вкл (TUN)`, () => cli('connect'), 'ok');
    await step(`Б${i}: на ходу → системный прокси`, () => cli('mode', 'proxy'), 'ok');
    await step(`Б${i}: VPN выкл`, () => cli('disconnect'), 'blocked');
  }
  // В. Быстрые переключения без пауз, потом проверка
  for (let i = 0; i < 5; i++) {
    cli('connect');
    cli('disconnect');
  }
  await step('В: после быстрых переключений — VPN вкл', () => cli('connect'), 'ok');
  await step('В: VPN выкл', () => cli('disconnect'), 'blocked');
  // Г. Настоящая страница со множеством запросов и выключение Kill Switch для Edge на ходу
  await step('Г: VPN вкл, google.com', () => cli('connect'), 'ok', 'https://www.google.com/');
  await step('Г: Edge выключен в Kill Switch', () => cli('ks', 'program', 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application', 'off'), 'ok', 'https://www.google.com/');
  await step('Г: Edge снова в Kill Switch', () => cli('ks', 'program', 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application', 'on'), 'ok', 'https://www.google.com/');
  await step('Г: VPN выкл', () => cli('disconnect'), 'blocked', 'https://www.google.com/');
} catch (e) {
  console.log(`ошибка: ${e.message}`);
} finally {
  writeFileSync('C:\\klick\\results\\ks-browser.json', JSON.stringify(rows, null, 2));
  const bad = rows.filter((r) => !r.good).length;
  console.log(`ИТОГ: шагов ${rows.length}, провалов ${bad}`);
  edge.kill();
}
