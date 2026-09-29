// Видеоинструкция kl!ck из превью окна (тестовые данные): `npm run dev` в одном окне, потом
//   npm run video        — dist/klick-instrukciya.mp4 с подписями и голосом Windows (≈3 мин)
//   npm run video:gif    — docs/demo.gif без звука для README (≈25 с)
// Нужен ffmpeg: в PATH или путь в переменной FFMPEG. Голос — VOICE (по умолчанию «Microsoft Irina Desktop»).

import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const MODE = process.argv[2] === 'gif' ? 'gif' : 'full';
const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.join(HERE, '..', '..');
const WORK = path.join(os.tmpdir(), `klick-video-${MODE}`);
const FFMPEG = process.env.FFMPEG ?? 'ffmpeg';
const BASE = 'http://127.0.0.1:5173';
const EDGE = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const LOGO = path.join(REPO, 'docs', 'logo');
const PORT = 9352;
const W = 1920;
const H = 1080;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const icon = (c) => 'data:image/png;base64,' + fs.readFileSync(path.join(LOGO, `${c}-dark.png`)).toString('base64');

// ── Сценарий ──────────────────────────────────────────────────────────────
// page: страница превью; cap: [заголовок, подпись]; say: что читает голос; act: действия.
const FULL = [
  {
    id: 's01', page: '/setup.html', cap: ['Установка', 'Скачайте klick-setup.exe со страницы релизов на GitHub и запустите. Права администратора нужны один раз — чтобы поставить службу.'],
    say: 'Скачайте установщик клик со страницы релизов на гитхабе и запустите его. Права администратора нужны один раз — чтобы поставить службу.',
    act: async (d) => { await d.wait(1800); await d.click('Установить'); await d.until(`document.body.innerText.includes('установлен')`, 20000); },
  },
  {
    id: 's02', cap: ['Установка', 'Нет WebView2 — установщик поставит его сам. Прежнюю версию kl!ck найдёт, перенесёт подписки и удалит.'],
    say: 'Если на компьютере нет компонента веб вью два, установщик поставит его сам. А прежнюю версию клика найдёт, перенесёт из неё подписки и удалит.',
    act: async (d) => { await d.wait(4000); await d.move('Готово'); },
    after: async (d) => { await d.click('Готово'); },
  },
  {
    id: 's03', page: '/?s=empty', cap: ['Добавьте подписку', 'Нажмите «Добавить подключение» и вставьте ссылку от вашего VPN-провайдера — тип ссылки kl!ck определит сам.'],
    say: 'После установки клик откроется пустым. Нажмите «Добавить подключение» и вставьте ссылку на подписку от вашего VPN-провайдера. Что это за ссылка, клик поймёт сам.',
    act: async (d) => {
      await d.wait(1500);
      await d.click('Добавить подключение');
      await d.wait(900);
      await d.type('Ссылка на подписку или конфигурацию', 'https://panel.example.com/sub/demo');
      await d.wait(600);
      await d.click('Добавить', { exact: true });
      await d.wait(1500);
    },
  },
  {
    id: 's04', cap: ['Подключитесь', 'Нажмите большую кнопку — и компьютер пойдёт через VPN. Цвет кнопки и значка в трее показывает состояние.'],
    say: 'Подписка добавлена: видны сервер, остаток трафика и срок действия. Теперь нажмите большую кнопку — и компьютер пойдёт через VPN. Цвет кнопки и значка в трее показывает состояние.',
    act: async (d) => { await d.wait(3500); await d.click('Включить VPN'); await d.wait(1200); await d.trayIcon('green'); },
  },
  {
    id: 's05', cap: ['Серверы', 'Нажмите на карточку подписки — «Проверить задержку» покажет, какой сервер быстрее. Переключаться можно на ходу.'],
    say: 'Нажмите на карточку подписки — откроется список серверов. «Проверить задержку» покажет, какой из них быстрее. Переключиться можно прямо на ходу, без переподключения.',
    act: async (d) => {
      await d.wait(600);
      await d.click('Нидерланды · Amsterdam');
      await d.wait(1200);
      await d.click('Проверить задержку');
      await d.wait(2600);
      await d.click('Турция · Istanbul');
    },
  },
  {
    id: 's06', cap: ['Режим', 'VPN (TUN) — все программы, включая игры. Системный прокси — браузеры и программы, которые его понимают.'],
    say: 'На вкладке «Соединение» выбирается режим. VPN, он же TUN, пускает через туннель все программы, включая игры. Системный прокси — только браузеры и программы, которые его понимают.',
    act: async (d) => { await d.nav('Соединение'); await d.wait(1500); await d.move('VPN (TUN)'); await d.wait(3000); await d.move('Системный прокси'); },
  },
  {
    id: 's07', cap: ['Куда направлять', '«VPN для всего» — через VPN всё. «VPN для выбранного» — заблокированное в РФ и ваш список, остальное напрямую.'],
    say: 'Ниже — куда направлять трафик. «VPN для всего»: через VPN идёт всё, а российские сайты можно пустить напрямую. «VPN для выбранного»: через VPN только заблокированное в России и ваш список, остальное — напрямую.',
    act: async (d) => { await d.wait(800); await d.click('VPN для всего'); await d.wait(5500); await d.click('VPN для выбранного'); },
  },
  {
    id: 's08', cap: ['Свой список', 'Добавьте в список сервис из каталога, программу, сайт или IP-адрес.'],
    say: 'В свой список можно добавить сервис из каталога, программу, сайт или IP-адрес.',
    act: async (d) => {
      await d.click('Добавить', { exact: true });
      await d.wait(900);
      await d.click('Сайт или IP');
      await d.wait(500);
      await d.type('example.com или 1.2.3.4', 'youtube.com');
      await d.wait(400);
      await d.click('Добавить · 1', { within: '.sheet' });
      await d.wait(1200);
    },
  },
  {
    id: 's09', cap: ['Сейчас в сети', 'Какие программы куда ходят — через VPN или напрямую. «Не открывается?» подскажет причину.'],
    say: 'Ниже видно, какие программы сейчас в сети и куда они идут — через VPN или напрямую. А если сайт не открывается, раздел «Не открывается?» подскажет причину.',
    act: async (d) => { await d.scrollTo('Сейчас в сети'); await d.wait(4500); await d.scrollTo('Не открывается?'); },
  },
  {
    id: 's10', cap: ['Как вас видят сайты', 'IP, страна и провайдер через VPN и напрямую, утечки DNS и IPv6.'],
    say: '«Как вас видят сайты» покажет ваш адрес, страну и провайдера — через VPN и напрямую — и проверит утечки DNS и IPv6.',
    act: async (d) => { await d.scrollTo('Как вас видят сайты'); await d.wait(900); await d.click('Проверить', { exact: true }); await d.wait(2500); },
  },
  {
    id: 's11', cap: ['Kill Switch', 'Отмеченные программы не выйдут в интернет мимо VPN: без VPN они остаются без сети.'],
    say: 'Kill Switch — в настройках. Отмеченные программы не выйдут в интернет мимо VPN: если VPN выключен или оборвался, они просто остаются без сети.',
    act: async (d) => {
      await d.nav('Настройки');
      await d.wait(1200);
      await d.click('Kill Switch', { within: '.window main' });
      await d.wait(1500);
      await d.click('Добавить приложение');
      await d.wait(1200);
      await d.click('Google Chrome');
      await d.wait(700);
      await d.click('Добавить · 1');
      await d.wait(900);
      await d.scrollTo('Google Chrome');
    },
  },
  {
    id: 's12', cap: ['Трей', 'Крестик не закрывает kl!ck, а сворачивает в трей — VPN продолжает работать.'],
    say: 'Крестик не закрывает клик, а сворачивает его в трей — VPN продолжает работать.',
    act: async (d) => { await d.wait(800); await d.click('Свернуть в трей'); await d.hideWindow(true); },
  },
  {
    id: 's13', cap: ['Трей', 'Правый щелчок по значку — быстрое меню. Левый — главное окно.'],
    say: 'Правый щелчок по значку в трее открывает быстрое меню: кнопка, скорость, серверы, режим и Kill Switch. Левый щелчок — главное окно.',
    act: async (d) => {
      await d.tray('right');
      await d.showTray(true);
      await d.wait(6500);
      await d.desktopClick();
      await d.showTray(false);
      await d.wait(700);
      await d.tray('left');
      await d.hideWindow(false);
    },
  },
  {
    id: 's14', cap: ['Готово', 'Скачать kl!ck: github.com/vbu00/klick'],
    say: 'Вот и всё. Ссылка на скачивание — в описании.',
    act: async (d) => { await d.wait(1500); },
  },
];

const GIF = [
  { id: 'g1', page: '/?s=empty', cap: ['1', 'Вставьте ссылку на подписку'], hold: 800, act: async (d) => {
    await d.wait(900); await d.click('Добавить подключение'); await d.wait(700);
    await d.type('Ссылка на подписку или конфигурацию', 'https://panel.example.com/sub/demo'); await d.wait(300);
    await d.click('Добавить', { exact: true }); await d.wait(1600);
  } },
  { id: 'g2', cap: ['2', 'Нажмите кнопку'], hold: 2200, act: async (d) => { await d.wait(500); await d.click('Включить VPN'); await d.wait(1200); await d.trayIcon('green'); } },
  { id: 'g3', cap: ['3', 'Правый щелчок по значку — быстрое меню'], hold: 3200, act: async (d) => { await d.hideWindow(true); await d.wait(500); await d.tray('right'); await d.showTray(true); } },
];

// ── Сцена в браузере ──────────────────────────────────────────────────────
// Выполняется в странице превью: фон «рабочего стола», панель задач со значком, подписи, курсор.
function stage(o) {
  if (document.getElementById('demo-root')) return;
  const st = document.createElement('style');
  st.textContent = `
    html, body { overflow: hidden !important; }
    body { background: radial-gradient(1100px 760px at 72% 26%, #1f3f78 0%, transparent 62%), radial-gradient(900px 700px at 18% 86%, #3b2a70 0%, transparent 60%), #0a0d16 !important; }
    .preview { background: transparent !important; min-height: 100vh !important; padding: 0 !important; justify-content: center !important; ${o.side ? 'padding-left: 560px !important;' : 'padding-bottom: 170px !important;'} }
    .preview-bar { display: none !important; }
    .preview .window, #root > .installer { transform: scale(${o.scale}); transform-origin: center center; transition: opacity .35s, transform .35s; }
    .demo-hidden .preview .window { opacity: 0; transform: scale(${o.scale * 0.94}); }
    #root > .installer { margin: 0 auto; }
    #demo-cap { position: fixed; z-index: 50; color: #fff; font-family: "Segoe UI Variable Display", "Segoe UI", sans-serif; transition: opacity .35s; }
    #demo-cap.side { left: 110px; top: 50%; transform: translateY(-50%); width: 600px; }
    #demo-cap.bottom { left: 50%; bottom: 96px; transform: translateX(-50%); width: 1300px; text-align: center; }
    #demo-cap .t { font-size: 22px; letter-spacing: .14em; text-transform: uppercase; color: #6ee7a0; font-weight: 700; margin-bottom: 14px; }
    #demo-cap .x { font-size: 38px; line-height: 1.32; font-weight: 600; color: #f2f4f8; text-wrap: pretty; }
    #demo-cap.big .x { font-size: 46px; }
    #demo-bar { position: fixed; left: 0; right: 0; bottom: 0; height: 56px; z-index: 40; background: rgba(24, 26, 32, .86); backdrop-filter: blur(20px); border-top: 1px solid rgba(255,255,255,.07); display: flex; align-items: center; justify-content: center; color: #fff; font: 13px "Segoe UI", sans-serif; }
    #demo-bar .start { width: 26px; height: 26px; display: grid; grid-template-columns: 1fr 1fr; gap: 2px; }
    #demo-bar .start i { background: #4cc2ff; border-radius: 2px; }
    #demo-bar .search { margin-left: 16px; width: 220px; height: 34px; border-radius: 17px; background: rgba(255,255,255,.08); display: flex; align-items: center; padding-left: 16px; color: #aab; }
    #demo-bar .tray { position: absolute; right: 18px; display: flex; align-items: center; gap: 16px; }
    #demo-bar .tray img { width: 24px; height: 24px; border-radius: 5px; padding: 5px; transition: background .2s; }
    #demo-bar .tray img.hot { background: rgba(255,255,255,.12); }
    #demo-bar .clock { text-align: right; line-height: 1.25; }
    #demo-cur { position: fixed; z-index: 100; left: 1300px; top: 700px; width: 30px; height: 30px; pointer-events: none; transition: left .75s cubic-bezier(.4,.1,.2,1), top .75s cubic-bezier(.4,.1,.2,1); filter: drop-shadow(0 2px 4px rgba(0,0,0,.5)); }
    .demo-ring { position: fixed; z-index: 99; width: 46px; height: 46px; margin: -23px 0 0 -23px; border-radius: 50%; border: 3px solid #6ee7a0; pointer-events: none; animation: demo-ring .55s ease-out forwards; }
    .demo-ring.right { border-color: #ffb454; }
    .demo-tag { position: fixed; z-index: 101; padding: 6px 12px; border-radius: 8px; background: #ffb454; color: #111; font: 700 17px "Segoe UI", sans-serif; pointer-events: none; animation: demo-tag 1.6s ease forwards; }
    @keyframes demo-ring { from { transform: scale(.3); opacity: 1; } to { transform: scale(1.4); opacity: 0; } }
    @keyframes demo-tag { 0% { opacity: 0; transform: translateY(6px); } 15%, 80% { opacity: 1; transform: none; } 100% { opacity: 0; } }
    #demo-tray { position: fixed; z-index: 45; right: 20px; bottom: 68px; width: 320px; height: 704px; border: 0; transform: scale(var(--sc, 1.2)) translateY(12px); transform-origin: bottom right; opacity: 0; transition: opacity .3s, transform .3s; pointer-events: none; border-radius: 12px; }
    #demo-tray.on { opacity: 1; transform: scale(var(--sc, 1.2)); }
    #demo-fade { position: fixed; inset: 0; z-index: 200; background: #0a0d16; opacity: ${o.fade ? 1 : 0}; transition: opacity .45s; pointer-events: none; }
  `;
  document.head.appendChild(st);
  const root = document.createElement('div');
  root.id = 'demo-root';
  const now = new Date();
  const hh = String(now.getHours()).padStart(2, '0') + ':' + String(now.getMinutes()).padStart(2, '0');
  const dd = now.toLocaleDateString('ru-RU');
  root.innerHTML = `
    <div id="demo-cap" class="${o.side ? 'side' : 'bottom'}"><div class="t"></div><div class="x"></div></div>
    <div id="demo-bar"><div class="start"><i></i><i></i><i></i><i></i></div><div class="search">Поиск</div>
      <div class="tray"><span style="opacity:.7">˄</span><img id="demo-ico" src="${o.icon}" alt=""><span>⌔</span><span>🔊</span><div class="clock">${hh}<br>${dd}</div></div></div>
    ${o.withTray ? `<iframe id="demo-tray" src="${location.origin}/?view=tray&s=connected"></iframe>` : ''}
    <svg id="demo-cur" viewBox="0 0 24 24"><path d="M3 2 L3 19 L8 14.5 L11.5 22 L14.5 20.7 L11 13.3 L17.5 13.3 Z" fill="#fff" stroke="#111" stroke-width="1.4" stroke-linejoin="round"/></svg>
    <div id="demo-fade"></div>`;
  document.body.appendChild(root);
  const tray = document.getElementById('demo-tray');
  if (tray) {
    tray.addEventListener('load', () => {
      const s = tray.contentDocument.createElement('style');
      s.textContent = 'html,body{background:transparent!important;overflow:hidden!important}.preview{min-height:0!important;padding:0!important;display:block!important;background:transparent!important}.preview-bar{display:none!important}';
      tray.contentDocument.head.appendChild(s);
      // Высота окна трея — по содержимому, масштаб — чтобы влезло над панелью задач.
      setTimeout(() => {
        const t = tray.contentDocument.querySelector('.tray');
        if (!t) return;
        const h = Math.ceil(t.scrollHeight);
        tray.style.height = h + 'px';
        tray.style.setProperty('--sc', Math.min(1.25, (1080 - 56 - 40) / h).toFixed(3));
      }, 1800);
    });
  }
  const cur = document.getElementById('demo-cur');
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const visible = (e) => {
    const r = e.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== 'hidden';
  };
  window.demo = {
    cap(t, x, big) {
      const c = document.getElementById('demo-cap');
      c.style.opacity = 0;
      setTimeout(() => {
        c.querySelector('.t').textContent = t;
        c.querySelector('.x').textContent = x;
        c.classList.toggle('big', !!big);
        c.style.opacity = 1;
      }, 350);
    },
    fade(on) { document.getElementById('demo-fade').style.opacity = on ? 1 : 0; },
    find(q, opt = {}) {
      const scope = opt.within ? document.querySelector(opt.within) : document;
      if (!scope) return null;
      const all = [...scope.querySelectorAll('button, [role=tab], [role=switch], a, textarea, input, .row, .pick, [role=button], h1, h2, h3, h4, .caps, .sec-title, span, div, b')]
        .filter((e) => !e.closest('#demo-root') && (opt.nav || !e.closest('nav')) && visible(e));
      const label = (e) => (e.getAttribute('aria-label') || e.getAttribute('placeholder') || '').toLowerCase();
      // textContent, а не innerText: подписи в верхнем регистре сделаны стилем.
      const text = (e) => (e.textContent || e.value || '').trim().toLowerCase();
      q = q.toLowerCase();
      const clickable = (e) => e.closest('button, [role=tab], [role=switch], a, .row, .pick, [role=button], textarea, input') || e;
      // Из нескольких совпадений — самое глубокое: у контейнера с одной кнопкой тот же текст, что у кнопки.
      const last = (f) => all.filter(f).pop();
      let hit = all.find((e) => label(e) === q) || last((e) => text(e) === q);
      if (!hit && !opt.exact) hit = last((e) => text(e).startsWith(q));
      return hit ? (opt.raw ? hit : clickable(hit)) : null;
    },
    point(el) {
      const r = el.getBoundingClientRect();
      return { x: r.left + Math.min(r.width / 2, 60 + r.width * 0.15), y: r.top + r.height / 2 };
    },
    async moveTo(x, y) { cur.style.left = x - 4 + 'px'; cur.style.top = y - 3 + 'px'; await wait(820); },
    ring(x, y, right) {
      const r = document.createElement('div');
      r.className = right ? 'demo-ring right' : 'demo-ring';
      r.style.left = x + 'px';
      r.style.top = y + 'px';
      document.body.appendChild(r);
      setTimeout(() => r.remove(), 700);
    },
    tag(x, y, text) {
      const t = document.createElement('div');
      t.className = 'demo-tag';
      t.textContent = text;
      t.style.left = x - 70 + 'px';
      t.style.top = y - 62 + 'px';
      document.body.appendChild(t);
      setTimeout(() => t.remove(), 1700);
    },
    async type(el, text) {
      el.focus();
      const proto = el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      const set = Object.getOwnPropertyDescriptor(proto, 'value').set;
      for (const ch of text) {
        set.call(el, el.value + ch);
        el.dispatchEvent(new Event('input', { bubbles: true }));
        await wait(38);
      }
    },
    icon(src) { document.getElementById('demo-ico').src = src; },
    hideWindow(on) { document.body.classList.toggle('demo-hidden', on); },
    showTray(on) { document.getElementById('demo-tray')?.classList.toggle('on', on); },
    trayPoint() { const r = document.getElementById('demo-ico').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; },
    hot(on) { document.getElementById('demo-ico').classList.toggle('hot', on); },
  };
}

// ── Браузер и запись ──────────────────────────────────────────────────────
fs.rmSync(WORK, { recursive: true, force: true });
fs.mkdirSync(path.join(WORK, 'frames'), { recursive: true });
const steps = MODE === 'gif' ? GIF : FULL;

let durations = {};
if (MODE === 'full') {
  fs.writeFileSync(path.join(WORK, 'lines.json'), JSON.stringify(steps.map((s) => ({ id: s.id, say: s.say }))), 'utf8');
  execFileSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(HERE, 'video-tts.ps1'), '-Dir', WORK, '-Voice', process.env.VOICE ?? 'Microsoft Irina Desktop'], { stdio: 'inherit' });
  durations = JSON.parse(fs.readFileSync(path.join(WORK, 'durations.json'), 'utf8').replace(/^\uFEFF/, ''));
}

const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'klick-video-'));
const edge = spawn(EDGE, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`, `--window-size=${W},${H}`, '--hide-scrollbars', '--force-device-scale-factor=1', '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows', 'about:blank'], { stdio: 'ignore' });

let target;
for (let i = 0; i < 60 && !target; i++) {
  try { target = (await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()).find((t) => t.type === 'page'); } catch {}
  if (!target) await sleep(200);
}
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r, j) => ((ws.onopen = r), (ws.onerror = j)));
let seq = 0;
const pending = new Map();
const frames = [];
let recording = false;
let paused = 0; // сколько секунд вырезано (переходы между страницами)
let pauseStart = 0;
const vnow = () => Date.now() / 1000 - paused;
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); return; }
  if (msg.method === 'Page.screencastFrame') {
    const { data, sessionId } = msg.params;
    ws.send(JSON.stringify({ id: ++seq, method: 'Page.screencastFrameAck', params: { sessionId } }));
    if (!recording) return;
    const file = `f${String(frames.length).padStart(5, '0')}.jpg`;
    fs.writeFileSync(path.join(WORK, 'frames', file), Buffer.from(data, 'base64'));
    // Время получения кадра: metadata.timestamp у Edge идёт по своим часам.
    frames.push({ file, t: vnow() });
  }
};
const send = (method, params = {}) => new Promise((r) => { const id = ++seq; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
const js = async (expr) => {
  const r = await Promise.race([
    send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true }),
    sleep(20000).then(() => { throw new Error('страница не ответила за 20 с: ' + expr.slice(0, 160)); }),
  ]);
  if (r.result?.exceptionDetails) throw new Error(r.result.exceptionDetails.exception?.description ?? JSON.stringify(r.result.exceptionDetails));
  return r.result?.result?.value;
};

await send('Page.enable');
// Страница не должна считаться фоновой: иначе таймеры замедляются до секунды.
await send('Emulation.setFocusEmulationEnabled', { enabled: true });
await send('Page.bringToFront');
await send('Emulation.setDeviceMetricsOverride', { width: W, height: H, deviceScaleFactor: 1, mobile: false });

const ICONS = { grey: icon('grey'), green: icon('green') };
let currentIcon = ICONS.grey;

async function startCast() {
  await send('Page.startScreencast', { format: 'jpeg', quality: 90, maxWidth: W, maxHeight: H, everyNthFrame: 1 });
}

async function open(pagePath, first) {
  if (!first) {
    await js(`window.demo && demo.fade(true)`);
    await sleep(550);
    recording = false;
    pauseStart = Date.now() / 1000;
  }
  await send('Page.navigate', { url: BASE + pagePath });
  await sleep(2200);
  const setup = pagePath.startsWith('/setup');
  await js(`(${stage.toString()})(${JSON.stringify({ scale: setup ? 1.25 : 1.3, side: !setup, icon: currentIcon, withTray: !setup, fade: true })})`);
  await sleep(900);
  if (!first) paused += Date.now() / 1000 - pauseStart;
  recording = true;
  await js('demo.fade(false)');
  await sleep(500);
}

// Действия для сценария.
const d = {
  wait: (ms) => sleep(ms),
  async until(expr, ms) { const end = Date.now() + ms; while (Date.now() < end) { if (await js(expr)) return; await sleep(200); } throw new Error('не дождался: ' + expr); },
  async el(q, opt = {}) {
    let ok = false;
    for (let i = 0; i < 30 && !ok; i++) {
      ok = await js(`(() => { const e = demo.find(${JSON.stringify(q)}, ${JSON.stringify(opt)}); if (!e) return false; e.scrollIntoView({ block: 'nearest' }); window.__demoEl = e; return true; })()`);
      if (!ok) await sleep(200);
    }
    if (!ok) {
      const seen = await js(`[...document.querySelectorAll('button,[role=tab]')].filter(e=>e.getBoundingClientRect().width).map(e=>(e.getAttribute('aria-label')||e.innerText).trim()).join(' | ')`);
      throw new Error(`не нашёл «${q}». Видно: ${seen}`);
    }
  },
  async move(q, opt) { await this.el(q, opt); await js(`(async () => { const p = demo.point(window.__demoEl); await demo.moveTo(p.x, p.y); })()`); },
  async click(q, opt) {
    await this.move(q, opt);
    await sleep(120);
    await js(`(() => { const p = demo.point(window.__demoEl); demo.ring(p.x, p.y); window.__demoEl.click(); })()`);
    await sleep(350);
  },
  async nav(q) { await this.click(q, { nav: true }); },
  async type(q, text) { await this.click(q); await js(`demo.type(window.__demoEl, ${JSON.stringify(text)})`); },
  async scrollTo(q) {
    await this.el(q, { raw: true });
    await js(`window.__demoEl.scrollIntoView({ behavior: 'smooth', block: 'center' })`);
    await sleep(1100);
  },
  async trayIcon(c) { currentIcon = ICONS[c]; await js(`demo.icon(${JSON.stringify(currentIcon)})`); },
  hideWindow: (on) => js(`demo.hideWindow(${on})`),
  showTray: (on) => js(`demo.showTray(${on})`),
  async tray(button) {
    await js(`(async () => { const p = demo.trayPoint(); await demo.moveTo(p.x, p.y); demo.hot(true); })()`);
    await sleep(200);
    await js(`(() => { const p = demo.trayPoint(); demo.ring(p.x, p.y, ${button === 'right'}); demo.tag(p.x, p.y, ${JSON.stringify(button === 'right' ? 'правый щелчок' : 'левый щелчок')}); })()`);
    await sleep(500);
    await js('demo.hot(false)');
  },
  async desktopClick() { await js(`(async () => { await demo.moveTo(1500, 380); demo.ring(1500, 380); })()`); await sleep(300); },
};

const marks = [];
try {
  await startCast();
  let first = true;
  for (const s of steps) {
    if (s.page) { await open(s.page, first); first = false; }
    await js(`demo.cap(${JSON.stringify(s.cap[0])}, ${JSON.stringify(s.cap[1])}, ${MODE === 'gif'})`);
    await sleep(450);
    const start = vnow();
    if (MODE === 'full') marks.push({ id: s.id, t: start });
    await s.act(d);
    const need = MODE === 'full' ? durations[s.id] + 0.7 : (s.hold ?? 1000) / 1000;
    const left = MODE === 'full' ? start + need - vnow() : need;
    if (left > 0) await sleep(left * 1000);
    if (s.after) await s.after(d);
    console.log(`${s.id} ok`);
  }
  await sleep(2500);
  recording = false;
  await send('Page.stopScreencast');
} finally {
  ws.close();
  edge.kill();
  await sleep(500);
  fs.rmSync(profile, { recursive: true, force: true });
}

// ── Сборка ffmpeg ─────────────────────────────────────────────────────────
const t0 = frames[0].t;
const end = vnow() - 0.5;
let list = '';
frames.forEach((f, i) => {
  const next = i + 1 < frames.length ? frames[i + 1].t : Math.max(end, f.t + 2);
  list += `file 'frames/${f.file}'\nduration ${Math.max(0.001, next - f.t).toFixed(3)}\n`;
});
list += `file 'frames/${frames[frames.length - 1].file}'\n`;
fs.writeFileSync(path.join(WORK, 'list.txt'), list);
fs.mkdirSync(path.join(REPO, 'dist'), { recursive: true });
const out = MODE === 'gif' ? path.join(REPO, 'docs', 'demo.gif') : path.join(REPO, 'dist', 'klick-instrukciya.mp4');

if (MODE === 'full') {
  const inputs = ['-f', 'concat', '-safe', '0', '-i', 'list.txt'];
  const parts = [];
  marks.forEach((m, i) => {
    inputs.push('-i', `${m.id}.wav`);
    parts.push(`[${i + 1}:a]adelay=${Math.round((m.t - t0 + 0.25) * 1000)}:all=1[a${i}]`);
  });
  const filter = `[0:v]fps=30,format=yuv420p[v];${parts.join(';')};${marks.map((_, i) => `[a${i}]`).join('')}amix=inputs=${marks.length}:normalize=0:duration=longest[a]`;
  execFileSync(FFMPEG, ['-y', '-hide_banner', '-loglevel', 'error', ...inputs, '-filter_complex', filter, '-map', '[v]', '-map', '[a]', '-c:v', 'libx264', '-preset', 'slow', '-crf', '20', '-c:a', 'aac', '-b:a', '160k', '-shortest', '-movflags', '+faststart', out], { cwd: WORK, stdio: 'inherit' });
} else {
  execFileSync(FFMPEG, ['-y', '-hide_banner', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', 'list.txt', '-vf', 'fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=160:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle', '-loop', '0', out], { cwd: WORK, stdio: 'inherit' });
}
console.log(`готово: ${out} (${(fs.statSync(out).size / 1048576).toFixed(1)} МБ, кадров ${frames.length}, ${(end - t0).toFixed(1)} с)`);
