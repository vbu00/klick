// kl!ck — окно. Экраны по макету «VPN Client»: Главная, Добавить,
// Маршрутизация, Настройки (+ режим, Kill Switch, тема, логи) и шторка
// выбора программ. Всё общение с Rust — через window.kl (tauri-bridge.js).
'use strict';

const kl = window.kl;
const T = window.klickTheme;
const $ = (id) => document.getElementById(id);
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const ic = (name, size, style = '') => `<span class="ic" style="--i:url(assets/icons/${name}.svg);width:${size}px;height:${size}px;${style}"></span>`;
const errText = (e) => (typeof e === 'string' ? e : e?.message || String(e));

const RED = 'var(--red)', ORANGE = 'var(--orange)', GREEN = 'var(--accent)', DIM = 'var(--dim)';
const TINTS = ['#a8c7fa', '#f5c26b', '#c4b5fd', '#9ad9c0', '#f4a7b9', '#b8c4d0'];
const MODE_NAMES = { proxy: 'Proxy · порт', sysproxy: 'Системный proxy', tun: 'VPN (TUN)' };

// ─────────── Состояние ───────────

let ov = null; // get_overview
let status = { state: 'off' };
let traffic = { up: 0, down: 0, upTotal: 0, downTotal: 0 };
let logs = [];
const pings = {}; // id подключения → { имя сервера → мс | null }
const hist = { dl: new Array(60).fill(0), ul: new Array(60).fill(0) };
const ui = {
  screen: 'home', sub: null, addTab: 'link', rulesTab: 'sites',
  expanded: false, menuOpen: false, confirmDel: false,
  linkInput: '', nameInput: '', siteInput: '',
  picker: null, pickerQuery: '', pickerSel: [], pickerApps: null,
  // Маршрутизация: объяснения от бэкенда (route.rs), проверка адреса, подтверждение.
  rv: null, checkInput: '', checkRes: null, routeAsk: null,
  // Соединение: живой список из ядра, проверка IP, окно правила.
  net: null, netOpen: {}, ip: null, ipBusy: false, netSheet: null,
  pinging: false, refreshing: false, adding: false, powerBusy: false,
  // Какое подключение открыто на Главной. Пока VPN работает, можно смотреть
  // другое, не разрывая текущее; переключение — кнопкой питания.
  viewId: null,
  // Окна-шторки: mode (как работает режим), ks (как работает Kill Switch),
  // switch (сменить подключение?), licenses.
  modal: null, modeInfo: 'tun', ksViz: 'on', switchTo: null,
  ksTab: 'apps', ksSiteInput: '', geoBusy: false, updChecking: false,
};
let info = null; // app_info: версия, сборка, система, база GeoIP
let licText = null;
let toasts = [];
let toastId = 0;
// Проблема Kill Switch: тост, потом точка на «Настройках», пока её не
// посмотрели; строка Kill Switch подсвечена, пока проблема не ушла.
let ksIssue = null;
let ksSeen = true;
let ksToastId = 0;

const S = () => ov.settings;
const profiles = () => ov?.profiles || [];
const byId = (id) => profiles().find((p) => p.id === id) || null;
// Открытое на Главной: выбранное вручную, иначе то, что подключается/работает.
const active = () => byId(ui.viewId) || byId(ov.activeProfile) || profiles()[0] || null;
const isOn = () => status.state === 'on';
const isBusy = () => status.state === 'connecting' || status.state === 'reconnecting';
const liveOn = (p) => isOn() && p && status.profileId === p.id;
// Работает другое подключение, а смотрим это.
const elsewhere = (p) => isOn() && p && status.profileId && status.profileId !== p.id && !!byId(status.profileId);

// ─────────── Форматирование ───────────

// Скорость — в мегабитах, как в макете.
const fmtRate = (bytes) => { const mb = (bytes * 8) / 1e6; return mb >= 1000 ? (mb / 1000).toFixed(2) + ' Gb/s' : mb.toFixed(2) + ' Mb/s'; };
const fmtBytes = (b) => (b >= 1024 ** 3 ? (b / 1024 ** 3).toFixed(2) + ' ГБ' : (b / 1024 ** 2).toFixed(1) + ' МБ');
const gb = (b) => { const v = b / 1024 ** 3; return v >= 100 ? Math.round(v) : +v.toFixed(1); };
const pingColor = (p) => (p == null ? RED : p < 80 ? GREEN : p < 160 ? 'var(--text)' : ORANGE);
const pingText = (p) => (p == null ? 'нет ответа' : p + ' мс');
const plural = (n, a, b, c) => { const m10 = n % 10, m100 = n % 100; return m10 === 1 && m100 !== 11 ? a : m10 >= 2 && m10 <= 4 && (m100 < 10 || m100 >= 20) ? b : c; };
const fmtDay = (secs) => {
  const d = new Date(secs * 1000);
  return d.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short', year: d.getFullYear() === new Date().getFullYear() ? undefined : 'numeric' });
};

// ─────────── Иконки сайтов ───────────

// Иконки качает Rust (favicon.rs) с самого сайта и кэширует; до ответа и
// если иконки нет — глобус, как в макете.
const favs = {}; // домен → data:-URI | null (нет или ещё грузится)
const favDomain = (pattern) => { const d = String(pattern).trim().replace(/^\*?\./, ''); return /^[a-z0-9.-]+\.[a-z0-9-]+$/i.test(d) ? d.toLowerCase() : null; };
function favTile(pattern, big = false) {
  const d = favDomain(pattern);
  if (d && !(d in favs)) loadFav(d);
  const src = d && favs[d];
  return `<div class="fav${big ? ' big' : ''}"${d ? ` data-fav="${esc(d)}"` : ''}>${ic('globe', big ? 16 : 14)}${src ? `<img src="${src}" alt="">` : ''}</div>`;
}
async function loadFav(d) {
  favs[d] = null;
  let src = null;
  try { src = await kl.favicon(d); } catch { /* нет — остаётся глобус */ }
  if (!src || !/^data:image\//.test(src)) return;
  favs[d] = src;
  document.querySelectorAll('[data-fav]').forEach((el) => {
    if (el.dataset.fav === d && !el.querySelector('img')) el.insertAdjacentHTML('beforeend', `<img src="${esc(src)}" alt="">`);
  });
}

function timerText() {
  const h = isOn() && status.since ? Math.max(0, Math.floor(Date.now() / 1000 - status.since)) : 0;
  return `${Math.floor(h / 3600)}:${String(Math.floor(h / 60) % 60).padStart(2, '0')}:${String(h % 60).padStart(2, '0')}`;
}
// Маршрутизация: тумблер и два положения, у каждого свой список.
const POS_NAMES = { proxy: 'VPN для всего', direct: 'VPN для выбранного' };
const onlyChosen = () => S().routing && S().defaultRoute === 'direct';
const curList = () => S().lists[S().defaultRoute];
function routeLabel() {
  const s = S();
  return s.routing ? POS_NAMES[s.defaultRoute] : 'всё через VPN';
}
function modeLabel() {
  const s = S();
  const m = s.mode === 'proxy' ? `Proxy · порт ${s.proxyPort}` : MODE_NAMES[s.mode];
  return `${m} · ${routeLabel()}`;
}

// ─────────── Тосты ───────────

// Снекбары по макету: не больше двух, 2,8 с (с кнопкой — 5 с). Пояснение
// под заголовком — только у ошибок, предупреждений и тех, где есть кнопка.
function toast(title, text = '', color = DIM, action = null) {
  const id = ++toastId, ms = action ? 5000 : 2800;
  toasts = [...toasts.slice(-1), { id, title, text, color, action, ms }];
  renderToasts();
  setTimeout(() => { toasts = toasts.filter((t) => t.id !== id); renderToasts(); renderNav(); }, ms);
  return id;
}
function renderToasts() {
  $('toasts').innerHTML = toasts.map((t) => {
    const showText = t.text && (t.color === RED || t.color === ORANGE || t.action);
    return `<div class="snack" data-id="${t.id}">
      <div class="dot" style="background:${t.color}"></div>
      <div class="bd" style="padding-right:${t.action ? 0 : 12}px"><div class="tt">${esc(t.title)}</div>${showText ? `<div class="tx">${esc(t.text)}</div>` : ''}</div>
      ${t.action ? `<button class="act press" data-tact="${t.id}">${esc(t.action.label)}</button>` : ''}</div>`;
  }).join('');
}
// Нажатие на снекбар убирает его; на кнопку — ещё и выполняет действие.
$('toasts').addEventListener('click', (e) => {
  const s = e.target.closest('.snack');
  if (!s) return;
  const id = +s.dataset.id;
  const t = toasts.find((x) => x.id === id);
  toasts = toasts.filter((x) => x.id !== id);
  renderToasts();
  renderNav();
  if (e.target.closest('[data-tact]') && t?.action) t.action.run();
});

// ─────────── Подсказки ───────────

// Как в макете: всплывают через 350 мс (сразу, если подсказка уже видна),
// над элементом, а у верхнего края окна — под ним.
let tipEl = null, tipTimer = 0;
function showTip(el) {
  clearTimeout(tipTimer);
  tipEl = el;
  const visible = !!$('tip').firstChild;
  tipTimer = setTimeout(() => {
    if (tipEl !== el || !el.isConnected) return;
    const r = el.getBoundingClientRect(), w = document.querySelector('.window').getBoundingClientRect();
    const above = r.top - w.top > 90;
    const x = Math.max(50, Math.min(w.width - 50, r.left - w.left + r.width / 2));
    const y = above ? r.top - w.top - 8 : r.bottom - w.top + 8;
    $('tip').innerHTML = `<div class="tip" style="left:${x}px;top:${y}px;transform:translate(-50%,${above ? '-100%' : '0'})"><div class="in">${esc(el.dataset.tip)}<i class="${above ? 'b' : 't'}"></i></div></div>`;
  }, visible ? 0 : 350);
}
function hideTip() {
  clearTimeout(tipTimer);
  tipEl = null;
  $('tip').innerHTML = '';
}
document.addEventListener('mouseover', (e) => {
  const el = e.target.closest?.('[data-tip]');
  if (el === tipEl) return;
  if (el) showTip(el); else hideTip();
});
document.addEventListener('mouseleave', hideTip);
document.addEventListener('mousedown', hideTip, true);
document.addEventListener('scroll', hideTip, true);

// ─────────── Графики ───────────

function buildPath(h, max) {
  const pts = h.map((v, i) => [(i / (h.length - 1)) * 300, 88 - (v / max) * 82]);
  const d = pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join(' ');
  return { line: d, area: d + ' L300 89 L0 89 Z' };
}
function updateLive() {
  // Часы и скорость — точечно, без перерисовки экрана.
  const set = (id, v) => { const el = $(id); if (el) el.textContent = v; };
  set('timer', timerText());
  set('dlText', fmtRate(isOn() ? traffic.down : 0));
  set('ulText', fmtRate(isOn() ? traffic.up : 0));
  set('dlTotal', 'всего ' + fmtBytes(traffic.downTotal || 0));
  set('ulTotal', 'всего ' + fmtBytes(traffic.upTotal || 0));
  const toMb = (b) => (b * 8) / 1e6;
  const dl = hist.dl.map(toMb), ul = hist.ul.map(toMb);
  const max = Math.max(50, ...dl, ...ul) * 1.1;
  const d = buildPath(dl, max), u = buildPath(ul, max);
  const g = $('graph');
  if (g) {
    g.querySelector('.dla').setAttribute('d', d.area);
    g.querySelector('.dll').setAttribute('d', d.line);
    g.querySelector('.ull').setAttribute('d', u.line);
    set('peak', fmtRate(Math.max(...hist.dl)));
  }
}

// ─────────── Главная ───────────

function renderHome() {
  if (!profiles().length) {
    return `<div class="onb">
      <div class="big">${ic('power', 38)}</div>
      <div style="font-size:20px;font-weight:600;margin-top:22px">Нет подключений</div>
      <div class="lead">Добавьте ссылку на подписку или одиночную конфигурацию — и можно включать.</div>
      <div style="margin-top:26px;display:flex;flex-direction:column;gap:10px">
        ${[['Вставьте ссылку', 'Подписка (https://…) или vless://, vmess://, trojan://, ss://'], ['Выберите режим', 'По умолчанию VPN (TUN) — работает для всех программ'], ['Нажмите кнопку питания', 'Сайты .ru и локальная сеть пойдут напрямую']]
          .map(([t, d], i) => `<div class="step"><div class="num">${i + 1}</div><div><div class="t14" style="font-weight:600">${t}</div><div class="t12" style="color:var(--text3)">${d}</div></div></div>`).join('')}
      </div>
      <button class="bigbtn press" style="margin-top:22px" data-act="goAdd">Добавить подключение</button>
    </div>`;
  }
  const p = active();
  const busy = isBusy() || ui.powerBusy;
  const other = elsewhere(p);
  // «on» — туннель идёт через открытое подключение.
  const on = isOn() && !other;
  const bad = status.state === 'error' || (on && status.health && !status.health.ok);
  const isSub = p.kind === 'sub';
  const multi = p.servers.length > 1 || isSub;
  const pp = pings[p.id] || {};
  const livePing = liveOn(p) && status.health ? (status.health.ok ? status.health.ms : null) : undefined;
  const activePing = ui.pinging ? '…' : pp[p.active] !== undefined ? pingText(pp[p.active]) : livePing !== undefined ? pingText(livePing) : '—';
  const activePingColor = ui.pinging ? DIM : pp[p.active] !== undefined ? pingColor(pp[p.active]) : livePing !== undefined ? pingColor(livePing) : DIM;
  const server = p.servers.find((s) => s.name === p.active) || p.servers[0];
  const subLine = isSub || multi ? server?.name || '' : 'IP: ' + (server?.host || '').replace(/:\d+$/, '');
  const statusLabel = other ? 'нажмите, чтобы переключиться сюда' : on ? 'время подключения' : status.state === 'connecting' ? 'подключение…' : status.state === 'reconnecting' ? 'переподключение…' : 'не подключено';
  const powerCls = other ? 'other' : on ? 'on' : busy ? 'busy' : status.state === 'error' ? 'bad' : '';

  let trafficHtml = '';
  if (p.info && (p.info.total > 0 || p.info.expire > 0 || p.info.download > 0)) {
    const used = p.info.upload + p.info.download;
    const pct = p.info.total ? Math.min(100, (used / p.info.total) * 100) : 0;
    const exp = p.info.expire ? new Date(p.info.expire * 1000) : null;
    const days = exp ? Math.max(0, Math.round((exp - new Date()) / 864e5)) : 0;
    trafficHtml = `<div class="traffic">
      <div class="top"><span>Трафик</span><b>${p.info.total ? `${gb(used)} из ${gb(p.info.total)} ГБ` : `${gb(used)} ГБ из ∞`}</b></div>
      <div class="bar"><div style="width:${pct}%;background:${pct > 90 ? ORANGE : 'var(--text)'}"></div></div>
      <div class="bottom"><div>${exp ? `Действует до <b>${exp.toLocaleDateString('ru-RU', { day: 'numeric', month: 'long' })}</b> · ${days === 0 ? 'истекает' : days + ' дн.'}` : 'Без срока действия'}</div>
      ${isSub && p.hasUrl ? `<button class="pill press" data-act="refreshSub">${ic('refresh', 13, ui.refreshing ? 'animation:spin 1s linear infinite' : '')}${ui.refreshing ? 'Обновляем…' : 'Обновить'}</button>` : ''}</div>
    </div>`;
  } else if (isSub && p.hasUrl) {
    trafficHtml = `<div class="traffic"><div class="bottom" style="margin-top:0"><div>${p.updatedAt ? 'Обновлена ' + new Date(p.updatedAt * 1000).toLocaleString('ru-RU', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : 'Подписка'}</div>
      <button class="pill press" data-act="refreshSub">${ic('refresh', 13, ui.refreshing ? 'animation:spin 1s linear infinite' : '')}${ui.refreshing ? 'Обновляем…' : 'Обновить'}</button></div></div>`;
  }

  let expandHtml = '';
  if (ui.expanded) {
    if (multi) {
      expandHtml = `<div class="expand">
        <div class="head"><div class="t12" style="margin:0;text-transform:uppercase;letter-spacing:.6px">Серверы · ${p.servers.length}</div>
          <button class="pill press" style="height:26px;border-radius:13px;font-weight:400" data-act="ping">${ui.pinging ? 'Проверяем…' : 'Проверить задержку'}</button></div>
        ${p.servers.map((s) => {
          const ms = pp[s.name];
          const txt = ui.pinging ? '…' : ms === undefined ? '' : pingText(ms);
          return `<div class="srv${s.name === p.active ? ' on' : ''}" data-server="${esc(s.name)}">
            <div class="radio"><i></i></div>
            <div style="flex:1;min-width:0"><div class="n">${esc(s.name)}</div><div class="pr">${esc(s.proto)}</div></div>
            <div class="ms" style="color:${ui.pinging || ms === undefined ? DIM : pingColor(ms)}">${txt}</div></div>`;
        }).join('')}
      </div>`;
    } else {
      expandHtml = `<div class="expand"><div style="padding:0 8px 8px;display:flex;flex-direction:column;gap:10px">
        <div class="kv"><span>Протокол</span><span>${esc(server?.proto)}</span></div>
        <div class="kv"><span>Сервер</span><span style="font-variant-numeric:tabular-nums">${esc(server?.host)}</span></div>
        <div class="kv"><span>Задержка</span><span style="color:${activePingColor}">${activePing}</span></div>
        <div style="display:flex;gap:8px;margin-top:4px"><button class="wide-btn press" data-act="ping">${ui.pinging ? 'Проверяем…' : 'Проверить задержку'}</button></div>
      </div></div>`;
    }
  }

  let menuHtml = '';
  if (ui.menuOpen) {
    const items = isSub && p.hasUrl
      ? [['refreshSub', 'Обновить подписку', 'refresh'], ['pingMenu', 'Проверить задержку', 'server'], ['copyLink', 'Копировать ссылку', 'copy']]
      : [['pingMenu', 'Проверить задержку', 'server'], ['copyLink', p.kind === 'file' ? 'Копировать серверы (YAML)' : 'Копировать конфигурацию', 'copy']];
    menuHtml = `<div class="scrim" data-act="closeMenu"></div><div class="menu">
      ${items.map(([act, l, i]) => `<button data-act="${act}">${ic(i, 16)}<span class="l">${l}</span></button>`).join('')}
      <div class="msep"></div>
      <button class="danger${ui.confirmDel ? ' confirm' : ''}" data-act="${ui.confirmDel ? 'removeProfile' : 'confirmDel'}">${ic('trash', 16)}<span class="l">${ui.confirmDel ? 'Нажмите ещё раз' : isSub ? 'Удалить подписку' : 'Удалить конфигурацию'}</span><span class="h">${ui.confirmDel ? 'удалить' : ''}</span></button>
    </div>`;
  }

  const notice = other ? ''
    : status.state === 'error' && status.error
    ? `<div class="errbox"><div class="dot" style="background:var(--red);margin-top:5px"></div><div>${esc(status.error)}</div></div>`
    : status.warning ? `<div class="errbox warn"><div class="dot" style="background:var(--orange);margin-top:5px"></div><div>${esc(status.warning)}</div></div>`
    : on && status.health && !status.health.ok ? `<div class="errbox warn"><div class="dot" style="background:var(--orange);margin-top:5px"></div><div>Туннель поднят, но через «${esc(status.server || '')}» ничего не открывается. Выберите другой сервер.</div></div>` : '';

  return `
    <div class="status"><div class="lbl">${statusLabel}</div><div class="timer" id="timer">${timerText()}</div></div>
    <div class="power-wrap">
      ${on && !bad ? '<div class="waves"><div class="grid"></div><div class="wave"></div><div class="ringfx"></div></div>' : ''}
      <button class="power ${powerCls}" data-act="power" aria-label="${other ? 'Переключиться сюда' : on || busy ? 'Отключить' : 'Подключить'}"><span class="disc">${ic('power', 34)}</span></button>
    </div>
    <button class="modechip press" data-act="goMode"><span class="d" style="background:${isOn() ? (bad ? RED : GREEN) : busy ? ORANGE : DIM}"></span>${esc(modeLabel())}</button>
    ${notice}
    ${profiles().length > 1 ? `<div class="chips">${profiles().map((x) => `<button class="chip press${x.id === p.id ? ' on' : ''}" data-profile="${x.id}">${liveOn(x) ? '<i class="live"></i>' : ''}<span>${esc(x.name)}</span></button>`).join('')}</div>` : ''}
    <div class="pcard" style="margin-top:${profiles().length > 1 ? 10 : 34}px">
      <div class="card">
        <div class="phead" data-act="toggleExpand">
          <div class="picon">${ic(isSub ? 'globe' : 'rules2', 24)}</div>
          <div style="flex:1;min-width:0">
            <div class="pnamerow"><div class="pname">${esc(p.name)}</div>${on ? '<span class="livebadge">Работает</span>' : ''}</div>
            <div class="psub"><span class="s">${esc(subLine)}</span><span class="sep"></span><span class="p" style="color:${activePingColor}">${activePing}</span></div>
          </div>
          <button class="menubtn press${ui.menuOpen ? ' on' : ''}" data-act="openMenu" data-tip="Действия" aria-label="Действия">${ic('dots', 20)}</button>
        </div>
        ${trafficHtml}
        ${expandHtml}
      </div>
      ${menuHtml}
    </div>
    <div class="speed">
      <div class="card"><div class="hd">Чтение${ic('cloud-down', 22)}</div><div class="v" id="dlText">${fmtRate(isOn() ? traffic.down : 0)}</div><div class="s" id="dlTotal">всего ${fmtBytes(traffic.downTotal || 0)}</div></div>
      <div class="card"><div class="hd">Загрузка${ic('cloud-up', 22)}</div><div class="v" id="ulText">${fmtRate(isOn() ? traffic.up : 0)}</div><div class="s" id="ulTotal">всего ${fmtBytes(traffic.upTotal || 0)}</div></div>
    </div>
    <div class="card graph">
      <div class="hd"><div style="font-size:13px;color:var(--dim)">Последние 60 секунд</div>
        <div class="legend"><span><i style="background:var(--accent)"></i>Чтение</span><span><i style="background:var(--dim)"></i>Загрузка</span></div></div>
      <svg id="graph" viewBox="0 0 300 90" preserveAspectRatio="none">
        <line x1="0" y1="89" x2="300" y2="89" style="stroke:var(--elem)" stroke-width="1"></line>
        <line x1="0" y1="45" x2="300" y2="45" style="stroke:var(--elem)" stroke-width="1" stroke-dasharray="3 4"></line>
        <path class="dla" style="fill:color-mix(in srgb,var(--accent) 12%,transparent)"></path>
        <path class="dll" fill="none" style="stroke:var(--accent)" stroke-width="2" stroke-linejoin="round"></path>
        <path class="ull" fill="none" style="stroke:var(--dim)" stroke-width="1.5" stroke-linejoin="round"></path>
      </svg>
      <div class="ft"><span>−60 с</span><span>Пик: <span id="peak">0.00 Mb/s</span></span><span>сейчас</span></div>
    </div>`;
}

// ─────────── Добавить ───────────

function detect(v) {
  v = v.trim();
  if (!v) return null;
  if (/^https?:\/\/\S+$/i.test(v)) return { kind: 'sub', title: 'Ссылка на подписку', text: 'Загрузим список серверов, лимит трафика и срок действия. Будет обновляться автоматически.' };
  const lines = v.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
  const re = /^(vless|vmess|trojan|ss|hysteria2|hy2|tuic|anytls):\/\//i;
  const m = lines[0].match(re);
  if (m && lines.length === 1) return { kind: 'direct', title: `Одиночная конфигурация · ${m[1].toUpperCase()}`, text: 'Прямое соединение с одним сервером. Без лимитов и срока — только адрес и ключ.' };
  if (m) return { kind: 'direct', title: `Список серверов · ${lines.filter((l) => re.test(l)).length}`, text: 'Несколько ссылок — добавим одним подключением со списком серверов.' };
  if (/^\s*proxies\s*:/m.test(v) || /"proxies"\s*:/.test(v)) return { kind: 'direct', title: 'Конфиг mihomo / Clash', text: 'Возьмём из него серверы.' };
  if (/^[A-Za-z0-9+/=_\-\s]{40,}$/.test(v)) return { kind: 'direct', title: 'Содержимое подписки', text: 'Похоже на список серверов в base64 — попробуем разобрать.' };
  return { kind: 'bad', title: 'Формат не распознан', text: 'Ожидается https://… или vless://, vmess://, trojan://, ss://, hysteria2://' };
}

function detectHtml() {
  const d = detect(ui.linkInput);
  const color = !d ? 'var(--ring)' : d.kind === 'bad' ? RED : GREEN;
  return `<div class="dot" style="background:${color}"></div>
    <div style="flex:1;min-width:0"><div style="font-size:13px;font-weight:600">${d ? d.title : 'Ожидаем ссылку'}</div>
    <div style="font-size:12px;color:var(--text3);margin-top:2px;text-wrap:pretty">${d ? d.text : 'Скопируйте её у провайдера VPN и вставьте выше — тип определится сам.'}</div></div>`;
}
function updateAddDynamic() {
  const d = detect(ui.linkInput);
  const ta = $('linkInput');
  if (ta) ta.style.borderColor = !d ? 'var(--line)' : d.kind === 'bad' ? RED : GREEN;
  if ($('detect')) $('detect').innerHTML = detectHtml();
  if ($('nameInput')) $('nameInput').classList.toggle('hidden', d?.kind !== 'sub');
  const b = $('addBtn');
  if (b) {
    b.classList.toggle('off', !d || d.kind === 'bad' || ui.adding);
    b.textContent = ui.adding ? (d?.kind === 'sub' ? 'Загружаем подписку…' : 'Добавляем…') : 'Добавить';
  }
}

function renderAdd() {
  const tabs = `<div class="seg" style="margin-top:18px"><button class="press${ui.addTab === 'link' ? ' on' : ''}" data-act="addTab" data-v="link">По ссылке</button><button class="press${ui.addTab === 'file' ? ' on' : ''}" data-act="addTab" data-v="file">Из файла</button></div>`;
  const body = ui.addTab === 'link'
    ? `<textarea class="linkarea" id="linkInput" spellcheck="false" placeholder="https://panel.example.com/sub/a1b2…  или  vless://…">${esc(ui.linkInput)}</textarea>
      <div class="detect" id="detect">${detectHtml()}</div>
      <input class="field hidden" id="nameInput" style="margin-top:10px" placeholder="Название (необязательно)" value="${esc(ui.nameInput)}" />
      <button class="bigbtn press" id="addBtn" style="margin-top:14px" data-act="addLink">Добавить</button>
      <div class="label" style="margin-top:26px">Что можно вставить</div>
      <div class="card mt what">
        <div class="row"><div class="ico">${ic('globe', 18)}</div><div><div class="t13">Ссылка на подписку</div><div class="tx">https://… от Remnawave, Marzban, 3x-ui и подобных. Даёт список серверов, лимит трафика и срок действия. Обновляется автоматически.</div></div></div>
        <div class="divider"></div>
        <div class="row"><div class="ico">${ic('rules2', 18)}</div><div><div class="t13">Одиночная конфигурация</div><div class="tx">vless://, vmess://, trojan://, ss://, hysteria2://, tuic:// — прямое соединение с одним сервером. Можно вставить сразу несколько строк.</div></div></div>
      </div>`
    : `<div class="drop" id="drop" data-act="addFile">${ic('cloud-up', 34)}<div style="font-size:14px;font-weight:600;color:var(--text)">Перетащите файл сюда</div><div style="font-size:12px">или нажмите, чтобы выбрать</div></div>
      <div class="note">Поддерживаются конфиги Mihomo / Clash: <b>.yaml</b>, <b>.yml</b>, <b>.json</b>, а также текстовый список ссылок. Серверы копируются в приложение — оригинал можно удалить.</div>`;
  return `<div class="h1">Добавить подключение</div>${tabs}${body}`;
}

// ─────────── Маршрутизация ───────────

const CHIP = { vpn: 'Через VPN', direct: 'Напрямую', block: 'Блок', ks: 'Только VPN' };
const STATE_OF = { proxy: 'vpn', direct: 'direct', block: 'block' };
const ACTION_OF = { vpn: 'proxy', direct: 'direct', block: 'block' };

// Итог строки — из route.rs (тот же порядок, что в правилах ядра).
function rowHtml(kind, key, name, sub, state, tile, removable, i) {
  const r = (ui.rv?.[kind === 'app' ? 'apps' : 'sites'] || []).find((x) => x.key === key && x.state === state);
  const result = r ? `<div class="rres"><span>итог</span><b class="t-${r.tone}">${esc(r.result)}</b></div>` : '';
  const hint = r?.hint ? `<div class="rhint${r.warn ? ' warn' : ''}">${esc(r.hint)}</div>` : '';
  return `<div class="rrow${r?.tone === 'muted' ? ' muted' : ''}">
    <div class="top">${tile}<div class="grow"><div class="t14 ell">${esc(name)}</div>${sub ? `<div class="mono sub">${esc(sub)}</div>` : ''}</div>
      <button class="rchip c-${state} press" data-act="cycleRule" data-kind="${kind}" data-key="${esc(key)}" data-state="${state}" aria-label="Сменить правило">${CHIP[state]}</button>
      ${removable ? `<button class="xbtn press" data-act="removeRule" data-kind="${kind}" data-i="${i}" aria-label="Удалить">${ic('close', 14)}</button>` : ''}</div>
    ${result}${hint}</div>`;
}

function renderRules() {
  const s = S(), on = s.routing, ex = s.defaultRoute === 'proxy', list = curList();
  const tog = (v, act, label) => `<div class="toggle${v ? ' on' : ''}" data-act="${act}" role="switch" aria-checked="${v}" aria-label="${label}"></div>`;
  const geoDate = info?.geo?.updated ? ' · база от ' + fmtDay(info.geo.updated) : '';
  const bl = ui.rv?.blockedCount ? `${ui.rv.blockedCount} доменов${ui.rv.blockedUpdated ? ', обновлён ' + fmtDay(ui.rv.blockedUpdated) : ''}.` : 'Скачается при подключении.';
  const set = ex
    ? ['ru', 'Набор «Россия напрямую»', 'Российские сайты и адреса — без VPN: банки и госуслуги видят ваш IP' + (geoDate || '.')]
    : ['blocked', 'Набор «Заблокированные в РФ»', 'YouTube, Discord, Instagram и другие — через VPN. ' + bl];
  const tiles = (i) => TINTS[i % TINTS.length];
  const ksRows = s.ksApps.filter((a) => a.on).map((a, i) => rowHtml('app', a.exe, a.name, a.exe, 'ks', `<div class="tile-ic" style="background:${tiles(i + 3)}">${esc((a.name || a.exe)[0].toUpperCase())}</div>`, false));
  const apps = ksRows.concat(list.apps.map((r, i) => rowHtml('app', r.exe, r.name, r.exe, STATE_OF[r.action], `<div class="tile-ic" style="background:${tiles(i)}">${esc((r.name || r.exe)[0].toUpperCase())}</div>`, true, i)));
  const sites = list.sites.map((r, i) => rowHtml('site', r.pattern, r.pattern, '', STATE_OF[r.action], favTile(r.pattern), true, i));
  const tabSites = ui.rulesTab === 'sites';
  const res = ui.checkRes;
  const check = `<div class="card mt checkbox">
      <div class="t14" style="font-weight:600">Проверить сайт</div>
      <div class="addline" style="margin-top:10px"><input class="field" id="checkInput" placeholder="например, youtube.com" value="${esc(ui.checkInput)}" spellcheck="false" aria-label="Адрес сайта" /><button class="sq press" data-act="checkRoute" aria-label="Проверить">${ic('chevron', 18)}</button></div>
      <div class="samples">${['youtube.com', 'gosuslugi.ru', 'wikipedia.org', 'nas.lan'].map((x) => `<button class="press" data-act="checkSample" data-v="${x}">${x}</button>`).join('')}</div>
      ${res ? `<div class="verdict t-${res.tone}">${esc(res.verdict)}</div>
      <ol class="ladder2">${res.steps.map((st, i) => `<li class="${i === res.hit ? 'hit' : i < res.hit ? 'past' : 'next'}"><span>${i + 1}</span><div><b>${esc(st.title)}</b><i>${esc(st.note)}</i></div></li>`).join('')}</ol>` : ''}
    </div>`;
  return `<div class="h1">Маршрутизация</div>
    <div class="card mt rcard">
      <div class="row" style="padding:0"><div class="grow"><div class="t14" style="font-weight:600">Маршрутизация</div>
        <div class="t12" style="line-height:1.45">${on ? 'Включена: трафик идёт по положению ниже и списку.' : 'Выключена: весь трафик идёт через VPN. Списки сохранены и не действуют.'}</div></div>
        ${tog(on, 'routingToggle', 'Маршрутизация')}</div>
      <div class="seg pos${on ? '' : ' off'}">${['proxy', 'direct'].map((v) => `<button class="press${s.defaultRoute === v ? ' on' : ''}" data-act="routePos" data-v="${v}"${on ? '' : ' disabled'}>${POS_NAMES[v]}</button>`).join('')}</div>
      ${on ? `<div class="t12" style="line-height:1.45;margin-top:10px">${ex ? 'Через VPN идёт всё, кроме списка ниже: его — напрямую или в блок.' : 'Через VPN идёт только список ниже, остальное — напрямую.'}</div>` : ''}
    </div>
    ${on && s.mode !== 'tun' ? `<div class="summary"><div class="dot" style="background:${ORANGE}"></div><div class="tx">Режим «${s.mode === 'proxy' ? 'Proxy' : 'Системный proxy'}»: правила программ действуют только на те, что сами ходят через прокси. Игры и торренты идут мимо. <a href="#" data-act="goModeSub">Включить VPN (TUN)</a></div></div>` : ''}
    <div class="card mt${on ? '' : ' dimmed'}"><div class="row"><div class="grow"><div class="t14">${set[1]}</div><div class="t12" style="line-height:1.4">${set[2]}</div></div>${tog(s.sets[set[0]], 'toggleSet', set[1])}</div></div>
    <div class="seg" style="margin-top:22px"><button class="press${tabSites ? '' : ' on'}" data-act="rulesTab" data-v="apps">Программы</button><button class="press${tabSites ? ' on' : ''}" data-act="rulesTab" data-v="sites">Сайты</button></div>
    <div class="hint">${tabSites ? 'Домен со всеми поддоменами. Нажмите на правило, чтобы сменить.' : 'Нажмите на правило, чтобы сменить. Программа главнее сайтов.'}</div>
    ${check}
    ${tabSites
      ? `<div class="addline" style="margin-top:12px"><input class="field" id="siteInput" placeholder="domain.com  или  .ru" value="${esc(ui.siteInput)}" spellcheck="false" aria-label="Сайт" /><button class="sq press" data-act="addSite" aria-label="Добавить сайт">${ic('plus', 22)}</button></div>
        <div class="rules">${sites.join('') || '<div class="empty-note">Сайтов в списке пока нет.</div>'}</div>`
      : `<button class="addwide press" data-act="pickApps" data-v="rules">${ic('plus', 20)}Выбрать программу…</button>
        <div class="rules">${apps.join('') || '<div class="empty-note">Программ в списке пока нет.</div>'}</div>`}`;
}

async function loadRouting() {
  try { ui.rv = await kl.routingView(); } catch { ui.rv = null; }
  if (ui.checkRes) { try { ui.checkRes = await kl.checkRoute(ui.checkInput); } catch {} }
  if (ui.screen === 'rules') render();
}

// Переключение тумблера или положения — сначала «что изменится».
async function askRoute(routing, route) {
  try { ui.routeAsk = { routing, route, p: await kl.routingPreview(routing, route) }; } catch (e) { return toast('Не получилось', errText(e), RED); }
  openModal('route');
}
function routeAskHtml() {
  const a = ui.routeAsk;
  if (!a) return '';
  const dot = { vpn: 'var(--accent)', direct: 'var(--text2)', muted: 'var(--dim)', warn: 'var(--orange)' };
  return sheetWrap(`<div class="mt-title" style="text-wrap:pretty">${esc(a.p.title)}</div>
    <div class="points">${a.p.lines.map((l) => `<div><span style="background:${dot[l.tone] || 'var(--dim)'}"></span><div>${esc(l.text)}</div></div>`).join('')}</div>
    <div class="mbtns"><button class="mbtn press" data-act="closeModal">Отмена</button><button class="mbtn accent press" data-act="confirmRoute">${esc(a.p.cta)}</button></div>`, 'closeModal');
}

// ─────────── Соединение ───────────

const ROUTE_TXT = { vpn: 'через VPN', direct: 'напрямую', block: 'блок', mixed: 'по-разному', ks: 'только через VPN' };
const ROUTE_CHIP = { vpn: 'Через VPN', direct: 'Напрямую', block: 'Блок', mixed: 'По-разному', ks: 'Только VPN' };
const agoTxt = (sec) => (sec < 60 ? 'только что' : Math.round(sec / 60) + ' мин назад');
const kbRate = (b) => (b >= 1024 * 1024 ? (b / 1024 / 1024).toFixed(1).replace('.', ',') + ' МБ/с' : Math.round(b / 1024) + ' КБ/с');

async function loadNet() {
  try { ui.net = await kl.connectionsView(); } catch { ui.net = null; }
  if (ui.screen === 'net' && !ui.netSheet) render();
}

function renderNet() {
  const s = S(), n = ui.net, on = isOn();
  const status = on ? `Подключено · ${modeLabel()}` : 'Выключено';
  const sys = on && s.mode !== 'tun';
  const tot = n ? n.nVpn + n.nDirect + n.nBlock : 0;
  const bar = (k, c) => (n && n[k] ? `<span style="flex:${n[k]} 0 0;background:${c}"></span>` : '');
  const failed = n?.failures || [];
  const fixOf = (f) => (f.route === 'block' ? ['unblock', 'Убрать блок'] : f.route === 'vpn' ? ['direct', 'Напрямую'] : ['vpn', 'Через VPN']);
  const apps = n?.apps || [];
  const ip = ui.ip;
  const ipCell = (x, err) => (x ? null : err ? 'не ответил' : '—');
  const ipRows = ip ? [
    ['IP', ip.vpn?.ip || ipCell(ip.vpn, ip.vpnError), ip.direct?.ip || ipCell(ip.direct, ip.directError)],
    ['Где', ip.vpn?.place || ipCell(ip.vpn, ip.vpnError), ip.direct?.place || ipCell(ip.direct, ip.directError)],
    ['Сеть', ip.vpn?.org || ipCell(ip.vpn, ip.vpnError), ip.direct?.org || ipCell(ip.direct, ip.directError)],
    ['Похоже на VPN', ip.vpn ? (ip.vpn.hosting ? 'да — адрес хостинга' : 'по названию — нет') : '—', ip.direct ? (ip.direct.hosting ? 'да' : 'нет') : '—'],
    ['IPv6', on ? (ip.ipv6 ? 'идёт мимо VPN' : 'нет') : '—', ip.ipv6 ? esc(ip.ipv6) : 'нет'],
  ] : [];
  return `<div class="h1">Соединение</div>
    <div class="netstat"><span class="dot" style="background:${on ? GREEN : 'var(--dim2)'}"></span>${esc(status)}</div>
    ${sys ? `<div class="summary"><div class="dot" style="background:${ORANGE}"></div><div class="tx">Системный прокси: здесь видны только программы, которые ходят через прокси. Игр и торрентов тут не будет — их трафик идёт мимо kl!ck.</div></div>` : ''}
    ${!on ? `<div class="card mt" style="padding:20px 16px;text-align:center"><div class="t14" style="font-weight:600">VPN выключен</div><div class="t12" style="line-height:1.45;margin-top:4px">Соединения видны, пока VPN включён. «Как тебя видят сайты» можно проверить и сейчас — покажет только прямое подключение.</div></div>` : `
    <div class="card mt" style="padding:14px">
      <div style="display:flex;justify-content:space-between;align-items:baseline"><div class="t14" style="font-weight:600">Куда идёт трафик</div><div class="t12">${tot} адресов</div></div>
      <div class="netbar">${bar('nVpn', 'var(--accent)')}${bar('nDirect', 'var(--dim)')}${bar('nBlock', 'var(--red)')}</div>
      <div class="netlegend"><span><b class="t-vpn">${n?.nVpn ?? 0}</b> через VPN</span><span><b class="t-direct">${n?.nDirect ?? 0}</b> напрямую</span><span><b class="t-block">${n?.nBlock ?? 0}</b> блок</span></div>
    </div>`}
    ${failed.length ? `<div class="label" style="color:var(--orange)">Не открывается? · ${failed.length}<span class="t12" style="float:right;text-transform:none;letter-spacing:0">за 10 минут</span></div>
    <div class="card mt">${failed.map((f, i) => { const [fx, fl] = fixOf(f); return `${i ? '<div class="divider"></div>' : ''}<div class="netfail">
      <div class="top"><div class="grow"><div class="mono t14 ell">${esc(f.host)}</div><div class="t12">${esc(f.app || 'программа не определена')} · ${agoTxt(f.ago)}</div></div>
        <button class="rchip c-vpn press" data-act="netFix" data-i="${i}" data-v="${fx}">${fl}</button></div>
      <div class="t12" style="color:var(--text2)"><b class="t-${f.route}">${ROUTE_TXT[f.route]}</b> · ${esc(f.why)}</div></div>`; }).join('')}</div>` : ''}
    ${on ? `<div class="label">Сейчас в сети · ${apps.length}<span class="t12" style="float:right;text-transform:none;letter-spacing:0">по скорости</span></div>
    <div class="card mt">${apps.map((a, i) => { const open = !!ui.netOpen[a.exe]; return `${i ? '<div class="divider"></div>' : ''}<div class="netapp">
      <div class="top"><button class="netexp press" data-act="netToggle" data-v="${esc(a.exe)}" aria-expanded="${open}"><div class="tile-ic" style="background:${TINTS[i % TINTS.length]}">${esc((a.name || a.exe)[0].toUpperCase())}</div>
        <div class="grow"><div class="t14 ell">${esc(a.name)}</div><div class="t12">${a.conns} соедин. · ${kbRate(a.speed)}</div></div><span class="caret${open ? ' open' : ''}">${ic('chevron', 14)}</span></button>
        <button class="rchip c-${a.route === 'mixed' ? 'mixed' : a.route} press" data-act="netRule" data-kind="app" data-v="${esc(a.exe)}">${ROUTE_CHIP[a.route]}</button></div>
      ${open ? `<div class="nethosts"><div class="t12" style="line-height:1.4;padding-bottom:6px">${esc(a.why)}</div>${a.hosts.slice(0, 12).map((h) => `<div class="nethost"><div class="grow"><div class="mono ell" style="font-size:12px">${esc(h.host)}${h.count > 1 ? ` <span class="t12">×${h.count}</span>` : ''}</div><div class="t12"><b class="t-${h.route}">${ROUTE_TXT[h.route]}</b> · ${esc(h.why)}</div></div>
          ${/ · UDP$/.test(h.host) ? '' : `<button class="netrulebtn press" data-act="netRule" data-kind="site" data-v="${esc(h.host)}" data-app="${esc(a.exe)}">Правило</button>`}</div>`).join('')}${a.hosts.length > 12 ? `<div class="t12" style="padding-top:6px">и ещё ${a.hosts.length - 12}</div>` : ''}</div>` : ''}
    </div>`; }).join('') || '<div class="empty-note">Пока тихо: ни одна программа не в сети.</div>'}</div>` : ''}
    <div class="card mt" style="padding:14px">
      <div style="display:flex;align-items:center;gap:10px"><div class="grow"><div class="t14" style="font-weight:600">Как тебя видят сайты</div><div class="t12" style="line-height:1.4">Через VPN спрашиваем ipinfo.io, напрямую — ipwho.is. Только когда нажмёте «Проверить».</div></div>
        <button class="mbtn press" style="height:36px;padding:0 14px;font-size:13px" data-act="netIp"${ui.ipBusy ? ' disabled' : ''}>${ui.ipBusy ? 'Проверяю…' : ip ? 'Ещё раз' : 'Проверить'}</button></div>
      ${ip ? `<div class="ipgrid"><span></span><b class="t-vpn">Через VPN</b><b class="t-direct">Напрямую</b>${ipRows.map(([k, v, d]) => `<span class="t12">${k}</span><span>${esc(v)}</span><span>${esc(d)}</span>`).join('')}</div>
      <div class="t12" style="line-height:1.4;margin-top:8px">«Похоже на VPN» — по названию сети; точно знают только базы самих сайтов. Утечку через WebRTC приложение не видит — её проверяют в браузере.</div>` : ''}
    </div>`;
}

// Правило из строки соединения: окно с вариантами, как на макете.
function netSheetHtml() {
  const t = ui.netSheet;
  if (!t) return '';
  const s = S(), ex = s.defaultRoute === 'proxy';
  const opts = [['vpn', 'Через VPN', ex ? 'В «VPN для всего» это и так по умолчанию — нужно, только чтобы перебить набор «Россия».' : 'Добавится в список того, что идёт через VPN.'],
    ['direct', 'Напрямую', ex ? 'Добавится в список исключений.' : 'В «VPN для выбранного» это и так по умолчанию — нужно, только чтобы перебить набор.'],
    ['block', 'Блок', 'Не пускать совсем.']];
  if (t.kind === 'app' && t.path) opts.push(['ks', 'Только через VPN', 'Через VPN всегда, без VPN — без сети (Kill Switch).']);
  const note = !s.routing ? 'Маршрутизация сейчас выключена — правило заработает, когда её включить.' : `Правило попадёт в список «${POS_NAMES[s.defaultRoute]}». Уже открытые соединения программа переоткроет сама — или перезапустите её.`;
  return sheetWrap(`<div class="mt-title" style="overflow-wrap:anywhere">${esc(t.name)}</div>
    <div class="mlead">Сейчас: <b class="t-${t.cur}">${ROUTE_TXT[t.cur] || t.cur}</b>${t.why ? ' · ' + esc(t.why) : ''}</div>
    <div class="netopts">${opts.filter(([k]) => k !== t.cur).map(([k, l, d]) => `<button class="netopt press" data-act="netApply" data-v="${k}"><b>${l}</b><span>${d}</span></button>`).join('')}</div>
    <div class="t12" style="line-height:1.45;margin-top:12px">${note}</div>
    <button class="mbtn press" style="width:100%;margin-top:14px" data-act="closeModal">Отмена</button>`, 'closeModal');
}

// Сайт для правила: без «www.», IP — как есть.
const ruleHost = (h) => h.replace(/ · UDP$/, '').replace(/^www\./, '');

async function netApply(v) {
  const t = ui.netSheet, s = S();
  closeModal();
  ui.netSheet = null;
  if (!t) return;
  const before = { lists: JSON.parse(JSON.stringify(s.lists)), ksApps: JSON.parse(JSON.stringify(s.ksApps)) };
  const list = curList();
  let patch;
  if (t.kind === 'site') {
    const pattern = ruleHost(t.name);
    const rest = list.sites.filter((r) => r.pattern !== pattern);
    patch = { lists: { ...s.lists, [s.defaultRoute]: { ...list, sites: v === 'unblock' ? rest : [{ pattern, action: ACTION_OF[v] }, ...rest] } } };
  } else {
    const rest = list.apps.filter((r) => r.exe.toLowerCase() !== t.key.toLowerCase());
    const ks = s.ksApps.filter((a) => a.exe.toLowerCase() !== t.key.toLowerCase());
    patch = v === 'ks'
      ? { ksApps: [...ks, { name: t.name, exe: t.key, path: t.path, on: true }], lists: { ...s.lists, [s.defaultRoute]: { ...list, apps: rest } } }
      : { ksApps: ks, lists: { ...s.lists, [s.defaultRoute]: { ...list, apps: [...rest, { name: t.name, exe: t.key, path: t.path || '', action: ACTION_OF[v] }] } } };
  }
  if (await save(patch)) {
    const what = v === 'unblock' ? 'блок снят' : ROUTE_TXT[v];
    toast('Правило добавлено', `${t.kind === 'app' ? t.name : ruleHost(t.name)} — ${what}.`, GREEN, { label: 'Отменить', run: () => save(before) });
    if (v === 'ks' && !S().killSwitch) toast('Kill Switch выключен', 'Без VPN программа пойдёт напрямую, пока его не включить.', ORANGE, { label: 'Включить', run: () => save({ killSwitch: true }) });
    loadNet();
  }
}

// ─────────── Настройки ───────────

const backBtn = () => `<button class="back press" data-act="back">${ic('chevron', 16)}Настройки</button>`;
const chevRow = (act, title, sub) => `<div class="row click" data-act="${act}"><div class="grow"><div class="t14">${title}</div><div class="t12">${esc(sub)}</div></div>${ic('chevron', 16, 'color:var(--dim2)')}</div>`;

function themeSummary() {
  const t = T.load();
  const name = T.THEMES.find((x) => x[0] === t.theme)[1];
  return name + (t.theme === 'custom' ? ' · ' + T.BASES.find((b) => b[0] === t.base)[1] : t.theme === 'system' ? ' · сейчас ' + (T.systemDark() ? 'тёмная' : 'светлая') : '');
}

const ksCount = (s) => {
  const a = s.ksApps.filter((x) => x.on).length, w = (s.ksSites || []).filter((x) => x.on).length;
  const parts = [];
  if (a || !w) parts.push(`${a} ${plural(a, 'приложение', 'приложения', 'приложений')}`);
  if (w) parts.push(`${w} ${plural(w, 'сайт', 'сайта', 'сайтов')}`);
  return parts.join(', ');
};
const infoBtn = (act, size = 18, extra = '') => `<button class="infobtn press" data-act="${act}" ${extra} data-tip="Как это работает" aria-label="Как это работает">${ic('info', size)}</button>`;

function renderSettings() {
  const s = S();
  if (ui.sub === 'mode') return renderMode();
  if (ui.sub === 'kill') return renderKill();
  if (ui.sub === 'theme') return renderTheme();
  if (ui.sub === 'logs') return renderLogs();
  if (ui.sub === 'about') return renderAbout();
  const general = [['autostart', 'Запускать с Windows', 'Свёрнутым в трей и сразу подключаться', ov.autostart], ['autoUpdate', 'Обновлять подписки', 'Каждые 12 часов в фоне', s.autoUpdate], ['notifyDrops', 'Уведомлять об обрывах', 'Всплывающее окно при потере соединения', s.notifyDrops]];
  return `<div class="h1">Настройки</div>
    <div class="label">Подключение</div>
    <div class="card mt">${chevRow('goModeSub', 'Режим подключения', modeLabel())}<div class="divider"></div>${ksIssue
      ? `<div class="row click alert" data-act="goKill"><div class="grow"><div class="t14">Kill Switch</div><div class="t12" style="color:var(--orange)">Не применился — подробности внутри</div></div><span class="adot"></span>${ic('chevron', 16, 'color:var(--dim2)')}</div>`
      : chevRow('goKill', 'Kill Switch', s.killSwitch ? `Вкл · ${ksCount(s)}` : 'Выкл')}</div>
    <div class="label">Оформление</div>
    <div class="card mt">${chevRow('goTheme', 'Тема', themeSummary())}</div>
    <div class="label">Общие</div>
    <div class="card mt">${general.map(([k, t, d, v], i) => `${i ? '<div class="divider"></div>' : ''}<div class="row"><div class="grow"><div class="t14">${t}</div><div class="t12">${d}</div></div><div class="toggle${v ? ' on' : ''}" data-act="general" data-v="${k}"></div></div>`).join('')}</div>
    <div class="label">Диагностика</div>
    <div class="card mt">${chevRow('goLogs', 'Логи подключения', `${logs.length} ${plural(logs.length, 'запись', 'записи', 'записей')}`)}<div class="divider"></div>
      <div class="row"><div class="grow"><div class="t14">Ядро Mihomo</div><div class="t12">${esc(ov.mihomoVersion || 'не найдено')}${ov.mihomoVersion ? ' · официальный релиз' : ''}</div></div><div class="dot" style="background:${ov.mihomoVersion ? GREEN : RED}"></div></div></div>
    <div class="card" style="margin-top:22px">${chevRow('goAbout', 'О приложении', 'Версия ' + ov.appVersion)}</div>`;
}

function renderAbout() {
  const i = info;
  const v = i?.version || ov.appVersion;
  const geo = i?.geo;
  const kv = (k, val, cls = '') => `<div class="kvrow"><span>${k}</span><span class="${cls}">${val}</span></div>`;
  return `${backBtn()}
    <div class="about">
      <span class="key"><span class="ic kf" style="--i:url(assets/mark-face.png)"></span><span class="ic kb" style="--i:url(assets/mark.png)"></span></span>
      <div class="nm">kl<b>!</b>ck</div>
      <div class="vr">Версия ${esc(v)}${i?.build ? ' · сборка ' + esc(i.build) : ''}</div>
      <button class="smallpill press" data-act="checkUpdate">${ui.updChecking ? 'Проверяем…' : 'Проверить обновления'}</button>
    </div>
    <div class="card" style="margin-top:24px">
      ${kv('Ядро', 'Mihomo ' + esc(i?.mihomo || ov.mihomoVersion || '—'))}
      <div class="divider"></div>
      <div class="kvrow"><span>База GeoIP</span><span class="geo">${geo?.updated ? 'от ' + esc(fmtDay(geo.updated)) : '—'}<button class="pill press" data-act="updateGeo">${ic('refresh', 13, ui.geoBusy ? 'animation:spin 1s linear infinite' : '')}${ui.geoBusy ? 'Обновляем…' : 'Обновить'}</button></span></div>
      <div class="divider"></div>
      ${kv('Система', esc(i?.system || 'Windows'))}
      <div class="divider"></div>
      <div class="kvrow click" data-act="openData"><span>Папка данных</span><span class="mono path">${esc(i?.dataDir || '%LOCALAPPDATA%\\com.vbu00.klick')}</span></div>
    </div>
    <div class="card mt">
      ${chevRow('openChangelog', 'Что нового', 'Версия ' + v + ' на GitHub')}<div class="divider"></div>
      ${chevRow('openRepo', 'Исходный код', 'GitHub · vbu00/klick')}<div class="divider"></div>
      ${chevRow('openLicenses', 'Лицензии открытого ПО', 'kl!ck и mihomo — MIT, база GeoIP — GPL-3.0')}
    </div>
    <div class="label">Авторы</div>
    <div class="card mt">
      ${kv('Разработка', 'vbu00')}
      <div class="divider"></div>
      ${kv('Дизайн', 'Dmitriy Medvedev')}
    </div>
    <div class="disclaimer">Приложение не предоставляет VPN-серверы — только подключается к тем, что вы добавили.</div>`;
}

const MODE_DEFS = [
  ['proxy', 'Proxy', 'ручная настройка', (s) => `Открывает прокси на 127.0.0.1:${s.proxyPort}. Через VPN пойдут только программы, где вы сами укажете этот адрес. Остальные — напрямую.`, 'Для браузеров с расширением, торрентов, отдельных программ'],
  ['sysproxy', 'Системный proxy', 'большинство программ', () => 'Windows сообщает адрес прокси всем программам. Браузеры, Telegram, магазины подхватят его сами. Игры и часть приложений его игнорируют.', 'При отключении прежние настройки прокси вернутся'],
  ['tun', 'VPN (TUN)', 'весь трафик', () => 'Создаёт виртуальный сетевой адаптер — через него идёт трафик всех программ без исключения, включая игры и UDP.', 'Рекомендуется'],
];

function renderMode() {
  const s = S();
  return `${backBtn()}
    <div class="h1" style="margin-top:12px">Режим подключения</div>
    <div class="lead">Определяет, какие программы будут ходить через подключение.</div>
    <div style="margin-top:16px;display:flex;flex-direction:column;gap:8px">
      ${MODE_DEFS.map(([k, t, , d, h]) => `<div class="modecard${s.mode === k ? ' on' : ''}" data-act="mode" data-v="${k}"><div class="radio${s.mode === k ? ' on' : ''}"><i></i></div>
        <div style="flex:1;min-width:0"><div style="display:flex;align-items:center;gap:8px"><div class="ti">${t}</div>${infoBtn('modeInfo', 18, `data-v="${k}"`)}</div><div class="ds">${d(s)}</div><div class="hn">${h}</div></div></div>`).join('')}
    </div>
    ${s.mode !== 'tun' ? `<div class="card mt" style="margin-top:8px"><div class="row"><div class="grow"><div class="t14">Порт прокси</div><div class="t12">SOCKS5 и HTTP на 127.0.0.1</div></div><input class="field" id="portInput" type="number" min="1024" max="65535" value="${s.proxyPort}" style="width:96px;height:36px;text-align:right;background:var(--elem)" /></div></div>` : ''}
    <div class="note" style="margin-top:14px">Куда пойдёт трафик — через VPN или напрямую — задаётся на экране «Маршрутизация».</div>`;
}

// Программы Kill Switch — «Только через VPN»: маршрут у них один. У сайтов
// пометка, только если их адреса общие с чужими и закрыть их нельзя.
function ksAppNote() {
  return '';
}
function ksSiteNote(p) {
  return (ov?.ksShared || []).includes(p) ? 'Часть адресов общая с другими сайтами (Cloudflare и т. п.) — их не закрываем, иначе пропадут и соседи.' : '';
}
const noteLine = (t) => (t ? `<div class="t12" style="color:var(--orange);white-space:normal;line-height:1.4">${esc(t)}</div>` : '');

function renderKill() {
  const s = S();
  const apps = ui.ksTab === 'apps';
  const list = apps ? s.ksApps : s.ksSites || [];
  const n = list.filter((a) => a.on).length;
  const body = apps
    ? `<div class="card mt ks-list" style="opacity:${s.killSwitch ? 1 : 0.45}">
      ${s.ksApps.map((a, i) => `${i ? '<div class="divider"></div>' : ''}<div class="row"><div class="tile-ic" style="background:${TINTS[(i + 2) % TINTS.length]}">${esc((a.name || a.exe)[0].toUpperCase())}</div>
        <div class="grow"><div class="t14">${esc(a.name)}</div><div class="mono" style="font-size:11px;color:var(--dim2)">${esc(a.exe)}</div>${noteLine(ksAppNote(a))}</div>
        <div class="toggle${a.on ? ' on' : ''}" data-act="ksApp" data-i="${i}"></div>
        <button class="rmbtn press" data-act="removeKs" data-i="${i}" aria-label="Удалить">${ic('close', 14)}</button></div>`).join('')}
      ${s.ksApps.length ? '<div class="divider"></div>' : ''}
      <div class="row click" style="justify-content:center;gap:8px;padding:16px 14px;color:var(--text2);font-size:13px;font-weight:500" data-act="pickApps" data-v="kill">${ic('plus', 18)}Добавить приложение</div>
    </div>`
    : `<div class="addline" style="margin-top:10px"><input class="field" id="ksSiteInput" placeholder="domain.com" value="${esc(ui.ksSiteInput)}" spellcheck="false" /><button class="sq press" data-act="addKsSite">${ic('plus', 22)}</button></div>
    <div class="card ks-list" style="margin-top:8px;opacity:${s.killSwitch ? 1 : 0.45}">
      ${list.map((a, i) => `${i ? '<div class="divider"></div>' : ''}<div class="row">${favTile(a.pattern, true)}
        <div class="grow" style="min-width:0"><div class="mono t14" style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(a.pattern)}</div>${noteLine(ksSiteNote(a.pattern))}</div>
        <div class="toggle${a.on ? ' on' : ''}" data-act="ksSite" data-i="${i}"></div>
        <button class="rmbtn press" data-act="removeKsSite" data-i="${i}" aria-label="Удалить">${ic('close', 14)}</button></div>`).join('')
        || '<div class="empty-note">Пока пусто. Добавьте домен выше.</div>'}
    </div>`;
  return `${backBtn()}
    <div class="h1row"><div class="h1" style="margin:0">Kill Switch</div>${infoBtn('ksInfo', 22, 'style="width:28px;height:28px"')}</div>
    ${ksIssue ? `<div class="errbox warn"><div class="dot" style="background:var(--orange);margin-top:5px"></div><div style="flex:1;min-width:0">${esc(ksIssue)}</div><button class="retry press" data-act="retryKs">Повторить</button></div>` : ''}
    <div class="card" style="margin-top:16px;padding:14px;display:flex;align-items:center;gap:12px">
      <div style="flex:1;min-width:0"><div style="font-size:15px;font-weight:600">${s.killSwitch ? 'Включён' : 'Выключен'}</div><div style="font-size:12px;color:var(--text3);margin-top:4px;line-height:1.45;text-wrap:pretty">Программы из списка ходят только через VPN: если VPN выключен или соединение оборвалось, они остаются без интернета. Сайты из списка без VPN не открываются.</div></div>
      <div class="toggle big${s.killSwitch ? ' on' : ''}" data-act="toggleKill"></div>
    </div>
    ${s.mode !== 'tun' && s.killSwitch ? `<div class="summary" style="margin-top:10px"><div class="dot" style="background:var(--orange)"></div><div class="tx">Режим «${s.mode === 'proxy' ? 'Proxy' : 'Системный proxy'}»: программы из списка выходят в сеть только через прокси kl!ck. Не умеют ходить через прокси (игры, торренты) — останутся без сети и при включённом VPN. Для них нужен режим VPN (TUN).</div></div>` : ''}
    <div class="seg" style="margin-top:22px"><button class="press${apps ? ' on' : ''}" data-act="ksTab" data-v="apps">Приложения</button><button class="press${!apps ? ' on' : ''}" data-act="ksTab" data-v="sites">Сайты</button></div>
    <div class="listhead"><div class="label" style="margin:0">${apps ? 'Защищённые приложения' : 'Защищённые сайты'}</div><div>${list.length ? `${n} из ${list.length}` : ''}</div></div>
    ${body}
    <div class="hint" style="color:var(--dim2);margin-top:12px">${apps
      ? 'Приложения, не отмеченные здесь, при выключенном VPN работают как обычно — напрямую. Блокировка остаётся и после выхода из kl!ck, пока VPN не включён.'
      : 'При выключенном VPN эти сайты не откроются ни в одной программе. Закрываются адреса самого домена и www — поддомены на других серверах могут открываться, а у сайтов за общим CDN (Cloudflare и т. п.) пропадут и соседи по адресу.'}</div>`;
}

function renderTheme() {
  const t = T.load();
  const opts = T.THEMES.map(([k, label]) => {
    const p = k === 'light' ? T.PALETTES.light : k === 'custom' ? T.PALETTES[t.base] : T.PALETTES.graphite;
    const sys = k === 'system', on = t.theme === k;
    const win = sys ? 'linear-gradient(135deg,#f5f5f7 50%,#1a1a1d 50%)' : p.win;
    const card = sys ? 'rgba(128,128,128,.28)' : p.card, line = sys ? 'rgba(128,128,128,.3)' : p.line, bar = sys ? 'rgba(128,128,128,.6)' : p.dim;
    const accent = k === 'custom' ? t.accent : k === 'light' ? '#1fa34a' : '#30d158';
    return `<div class="themeopt${on ? ' on' : ''}" data-act="theme" data-v="${k}"><div class="prev" style="background:${win};border-color:${line}"><div class="top"><i style="background:${accent}"></i><b style="background:${bar}"></b></div><div class="body" style="background:${card}"></div></div>
      <div class="lab"><div class="radio${on ? ' on' : ''}"><i></i></div>${label}</div></div>`;
  }).join('');
  const desc = { system: 'Повторяет тему Windows и переключается вместе с ней.', light: 'Светлый фон и тёмный текст — удобно при ярком освещении.', dark: 'Тёмный фон — меньше нагрузки на глаза вечером.', custom: 'Выберите основу и цвет акцента ниже.' }[t.theme];
  return `${backBtn()}<div class="h1" style="margin-top:12px">Оформление</div>
    <div class="themes">${opts}</div>
    <div class="hint" style="margin-top:12px">${desc}</div>
    ${t.theme === 'custom' ? `<div class="label">Основа</div>
      <div class="seg" style="margin-top:10px">${T.BASES.map(([k, l]) => `<button class="press${t.base === k ? ' on' : ''}" data-act="themeBase" data-v="${k}">${l}</button>`).join('')}</div>
      <div class="label">Акцент</div>
      <div class="swatches">${T.ACCENTS.map((c) => `<button title="${c}" data-act="accent" data-v="${c}" style="background:${c};box-shadow:0 0 0 2px var(--card),0 0 0 4px ${t.accent === c ? c : 'transparent'}"></button>`).join('')}</div>
      <div class="hint" style="color:var(--dim2);margin-top:12px">Акцентом подсвечиваются кнопка подключения, переключатели и статус «подключено».</div>` : ''}`;
}

function logColor(l) {
  return l === 'WARN' ? ORANGE : l === 'ERR' ? RED : DIM;
}
function logLine(l) {
  return `<div class="l"><span class="tm">${esc(l.time)}</span><span class="lv" style="color:${logColor(l.level)}">${esc(l.level)}</span><span class="tx">${esc(l.text)}</span></div>`;
}
function renderLogs() {
  return `${backBtn()}
    <div style="display:flex;align-items:center;justify-content:space-between;margin-top:12px"><div class="h1" style="margin:0">Логи</div>
      <div style="display:flex;gap:6px"><button class="smallbtn press" data-act="copyLogs">${ic('copy2', 13)}Копировать</button><button class="smallbtn press" data-act="clearLogs">Очистить</button></div></div>
    <div class="logbox" id="logbox">${logs.length ? logs.map(logLine).join('') : '<div style="padding:16px 12px;color:var(--dim2);text-align:center">Пусто</div>'}</div>`;
}

// ─────────── Шторки «Как это работает», смена подключения, лицензии ───────────

// Пиктограмма монитора — как в макете, из двух прямоугольников.
const pc = (big) => `<div class="pc${big ? ' big' : ''}"><i></i><b></b></div>`;
// Узел схемы: значок, подпись, пояснение. x, y — в процентах поля.
const node = (x, y, iconHtml, title, sub, style = '', w = 84) => `<div class="node" style="left:${x}%;top:${y}%;width:${w}px"><div class="nic" style="${style}">${iconHtml}</div><div class="nt">${esc(title)}</div><div class="ns">${esc(sub)}</div></div>`;
const pill = (x, y, text, dot, style = '') => `<div class="flowpill" style="left:${x}%;top:${y}%;${style}">${dot ? `<span style="background:${dot}"></span>` : ''}${esc(text)}</div>`;
const path = (d, kind) => `<path d="${d}" fill="none" vector-effect="non-scaling-stroke" class="fl ${kind}"></path>`;
const flowSvg = (paths) => `<svg viewBox="0 0 100 100" preserveAspectRatio="none" class="flow">${paths.join('')}</svg>`;
const sheetWrap = (inner, closeAct) => `<div class="sheet modal"><div class="bg" data-act="${closeAct}"></div><div class="body">
  <div class="grip"></div>${inner}</div></div>`;

const MODE_INFO = {
  proxy: { top: ['Настроенные', 'браузер, торрент'], bot: ['Остальные', 'приложения'], pill: (s) => '127.0.0.1:' + s.proxyPort, split: true,
    points: [['var(--accent)', (s) => `Через VPN идут только программы, в которых вы вручную указали адрес 127.0.0.1:${s.proxyPort}.`], ['var(--dim)', () => 'Все остальные ходят напрямую через провайдера — как без VPN.']] },
  sysproxy: { top: ['Большинство', 'браузеры, Telegram'], bot: ['Игры', 'и часть программ'], pill: () => 'Системный прокси', split: true,
    points: [['var(--accent)', () => 'Windows сама передаёт адрес прокси программам — большинство подхватывает его без настройки.'], ['var(--dim)', () => 'Игры, UDP-трафик и программы, игнорирующие настройки системы, идут напрямую.']] },
  tun: { top: ['Все программы', 'включая игры'], bot: ['Службы', 'и UDP-трафик'], pill: () => 'TUN-адаптер', split: false,
    points: [['var(--accent)', () => 'Виртуальный сетевой адаптер перехватывает трафик всех программ — ничего не нужно настраивать.'], ['var(--accent)', () => (!S().routing ? 'Маршрутизация выключена — через туннель идёт всё.' : onlyChosen() ? 'Через туннель при этом идёт только то, что выбрано в «Маршрутизации», — остальное напрямую.' : 'Исключения из «Маршрутизации» (например, .ru) по-прежнему идут напрямую.')]] },
};

function modeInfoHtml() {
  const s = S(), k = ui.modeInfo, D = MODE_INFO[k];
  const svg = D.split
    ? flowSvg([path('M14 74 C32 74,32 26,50 26', 'idle'), path('M14 26 L50 26', 'go'), path('M50 26 C68 26,68 50,86 50', 'go'), path('M14 74 L50 74', 'dim'), path('M50 74 C68 74,68 50,86 50', 'dim')])
    : flowSvg([path('M14 74 L50 74', 'idle'), path('M50 74 C68 74,68 50,86 50', 'idle'), path('M14 26 L50 26', 'go'), path('M14 74 C32 74,32 26,50 26', 'go'), path('M50 26 C68 26,68 50,86 50', 'go')]);
  return sheetWrap(`<div class="mt-title">Как работает режим</div>
    <div class="seg" style="margin-top:14px">${[['proxy', 'Proxy'], ['sysproxy', 'Системный'], ['tun', 'VPN (TUN)']].map(([v, l]) => `<button class="press${k === v ? ' on' : ''}" data-act="modeInfoTab" data-v="${v}">${l}</button>`).join('')}</div>
    <div class="flowbox"><div class="field170">${svg}
      ${node(14, 26, pc(), D.top[0], D.top[1], 'background:color-mix(in srgb,var(--accent) 14%,var(--elem));color:var(--accent)')}
      ${node(14, 74, pc(), D.bot[0], D.bot[1], `color:${D.split ? 'var(--text2)' : 'var(--accent)'}`)}
      ${pill(50, 26, D.pill(s), 'var(--accent)', 'background:color-mix(in srgb,var(--accent) 16%,var(--card));border-color:color-mix(in srgb,var(--accent) 40%,transparent)')}
      ${pill(50, 74, 'Провайдер', null, `background:var(--win);color:${D.split ? 'var(--text)' : 'var(--dim2)'}`)}
      ${node(86, 50, ic('globe', 18), 'Интернет', 'сайты')}
    </div></div>
    <div class="points">${D.points.map(([dot, t]) => `<div><span style="background:${dot}"></span><div>${esc(t(s))}</div></div>`).join('')}</div>
    <div class="mbtns"><button class="mbtn press" data-act="closeModal">Понятно</button>${k !== s.mode ? `<button class="mbtn accent press" data-act="modeInfoPick">Выбрать</button>` : ''}</div>`, 'closeModal');
}

function ksInfoHtml() {
  const s = S(), on = ui.ksViz === 'on';
  const n = s.ksApps.filter((a) => a.on).length + (s.ksSites || []).filter((a) => a.on).length;
  const svg = on
    ? flowSvg([path('M16 74 L50 74', 'idle'), path('M50 74 C68 74,68 50,84 50', 'idle'), path('M16 26 L50 26', 'go'), path('M16 74 C32 74,32 26,50 26', 'go'), path('M50 26 C68 26,68 50,84 50', 'go')])
    : flowSvg([path('M50 26 C68 26,68 50,84 50', 'idle'), path('M16 74 L50 74', 'dim'), path('M50 74 C68 74,68 50,84 50', 'dim'), path('M16 26 L30 26', 'cut')]);
  return sheetWrap(`<div class="mt-title">Как работает Kill Switch</div>
    <div class="seg" style="margin-top:14px"><button class="press${on ? ' on' : ''}" data-act="ksViz" data-v="on">VPN включён</button><button class="press${!on ? ' on' : ''}" data-act="ksViz" data-v="off">VPN выключен</button></div>
    <div class="flowbox"><div class="field170">${svg}
      ${on ? '' : `<div class="cutmark">${ic('close', 10)}</div>`}
      ${node(16, 26, pc(), 'Защищённые', n + ' в списке', on ? 'color:var(--text2)' : 'background:color-mix(in srgb,var(--red) 16%,transparent);color:var(--red)', 76)}
      ${node(16, 74, pc(), 'Остальные', 'приложения', 'color:var(--text2)', 76)}
      ${pill(50, 26, 'VPN', on ? 'var(--accent)' : 'var(--ring)', on ? 'background:color-mix(in srgb,var(--accent) 16%,var(--card));border-color:color-mix(in srgb,var(--accent) 40%,transparent)' : 'background:var(--win);color:var(--dim2)')}
      ${pill(50, 74, 'Провайдер', null, `background:var(--win);color:${on ? 'var(--dim2)' : 'var(--text)'}`)}
      ${node(84, 50, ic('globe', 18), 'Интернет', 'сайты', '', 76)}
    </div></div>
    <div class="points"><div><span style="background:${on ? 'var(--accent)' : 'var(--red)'}"></span><div>${on
      ? 'Программы из списка идут только через VPN — что бы ни было в «Маршрутизации».' + (s.mode === 'tun' ? '' : ' В режимах Proxy программа, не умеющая ходить через прокси, остаётся без сети.')
      : 'Защищённые программы и сайты остаются без интернета — их данные не уйдут через провайдера. Остальные работают напрямую.'}</div></div></div>
    <button class="mbtn press" style="width:100%;margin-top:16px" data-act="closeModal">Понятно</button>`, 'closeModal');
}

function switchHtml() {
  const live = byId(status.profileId), tgt = byId(ui.switchTo);
  if (!live || !tgt) return '';
  return sheetWrap(`<div class="mt-title" style="text-wrap:pretty">Сменить подключение?</div>
    <div class="mlead">Сейчас работает <b>${esc(live.name)}</b>. Текущее соединение разорвётся на пару секунд, и трафик пойдёт через <b>${esc(tgt.name)}</b>.</div>
    <div class="flowbox" style="margin-top:16px"><div class="field150">
      ${flowSvg([path('M14 50 C32 50,30 22,50 22 C70 22,68 50,86 50', 'go'), path('M14 50 C32 50,30 78,50 78 C70 78,68 50,86 50', 'next')])}
      ${node(14, 50, pc(true), 'Компьютер', 'приложения', 'color:var(--text2)', 72)}
      <div class="flowpill wide" style="left:50%;top:22%;background:color-mix(in srgb,var(--accent) 16%,var(--card));border-color:color-mix(in srgb,var(--accent) 40%,transparent)"><span style="background:var(--accent)"></span><em>${esc(live.name)}</em></div>
      <div class="flowpill wide glow" style="left:50%;top:78%;background:var(--win)"><span class="ring"></span><em>${esc(tgt.name)}</em></div>
      ${node(86, 50, ic('globe', 20), 'Интернет', 'сайты', 'color:var(--text2)', 72)}
    </div>
    <div class="legend2"><span><i class="solid"></i>трафик сейчас</span><span><i class="dash"></i>после переключения</span></div></div>
    <div class="mbtns"><button class="mbtn press" data-act="closeModal">Отмена</button><button class="mbtn accent press" data-act="confirmSwitch">Переключить</button></div>`, 'closeModal');
}

function licensesHtml() {
  return `<div class="sheet"><div class="bg" data-act="closeModal"></div><div class="body">
    <div class="grip"></div>
    <div class="top"><div style="flex:1;min-width:0"><div style="font-size:20px;font-weight:600">Лицензии</div>
      <div style="font-size:12px;color:var(--dim);margin-top:4px">kl!ck, ядро mihomo и база GeoIP</div></div>
      <button class="x press" data-act="closeModal">${ic('close', 14)}</button></div>
    <pre class="lic">${licText == null ? 'Загружаю…' : esc(licText)}</pre>
  </div></div>`;
}

function renderModal() {
  const box = $('modal');
  const html = ui.modal === 'mode' ? modeInfoHtml() : ui.modal === 'ks' ? ksInfoHtml() : ui.modal === 'switch' ? switchHtml() : ui.modal === 'route' ? routeAskHtml() : ui.modal === 'netrule' ? netSheetHtml() : ui.modal === 'licenses' ? licensesHtml() : '';
  // Переключение вкладок внутри шторки — без повторного выезда.
  const same = !!html && box.dataset.kind === ui.modal;
  box.innerHTML = html;
  box.firstElementChild?.classList.toggle('noanim', same);
  if (!html) ui.modal = null;
  box.dataset.kind = ui.modal || '';
}
function openModal(kind) {
  hideTip();
  ui.modal = kind;
  renderModal();
}
function closeModal() {
  ui.modal = null;
  ui.switchTo = null;
  renderModal();
}

// ─────────── Шторка: выбор программ ───────────

function renderSheet() {
  const box = $('sheet');
  if (!ui.picker) { box.innerHTML = ''; return; }
  const s = S();
  const taken = new Set((ui.picker === 'rules' ? curList().apps.concat(s.ksApps.filter((a) => a.on)) : s.ksApps).map((a) => a.exe.toLowerCase()));
  const n = ui.pickerSel.length;
  box.innerHTML = `<div class="sheet"><div class="bg" data-act="closePicker"></div><div class="body">
    <div class="grip"></div>
    <div class="top"><div style="flex:1;min-width:0"><div style="font-size:20px;font-weight:600">Выбрать приложение</div>
      <div style="font-size:12px;color:var(--dim);margin-top:4px;line-height:1.45;text-wrap:pretty">${ui.picker === 'rules' ? (S().defaultRoute === 'direct' ? 'Отмеченные программы пойдут через VPN. Правило потом можно сменить.' : 'Отмеченные программы пойдут напрямую. Правило потом можно сменить.') : 'Отмеченные программы ходят только через VPN: без VPN — без сети.'}</div></div>
      <button class="x press" data-act="closePicker">${ic('close', 14)}</button></div>
    <input class="search" id="pickerQuery" placeholder="Поиск по названию или .exe" value="${esc(ui.pickerQuery)}" spellcheck="false" />
    <div class="cap"><div class="label" style="margin:0">Запущено сейчас · ${ui.pickerApps ? ui.pickerApps.length : '…'}</div><div style="font-size:12px;color:var(--dim2)">${isOn() ? 'по сетевой активности' : 'по алфавиту'}</div></div>
    <div class="list" id="pickerList">${pickerList(taken)}</div>
    <div class="foot"><button class="browse press" data-act="pickerBrowse">Обзор…</button><button class="ok press${n ? ' ready' : ''}" data-act="pickerConfirm">${n ? 'Добавить · ' + n : 'Выберите приложения'}</button></div>
  </div></div>`;
}
function pickerList(taken) {
  if (!ui.pickerApps) return '<div class="empty-note">Смотрю, что запущено…</div>';
  const q = ui.pickerQuery.trim().toLowerCase();
  const list = ui.pickerApps.filter((a) => !q || a.name.toLowerCase().includes(q) || a.exe.toLowerCase().includes(q));
  if (!list.length) return '<div style="padding:32px 16px;text-align:center"><div style="font-size:14px;font-weight:600">Ничего не найдено</div><div style="font-size:12px;color:var(--dim);margin-top:4px;line-height:1.45">Программа не запущена? Выберите её .exe вручную через «Обзор…».</div></div>';
  return list.map((a) => {
    const had = taken.has(a.exe.toLowerCase()), sel = ui.pickerSel.includes(a.exe);
    const act = had ? 'уже добавлено' : a.activity == null ? '' : a.activity >= 1024 * 1024 ? '↓ ' + (a.activity / 1024 / 1024).toFixed(1) + ' МБ/с' : a.activity > 0 ? '↓ ' + Math.max(1, Math.round(a.activity / 1024)) + ' КБ/с' : 'нет трафика';
    const i = ui.pickerApps.indexOf(a);
    return `<div class="pick${sel ? ' sel' : ''}${had ? ' had' : ''}" data-pick="${esc(a.exe)}">
      <div class="box"><span></span></div>
      <div class="tile-ic" style="background:${TINTS[i % TINTS.length]}">${esc(a.name[0].toUpperCase())}</div>
      <div style="flex:1;min-width:0"><div class="t14" style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(a.name)}</div><div class="mono" style="font-size:11px;color:var(--dim2);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(a.exe)} · PID ${a.pid}</div></div>
      <div class="act${a.activity >= 1024 * 1024 && !had ? ' hot' : ''}">${act}</div></div>`;
  }).join('');
}
async function openPicker(kind) {
  ui.picker = kind; ui.pickerQuery = ''; ui.pickerSel = []; ui.pickerApps = null;
  renderSheet();
  try { ui.pickerApps = await kl.runningApps(); } catch { ui.pickerApps = []; }
  if (ui.picker) renderSheet();
}
async function addPicked(apps) {
  const s = S();
  if (ui.picker === 'rules') {
    // Новое правило — противоположное положению: иначе оно ничего не меняло бы.
    const action = s.defaultRoute === 'direct' ? 'proxy' : 'direct';
    await saveList({ apps: [...curList().apps, ...apps.map((a) => ({ name: a.name, exe: a.exe, path: a.path || '', action }))] });
    toast(apps.length === 1 ? 'Программа добавлена' : 'Добавлено программ: ' + apps.length, action === 'proxy' ? 'Пойдёт через VPN. Правило можно сменить в строке.' : 'Пойдёт напрямую. Правило можно сменить в строке.', GREEN);
  } else {
    await save({ ksApps: [...s.ksApps, ...apps.map((a) => ({ name: a.name, exe: a.exe, path: a.path, on: true }))] });
    toast(apps.length === 1 ? 'Приложение добавлено' : 'Добавлено приложений: ' + apps.length, 'Только через VPN: без VPN — без сети.', GREEN);
  }
  ui.picker = null;
  renderSheet();
  render();
}

// ─────────── Навигация и отрисовка ───────────

const NAV = [['home', 'home', 'Главная'], ['rules', 'rules2', 'Маршрутизация'], ['net', 'pulse', 'Соединение'], ['add', 'plus', 'Добавить'], ['settings', 'settings', 'Настройки']];
function render() {
  if (!ov) return;
  const html = ui.screen === 'home' ? renderHome() : ui.screen === 'add' ? renderAdd() : ui.screen === 'rules' ? renderRules() : ui.screen === 'net' ? renderNet() : renderSettings();
  $('screen').innerHTML = html;
  renderNav();
  if (ui.screen === 'add' && ui.addTab === 'link') updateAddDynamic();
  if (ui.screen === 'home') updateLive();
}
function renderNav() {
  // Точка — когда тост о проблеме уже ушёл, а раздел ещё не открывали.
  const badge = ksIssue && !ksSeen && !toasts.some((t) => t.id === ksToastId);
  $('nav').innerHTML = NAV.map(([k, i, t]) => `<button class="${ui.screen === k ? 'on' : ''}" data-nav="${k}" data-tip="${t}" aria-label="${t}">${ic(i, 24)}${k === 'settings' && badge ? '<i class="badge"></i>' : ''}</button>`).join('');
}

function setKsIssue(issue) {
  if (issue && issue !== ksIssue) {
    ksSeen = ui.screen === 'settings' && ui.sub === 'kill';
    ksToastId = toast('Kill Switch не применился', issue, ORANGE, { label: 'Открыть', run: () => go('settings', 'kill') });
  }
  if (!issue && ksIssue) toast('Kill Switch работает', 'Правила брандмауэра на месте.', GREEN);
  ksIssue = issue || null;
  if (!ksIssue) ksSeen = true;
  if (ui.screen === 'settings') render(); else renderNav();
}

function go(screen, sub = null) {
  if (screen === 'settings' && sub === 'kill') ksSeen = true;
  const same = ui.screen === screen && ui.sub === sub;
  hideTip();
  ui.screen = screen; ui.sub = sub; ui.menuOpen = false; ui.confirmDel = false;
  if (sub === 'about' && !same) loadInfo();
  if (screen === 'rules' && !same) loadRouting();
  if (screen === 'net' && !same) loadNet();
  if (!same) $('scroll').scrollTop = 0;
  // Анимация входа — только при смене экрана.
  $('screen').className = '';
  void $('screen').offsetWidth;
  $('screen').className = 'screen';
  render();
}
$('nav').addEventListener('click', (e) => { const b = e.target.closest('[data-nav]'); if (b) go(b.dataset.nav); });

async function refresh() {
  ov = await kl.overview();
  if (ui.screen === 'rules') loadRouting();
  if ((ov.killSwitchIssue || null) !== ksIssue) setKsIssue(ov.killSwitchIssue);
  // Один раз после обновления: что поменялось в настройках.
  if (ov.notice) toast('Настройки обновлены', ov.notice, DIM);
  status = ov.status;
  traffic = ov.traffic || traffic;
  render();
}

// Ядро применяет новые правила к новым соединениям — уже открытые (игра,
// звонок в Discord) живут по старым, пока программу не перезапустят.
const RULE_KEYS = ['lists', 'sets', 'routing', 'defaultRoute', 'ksApps'];
let ruleHintShown = false;

// Правка списка текущего положения.
const saveList = (patch) => save({ lists: { ...S().lists, [S().defaultRoute]: { ...curList(), ...patch } } });

async function save(patch) {
  try {
    const r = await kl.updateSettings(patch);
    ov.settings = r.settings;
    if (r.ksShared) ov.ksShared = r.ksShared;
    if (ui.screen === 'rules') loadRouting();
    if (!ruleHintShown && isOn() && RULE_KEYS.some((k) => k in patch)) {
      ruleHintShown = true;
      setTimeout(() => toast('Правила применены', 'Уже открытые соединения идут по-старому — перезапустите игру или программу, чтобы сразу по-новому.', DIM), 900);
    }
    return true;
  } catch (e) {
    toast('Не сохранилось', errText(e), RED);
    return false;
  } finally {
    render();
  }
}

async function togglePower() {
  if (!profiles().length) return toast('Нет подключений', 'Сначала добавьте подписку или конфигурацию.', RED);
  const p = active();
  // VPN работает через другое подключение — сперва спросить.
  if (elsewhere(p)) {
    ui.switchTo = p.id;
    return openModal('switch');
  }
  if (isOn() || isBusy()) {
    await kl.disconnect();
    const s = S(), n = s.ksApps.filter((a) => a.on).length + (s.ksSites || []).filter((a) => a.on).length;
    if (s.killSwitch && n) toast('Kill Switch активен', `${ksCount(s)} без интернета до включения VPN.`, ORANGE);
    return;
  }
  ui.powerBusy = true; render();
  try {
    if (p.id !== ov.activeProfile) { await kl.selectProfile(p.id); ov.activeProfile = p.id; }
    ui.viewId = null;
    await kl.connect();
  } catch { /* причину показывает карточка ошибки */ }
  ui.powerBusy = false; render();
}

async function doSwitch() {
  const tgt = byId(ui.switchTo);
  closeModal();
  if (!tgt) return;
  ui.viewId = null;
  ov.activeProfile = tgt.id;
  ui.powerBusy = true; render();
  try {
    await kl.selectProfile(tgt.id);
    toast('Подключено к ' + tgt.name, '', GREEN);
  } catch (e) {
    toast('Не переключилось', errText(e), RED);
  }
  ui.powerBusy = false;
  await refresh();
}

async function loadInfo() {
  try { info = await kl.appInfo(); } catch { return; }
  if (ui.screen === 'settings' || ui.screen === 'rules') render();
}

async function doPing() {
  const p = active();
  if (!p || ui.pinging) return;
  ui.pinging = true; render();
  try { pings[p.id] = await kl.ping(p.id); } catch (e) { toast('Задержку не проверить', errText(e), RED); }
  ui.pinging = false; render();
}

async function doRefreshSub() {
  const p = active();
  if (!p || ui.refreshing) return;
  ui.refreshing = true; ui.menuOpen = false; render();
  try {
    const n = await kl.refreshSub(p.id);
    toast('Подписка обновлена', `Серверов: ${n}. Список и лимиты актуальны.`, GREEN);
  } catch (e) {
    toast('Не удалось обновить подписку', errText(e), RED);
  }
  ui.refreshing = false;
  await refresh();
}

async function doAddLink() {
  const d = detect(ui.linkInput);
  if (!d || d.kind === 'bad' || ui.adding) return;
  ui.adding = true; updateAddDynamic();
  try {
    const r = await kl.addLink(ui.linkInput, d.kind === 'sub' ? ui.nameInput : '');
    ui.linkInput = ''; ui.nameInput = '';
    ui.expanded = true;
    await refresh();
    go('home');
    toast('Подключение добавлено', r.kind === 'sub' ? `Подписка загружена: ${r.servers} ${plural(r.servers, 'сервер', 'сервера', 'серверов')}. Проверьте задержку.` : 'Готово к подключению.', GREEN);
  } catch (e) {
    toast('Не добавилось', errText(e), RED);
  }
  ui.adding = false; updateAddDynamic();
}

async function addFromPath(path) {
  try {
    const r = await kl.addFile(path);
    ui.expanded = true;
    await refresh();
    go('home');
    toast('Файл импортирован', `${path.split(/[\\/]/).pop()} · ${r.servers} ${plural(r.servers, 'сервер', 'сервера', 'серверов')}.`, GREEN);
  } catch (e) {
    toast('Файл не подошёл', errText(e), RED);
  }
}

// ─────────── Клики ───────────

const actions = {
  goAdd: () => go('add'),
  goMode: () => go('settings', 'mode'),
  power: togglePower,
  toggleExpand: () => { ui.expanded = !ui.expanded; render(); },
  openMenu: (el, e) => { e.stopPropagation(); ui.menuOpen = !ui.menuOpen; ui.confirmDel = false; render(); },
  closeMenu: () => { ui.menuOpen = false; ui.confirmDel = false; render(); },
  confirmDel: () => { ui.confirmDel = true; render(); },
  removeProfile: async () => {
    const p = active();
    ui.menuOpen = false; ui.confirmDel = false; ui.expanded = false;
    try {
      const r = await kl.removeProfile(p.id);
      ui.viewId = null;
      await refresh();
      toast(p.kind === 'sub' ? 'Подписка удалена' : 'Конфигурация удалена', r.was_live ? p.name + ' · соединение разорвано' : p.name, DIM, {
        label: 'Отменить', run: async () => { await kl.restoreProfile(r.profile, r.index, true); await refresh(); },
      });
    } catch (e) { toast('Не удалилось', errText(e), RED); }
  },
  refreshSub: doRefreshSub,
  ping: doPing,
  pingMenu: () => { ui.menuOpen = false; ui.expanded = true; doPing(); },
  copyLink: async () => {
    ui.menuOpen = false; render();
    try { await kl.copyText(await kl.profileLink(active().id)); toast('Скопировано', active().kind === 'sub' ? 'Ссылка на подписку в буфере обмена.' : 'Конфигурация в буфере обмена.', GREEN); } catch (e) { toast('Не скопировалось', errText(e), RED); }
  },
  addTab: (el) => { ui.addTab = el.dataset.v; render(); },
  addLink: doAddLink,
  addFile: async () => { const p = await kl.pickConfig(); if (p) addFromPath(p); },
  rulesTab: (el) => { ui.rulesTab = el.dataset.v; render(); },
  routingToggle: () => askRoute(!S().routing, S().defaultRoute),
  routePos: (el) => { if (S().routing && el.dataset.v !== S().defaultRoute) askRoute(true, el.dataset.v); },
  confirmRoute: async () => {
    const a = ui.routeAsk;
    closeModal();
    if (a && (await save({ routing: a.routing, defaultRoute: a.route }))) toast(a.routing ? 'Маршрутизация: «' + POS_NAMES[a.route] + '»' : 'Маршрутизация выключена', a.routing ? '' : 'Всё идёт через VPN.', GREEN);
  },
  toggleSet: () => { const k = S().defaultRoute === 'proxy' ? 'ru' : 'blocked'; save({ sets: { ...S().sets, [k]: !S().sets[k] } }); },
  addSite: () => {
    const v = ui.siteInput.trim();
    if (!v) return $('siteInput')?.focus();
    ui.siteInput = '';
    // Новое правило — противоположное положению: иначе оно ничего не меняло бы.
    saveList({ sites: [{ pattern: v, action: S().defaultRoute === 'direct' ? 'proxy' : 'direct' }, ...curList().sites.filter((r) => r.pattern !== v.toLowerCase())] });
  },
  cycleRule: async (el) => {
    const s = S(), kind = el.dataset.kind, key = el.dataset.key, st = el.dataset.state;
    if (kind === 'site') {
      const next = { vpn: 'direct', direct: 'block', block: 'proxy' }[st];
      return saveList({ sites: curList().sites.map((r) => (r.pattern === key ? { ...r, action: next } : r)) });
    }
    // Программа: через VPN → напрямую → блок → только через VPN → через VPN.
    if (st === 'ks') {
      const k = s.ksApps.find((a) => a.on && a.exe === key);
      return save({
        ksApps: s.ksApps.filter((a) => a !== k),
        lists: { ...s.lists, [s.defaultRoute]: { ...curList(), apps: [...curList().apps, { name: k.name, exe: k.exe, path: k.path, action: 'proxy' }] } },
      });
    }
    const r = curList().apps.find((a) => a.exe === key);
    if (st === 'block') {
      if (!r.path) {
        toast('«Только через VPN» недоступно', 'Для него нужен путь к программе: удалите её из списка и добавьте заново через «Выбрать программу».', ORANGE);
        return saveList({ apps: curList().apps.map((a) => (a === r ? { ...a, action: 'proxy' } : a)) });
      }
      const ok = await save({
        ksApps: [...s.ksApps.filter((a) => a.exe !== r.exe), { name: r.name, exe: r.exe, path: r.path, on: true }],
        lists: { ...s.lists, [s.defaultRoute]: { ...curList(), apps: curList().apps.filter((a) => a !== r) } },
      });
      if (ok && !S().killSwitch) toast('Kill Switch выключен', 'Без VPN программа пойдёт напрямую, пока его не включить.', ORANGE, { label: 'Включить', run: () => save({ killSwitch: true }) });
      return;
    }
    saveList({ apps: curList().apps.map((a) => (a === r ? { ...a, action: st === 'vpn' ? 'direct' : 'block' } : a)) });
  },
  removeRule: (el) => {
    const kind = el.dataset.kind === 'app' ? 'apps' : 'sites', i = +el.dataset.i;
    const item = curList()[kind][i], pos = S().defaultRoute;
    saveList({ [kind]: curList()[kind].filter((_, j) => j !== i) });
    toast('Правило удалено', item.pattern || item.name, DIM, {
      label: 'Отменить', run: () => { if (S().defaultRoute !== pos) return; const arr = [...curList()[kind]]; arr.splice(Math.min(i, arr.length), 0, item); saveList({ [kind]: arr }); },
    });
  },
  netToggle: (el) => { ui.netOpen[el.dataset.v] = !ui.netOpen[el.dataset.v]; render(); },
  netRule: (el) => {
    const n = ui.net, kind = el.dataset.kind;
    if (kind === 'app') {
      const a = n?.apps.find((x) => x.exe === el.dataset.v);
      if (!a) return;
      ui.netSheet = { kind, key: a.exe, name: a.name, path: a.path, cur: a.route === 'mixed' ? 'mixed' : a.route, why: a.why };
    } else {
      const a = n?.apps.find((x) => x.exe === el.dataset.app);
      const h = a?.hosts.find((x) => x.host === el.dataset.v);
      ui.netSheet = { kind, key: el.dataset.v, name: el.dataset.v, cur: h?.route || 'vpn', why: h?.why || '' };
    }
    openModal('netrule');
  },
  netFix: (el) => {
    const f = ui.net?.failures[+el.dataset.i];
    if (!f) return;
    ui.netSheet = { kind: 'site', key: f.host, name: f.host, cur: f.route, why: f.why };
    if (el.dataset.v === 'unblock') return netApply('unblock');
    openModal('netrule');
  },
  netApply: (el) => netApply(el.dataset.v),
  netIp: async () => {
    ui.ipBusy = true; render();
    try { ui.ip = await kl.checkIp(); } catch (e) { toast('Не получилось проверить', errText(e), RED); }
    ui.ipBusy = false; render();
  },
  checkRoute: async () => { try { ui.checkRes = await kl.checkRoute(ui.checkInput); } catch { ui.checkRes = null; } render(); },
  checkSample: async (el) => { ui.checkInput = el.dataset.v; actions.checkRoute(); },
  pickApps: (el) => openPicker(el.dataset.v),
  closePicker: () => { ui.picker = null; renderSheet(); },
  pickerBrowse: async () => {
    const p = await kl.pickExe();
    if (!p) return;
    const app = await kl.appFromFile(p);
    addPicked([app]);
  },
  pickerConfirm: () => {
    if (!ui.pickerSel.length) return;
    addPicked(ui.pickerApps.filter((a) => ui.pickerSel.includes(a.exe)));
  },
  back: () => go('settings'),
  goModeSub: () => go('settings', 'mode'),
  goKill: () => go('settings', 'kill'),
  goTheme: () => go('settings', 'theme'),
  goLogs: () => go('settings', 'logs'),
  goAbout: () => go('settings', 'about'),
  // Шторки
  closeModal,
  modeInfo: (el, e) => { e.stopPropagation(); ui.modeInfo = el.dataset.v; openModal('mode'); },
  modeInfoTab: (el) => { ui.modeInfo = el.dataset.v; renderModal(); },
  modeInfoPick: () => { const k = ui.modeInfo; closeModal(); if (k !== S().mode) save({ mode: k }); },
  ksInfo: () => { ui.ksViz = isOn() ? 'on' : 'off'; openModal('ks'); },
  ksViz: (el) => { ui.ksViz = el.dataset.v; renderModal(); },
  confirmSwitch: doSwitch,
  // О приложении
  checkUpdate: async () => {
    if (ui.updChecking) return;
    ui.updChecking = true; render();
    try {
      const u = await kl.checkUpdate();
      if (u.newer) toast('Доступна версия ' + u.latest, 'Сейчас установлена ' + u.current + '.', GREEN, { label: 'Открыть', run: () => kl.openUrl(u.url) });
      else toast('Обновлений нет', 'У вас последняя версия ' + u.current + '.', GREEN);
    } catch (e) { toast('Не удалось проверить', errText(e), RED); }
    ui.updChecking = false; render();
  },
  updateGeo: async () => {
    if (ui.geoBusy) return;
    ui.geoBusy = true; render();
    try {
      const g = await kl.updateGeo();
      if (info) info.geo = g;
      toast('База GeoIP обновлена', 'Российские адреса определяются по свежим данным.', GREEN);
    } catch (e) { toast('База GeoIP не обновилась', errText(e), RED); }
    ui.geoBusy = false; render();
  },
  openChangelog: () => kl.openUrl((info?.repo || 'https://github.com/vbu00/klick') + '/releases/tag/v' + (info?.version || ov.appVersion)).catch((e) => toast('Не открылось', errText(e), RED)),
  openRepo: () => kl.openUrl(info?.repo || 'https://github.com/vbu00/klick').catch((e) => toast('Не открылось', errText(e), RED)),
  openLicenses: async () => {
    openModal('licenses');
    if (licText == null) { try { licText = await kl.licenses(); } catch (e) { licText = errText(e); } if (ui.modal === 'licenses') renderModal(); }
  },
  openData: () => kl.openDataDir(),
  general: async (el) => {
    const k = el.dataset.v;
    if (k === 'autostart') {
      try { ov.autostart = await kl.setAutostart(!ov.autostart); } catch (e) { toast('Автозапуск не изменился', errText(e), RED); }
      render();
    } else save({ [k]: !S()[k] });
  },
  mode: (el) => { if (el.dataset.v !== S().mode) save({ mode: el.dataset.v }); },
  toggleKill: () => save({ killSwitch: !S().killSwitch }),
  retryKs: async (el) => {
    el.disabled = true;
    el.textContent = 'Применяю…';
    try { setKsIssue(await kl.retryKillSwitch()); } catch (e) { toast('Не получилось', errText(e), RED); }
    render();
  },
  ksTab: (el) => { ui.ksTab = el.dataset.v; render(); },
  addKsSite: () => {
    const raw = ui.ksSiteInput.trim();
    if (!raw) return $('ksSiteInput')?.focus();
    const d = raw.toLowerCase().replace(/^[a-z]+:\/\//, '').replace(/[/?#].*$/, '').replace(/:\d+$/, '').replace(/^\*?\./, '').replace(/\.$/, '');
    if (!/^([a-z0-9-]+\.)+[a-z0-9-]+$/.test(d) || /^[\d.]+$/.test(d)) return toast('Нужен адрес сайта', 'Например, sberbank.ru — зону целиком (.ru) Kill Switch закрыть не может.', ORANGE);
    if ((S().ksSites || []).some((x) => x.pattern === d)) return toast('Уже в списке', d);
    ui.ksSiteInput = '';
    save({ ksSites: [{ pattern: d, on: true }, ...(S().ksSites || [])] });
  },
  ksSite: (el) => { const i = +el.dataset.i; save({ ksSites: S().ksSites.map((a, j) => (j === i ? { ...a, on: !a.on } : a)) }); },
  removeKsSite: (el) => {
    const i = +el.dataset.i, item = S().ksSites[i];
    save({ ksSites: S().ksSites.filter((_, j) => j !== i) });
    toast('Удалено', item.pattern, DIM, { label: 'Отменить', run: () => { const arr = [...S().ksSites]; arr.splice(Math.min(i, arr.length), 0, item); save({ ksSites: arr }); } });
  },
  ksApp: (el) => { const i = +el.dataset.i; save({ ksApps: S().ksApps.map((a, j) => (j === i ? { ...a, on: !a.on } : a)) }); },
  removeKs: (el) => {
    const i = +el.dataset.i, item = S().ksApps[i];
    save({ ksApps: S().ksApps.filter((_, j) => j !== i) });
    toast('Удалено', item.name, DIM, { label: 'Отменить', run: () => { const arr = [...S().ksApps]; arr.splice(Math.min(i, arr.length), 0, item); save({ ksApps: arr }); } });
  },
  theme: (el) => { T.save({ theme: el.dataset.v }); render(); },
  themeBase: (el) => { T.save({ base: el.dataset.v }); render(); },
  accent: (el) => { T.save({ accent: el.dataset.v }); render(); },
  copyLogs: async () => { await kl.copyText(logs.map((l) => `${l.time} ${l.level} ${l.text}`).join('\n')); toast('Скопировано', `${logs.length} строк в буфере обмена.`, GREEN); },
  clearLogs: async () => {
    const prev = logs;
    logs = [];
    await kl.clearLogs();
    render();
    toast('Логи очищены', prev.length + ' записей', DIM, { label: 'Отменить', run: async () => { await kl.restoreLogs(prev); logs = [...prev, ...logs]; render(); } });
  },
};

document.addEventListener('click', (e) => {
  const prof = e.target.closest('[data-profile]');
  if (prof) {
    const id = prof.dataset.profile;
    ui.expanded = false; ui.menuOpen = false;
    if (isOn() || isBusy()) {
      // VPN работает — только смотрим; переключение — кнопкой питания.
      ui.viewId = id === status.profileId ? null : id;
    } else {
      ui.viewId = null;
      ov.activeProfile = id;
      kl.selectProfile(id).then(refresh).catch((err) => toast('Не переключилось', errText(err), RED));
    }
    render();
    return;
  }
  const srv = e.target.closest('[data-server]');
  if (srv) {
    const p = active(), name = srv.dataset.server;
    if (name === p.active) return;
    p.active = name;
    render();
    kl.selectServer(p.id, name)
      .then(() => { if (liveOn(p)) toast('Сервер сменён', name, GREEN); })
      .catch((err) => toast('Сервер не переключился', errText(err), RED));
    return;
  }
  const pick = e.target.closest('[data-pick]');
  if (pick) {
    if (pick.classList.contains('had')) return;
    const exe = pick.dataset.pick;
    ui.pickerSel = ui.pickerSel.includes(exe) ? ui.pickerSel.filter((x) => x !== exe) : [...ui.pickerSel, exe];
    renderSheet();
    return;
  }
  const a = e.target.closest('[data-act]');
  if (a && actions[a.dataset.act]) actions[a.dataset.act](a, e);
});
// Правый клик по карточке — то же меню, что и «⋯».
document.addEventListener('contextmenu', (e) => {
  e.preventDefault();
  if (e.target.closest('.phead')) { ui.menuOpen = true; ui.confirmDel = false; render(); }
});

document.addEventListener('input', (e) => {
  const id = e.target.id;
  if (id === 'linkInput') { ui.linkInput = e.target.value; updateAddDynamic(); }
  else if (id === 'nameInput') ui.nameInput = e.target.value;
  else if (id === 'siteInput') ui.siteInput = e.target.value;
  else if (id === 'checkInput') ui.checkInput = e.target.value;
  else if (id === 'ksSiteInput') ui.ksSiteInput = e.target.value;
  else if (id === 'pickerQuery') {
    ui.pickerQuery = e.target.value;
    const s = S();
    $('pickerList').innerHTML = pickerList(new Set((ui.picker === 'rules' ? curList().apps.concat(s.ksApps.filter((a) => a.on)) : s.ksApps).map((a) => a.exe.toLowerCase())));
  }
});
document.addEventListener('change', (e) => {
  if (e.target.id !== 'portInput') return;
  const v = parseInt(e.target.value, 10);
  if (!(v >= 1024 && v <= 65535)) { toast('Порт — от 1024 до 65535', '', ORANGE); e.target.value = S().proxyPort; return; }
  save({ proxyPort: v });
});
document.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && e.target.id === 'siteInput') actions.addSite();
  if (e.key === 'Enter' && e.target.id === 'checkInput') actions.checkRoute();
  if (e.key === 'Enter' && e.target.id === 'ksSiteInput') actions.addKsSite();
  if (e.key === 'Enter' && e.target.id === 'linkInput' && (e.ctrlKey || !ui.linkInput.includes('\n'))) { e.preventDefault(); doAddLink(); }
  if (e.key === 'Escape') {
    if (ui.modal) closeModal();
    else if (ui.picker) { ui.picker = null; renderSheet(); }
    else if (ui.menuOpen) actions.closeMenu();
  }
});

$('minBtn').onclick = () => kl.minimize();
$('closeBtn').onclick = () => kl.close();

// Перетаскивание файла — на экране «Добавить» / «Из файла».
kl.onFileDrop?.((ev) => {
  const drop = $('drop');
  if (ev.type === 'over' || ev.type === 'enter') drop?.classList.add('over');
  else if (ev.type === 'leave') drop?.classList.remove('over');
  else if (ev.type === 'drop') {
    drop?.classList.remove('over');
    if (ev.paths?.length) addFromPath(ev.paths[0]);
  }
});

// ─────────── События ядра ───────────

kl.on('core-status', (s) => {
  const wasOn = isOn();
  status = s;
  if (wasOn && !isOn()) { hist.dl.fill(0); hist.ul.fill(0); traffic = { up: 0, down: 0, upTotal: 0, downTotal: 0 }; }
  if (ui.screen === 'home' || ui.screen === 'rules') render();
});
kl.on('traffic', (t) => {
  traffic = t;
  hist.dl.push(t.down); hist.dl.shift();
  hist.ul.push(t.up); hist.ul.shift();
  if (ui.screen === 'home') updateLive();
});
kl.on('logs', (batch) => {
  logs = [...logs, ...batch].slice(-500);
  const box = $('logbox');
  if (box) {
    const atEnd = $('scroll').scrollTop + $('scroll').clientHeight >= $('scroll').scrollHeight - 30;
    if (box.firstElementChild && !box.firstElementChild.classList.contains('l')) box.innerHTML = '';
    box.insertAdjacentHTML('beforeend', batch.map(logLine).join(''));
    while (box.children.length > 500) box.removeChild(box.firstChild);
    if (atEnd) $('scroll').scrollTop = $('scroll').scrollHeight;
  }
});
kl.on('pings', (m) => {
  const p = byId(status.profileId);
  if (p && status.profileId === p.id) { pings[p.id] = { ...(pings[p.id] || {}), ...m }; if (ui.screen === 'home' && !ui.menuOpen) render(); }
});
kl.on('profiles-changed', refresh);
kl.on('killswitch', (issue) => setKsIssue(issue));

setInterval(() => { if (isOn() && ui.screen === 'home') updateLive(); }, 1000);
// «Соединение»: живой список, пока страница открыта и окно не свернуло
// правило в окошке.
setInterval(() => { if (ui.screen === 'net' && isOn() && !ui.modal && !document.hidden) loadNet(); }, 2000);

(async () => {
  await refresh();
  loadInfo();
  logs = await kl.getLogs();
  if (ui.screen === 'settings') render();
})();
