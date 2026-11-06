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

// Новое окно по макету «kl!ck — интерактивный прототип»: проверки и снимок каждого экрана.
const ok = (name, cond, detail = '') => console.log(`${cond ? 'OK  ' : 'FAIL'} ${name}${detail ? ' - ' + detail : ''}`);
const call = (cmd, args) => m.evalJs(`window.__TAURI_INTERNALS__.invoke('service_call', { request: ${JSON.stringify(args ? { cmd, args } : { cmd })} }).then((v) => ({ ok: true, v }), (e) => ({ ok: false, e }))`);
const click = (sel, text) => m.evalJs(`(() => {
  const all = [...document.querySelectorAll(${JSON.stringify(sel)})];
  const b = ${text ? `all.find((e) => e.textContent.trim().startsWith(${JSON.stringify(text)}))` : 'all[0]'};
  if (!b) return false;
  b.click();
  return true;
})()`);
// Анимации появления — мгновенно: снимок не должен поймать экран на середине.
await m.evalJs(`(() => { const s = document.createElement('style'); s.textContent = '*,*::before,*::after{animation-duration:0s!important;animation-delay:0s!important;transition-duration:0s!important;transition-delay:0s!important}'; document.head.appendChild(s); return true; })()`);
await sleep(1500);

const look = JSON.parse(await m.evalJs(`JSON.stringify((() => {
  const r = document.querySelector('.bm-root');
  const b = r?.getBoundingClientRect();
  return { root: !!r, w: b?.width, h: b?.height, inter: document.fonts.check('13px "Inter Variable"'), font: r ? getComputedStyle(r).fontFamily : '', bg: r ? getComputedStyle(r).backgroundColor : '', accent: r ? getComputedStyle(r).getPropertyValue('--accent') : '', caps: [...document.querySelectorAll('.bm-root .cap')].map((x) => x.getAttribute('aria-label')).join(', ') };
})())`));
ok('окно по макету открылось', look.root);
const scr = JSON.parse(await m.evalJs(geometry)).screen[1];
ok('окно 382×797 (ниже — только на низком экране)', look.w === 382 && (look.h === 797 || (look.h < 797 && scr < 830)), `${look.w}×${look.h}, экран ${scr}`);
ok('шрифт Inter загружен', look.inter, look.font);
ok('кнопки заголовка', look.caps === 'Свернуть, Закрыть в трей', look.caps);
console.log(`тема: фон ${look.bg}, акцент ${look.accent}`);

// Новые команды службы.
const st = (await call('settings'))?.v;
const first = st?.connections?.[0];
if (first) {
  const r1 = await call('rename_connection', { id: first.id, name: 'Переименовано' });
  const after = (await call('settings'))?.v?.connections?.find((c) => c.id === first.id)?.name;
  ok('rename_connection', r1?.ok && after === 'Переименовано', `было «${first.name}», стало «${after}»`);
  await call('rename_connection', { id: first.id, name: first.name });
  const r2 = await call('rename_connection', { id: first.id, name: '   ' });
  ok('пустое название не принимается', !r2?.ok && r2?.e?.code === 'input.empty_name', JSON.stringify(r2?.e));
  const link = await call('connection_link', { id: first.id });
  if (first.kind === 'file') ok('у файла ссылки нет', !link?.ok && link?.e?.code === 'conn.no_link', JSON.stringify(link?.e));
  else ok('connection_link отдаёт ссылку', link?.ok && typeof link.v === 'string' && link.v.includes('://'), link?.ok ? `${String(link.v).split('://')[0]}://…` : JSON.stringify(link?.e));
  const file = st.connections.find((c) => c.kind === 'file');
  if (file && file !== first) {
    const l2 = await call('connection_link', { id: file.id });
    ok('у файла ссылки нет', !l2?.ok && l2?.e?.code === 'conn.no_link', JSON.stringify(l2?.e));
  }
} else ok('есть подключения для проверки команд', false);
const d = await call('delay');
ok('delay без VPN — null', d?.ok && d.v?.delay === null, JSON.stringify(d?.v ?? d?.e));
await sleep(800);

// Экраны и листы. [имя снимка, что нажать: [селектор, текст]…]
const steps = [
  ['page-main', []],
  ['page-menu', [['button[aria-label="Ещё"]']]],
  ['page-rename', [['[role=menuitem]', 'Переименовать']]],
  ['page-speed', [['[role=alertdialog] button', 'Отмена'], ['button[aria-label="Скорость — подробнее"]']]],
  ['page-add', [['[role=dialog][aria-label="Скорость"] button[aria-label="Закрыть"]'], ['button[aria-label="Добавить подписку"]']]],
  ['page-routes', [['[role=dialog][aria-label="Добавить подключение"] button[aria-label="Закрыть"]'], ['nav button[aria-label="Маршруты"]']]],
  ['page-rules', [['button.rrow', 'Мои']]],
  ['page-diag', [['[role=dialog][aria-label^="Мои"] button[aria-label="Закрыть"]'], ['button', 'Что-то не открывается']]],
  ['page-settings', [['nav button[aria-label="Настройки"]']]],
  ['page-method', [['button.rrow', 'Способ подключения']]],
  ['page-look', [['[role=dialog][aria-label="Способ подключения"] button[aria-label="Закрыть"]'], ['button.rrow', 'Оформление']]],
  ['page-look-dark', [['button[role=radio]', 'Тёмная']]],
  ['page-killswitch', [['button[role=radio]', 'Системная'], ['nav button[aria-label="Настройки"]'], ['[role=button]', 'Kill Switch']]],
  ['page-more', [['nav button[aria-label="Настройки"]'], ['button.rrow', 'Дополнительно']]],
  ['page-about', [['nav button[aria-label="Настройки"]'], ['button.rrow', 'О приложении']]],
];
for (const [name, clicks] of steps) {
  let fine = true;
  for (const [sel, text] of clicks) {
    if (!(await click(sel, text))) {
      fine = false;
      console.log(`FAIL ${name}: не нашёл ${sel}${text ? ' «' + text + '»' : ''}`);
    }
    await sleep(700);
  }
  const shot = await m.shot(name);
  const scrolled = await m.evalJs(`document.querySelector('.bm-root')?.scrollTop ?? -1`);
  if (fine) ok(`снимок ${name}`, shot && scrolled === 0, scrolled ? `окно сдвинуто на ${scrolled}` : '');
}
await click('nav button[aria-label="Главная"]');
await sleep(500);
const errors = await m.evalJs(`document.body.innerText.includes('Служба kl!ck не отвечает')`);
ok('служба отвечает окну', !errors);

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
