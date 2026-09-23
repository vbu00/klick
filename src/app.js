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
const ACTIONS = [['proxy', 'Через VPN'], ['direct', 'Напрямую'], ['block', 'Блок']];
const MODE_NAMES = { proxy: 'Proxy · порт', sysproxy: 'Системный proxy', tun: 'VPN (TUN)' };
const ROUTE_NAMES = { rule: 'по правилам', global: 'всё через VPN', direct: 'всё напрямую' };

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
  pinging: false, refreshing: false, adding: false, powerBusy: false,
};
let toasts = [];
let toastId = 0;
// Проблема Kill Switch: тост, потом точка на «Настройках», пока её не
// посмотрели; строка Kill Switch подсвечена, пока проблема не ушла.
let ksIssue = null;
let ksSeen = true;
let ksToastId = 0;

const S = () => ov.settings;
const profiles = () => ov?.profiles || [];
const active = () => profiles().find((p) => p.id === ov.activeProfile) || profiles()[0] || null;
const isOn = () => status.state === 'on';
const isBusy = () => status.state === 'connecting' || status.state === 'reconnecting';
const liveOn = (p) => isOn() && p && status.profileId === p.id;

// ─────────── Форматирование ───────────

// Скорость — в мегабитах, как в макете.
const fmtRate = (bytes) => { const mb = (bytes * 8) / 1e6; return mb >= 1000 ? (mb / 1000).toFixed(2) + ' Gb/s' : mb.toFixed(2) + ' Mb/s'; };
const fmtBytes = (b) => (b >= 1024 ** 3 ? (b / 1024 ** 3).toFixed(2) + ' ГБ' : (b / 1024 ** 2).toFixed(1) + ' МБ');
const gb = (b) => { const v = b / 1024 ** 3; return v >= 100 ? Math.round(v) : +v.toFixed(1); };
const pingColor = (p) => (p == null ? RED : p < 80 ? GREEN : p < 160 ? 'var(--text)' : ORANGE);
const pingText = (p) => (p == null ? 'нет ответа' : p + ' мс');
const plural = (n, a, b, c) => { const m10 = n % 10, m100 = n % 100; return m10 === 1 && m100 !== 11 ? a : m10 >= 2 && m10 <= 4 && (m100 < 10 || m100 >= 20) ? b : c; };
function timerText() {
  const h = isOn() && status.since ? Math.max(0, Math.floor(Date.now() / 1000 - status.since)) : 0;
  return `${Math.floor(h / 3600)}:${String(Math.floor(h / 60) % 60).padStart(2, '0')}:${String(h % 60).padStart(2, '0')}`;
}
function modeLabel() {
  const s = S();
  const m = s.mode === 'proxy' ? `Proxy · порт ${s.proxyPort}` : MODE_NAMES[s.mode];
  return `${m} · ${ROUTE_NAMES[s.routeMode]}`;
}

// ─────────── Тосты ───────────

function toast(title, text = '', color = DIM, action = null) {
  const id = ++toastId, ms = action ? 5000 : 4200;
  toasts = [...toasts.slice(-2), { id, title, text, color, action, ms }];
  renderToasts();
  setTimeout(() => { toasts = toasts.filter((t) => t.id !== id); renderToasts(); renderNav(); }, ms);
  return id;
}
function renderToasts() {
  $('toasts').innerHTML = toasts.map((t) => `<div class="toast" data-id="${t.id}">
    <div class="dot" style="background:${t.color}"></div>
    <div style="flex:1;min-width:0"><div class="tt">${esc(t.title)}</div>${t.text ? `<div class="tx">${esc(t.text)}</div>` : ''}</div>
    ${t.action ? `<button class="act press" data-tact="${t.id}">${esc(t.action.label)}</button><div class="bar" style="animation-duration:${t.ms}ms"></div>` : ''}
    <button class="x press" data-tclose="${t.id}">${ic('close', 14)}</button></div>`).join('');
}
$('toasts').addEventListener('click', (e) => {
  const a = e.target.closest('[data-tact]');
  const c = e.target.closest('[data-tclose]');
  const id = +(a?.dataset.tact || c?.dataset.tclose || 0);
  if (!id) return;
  const t = toasts.find((x) => x.id === id);
  toasts = toasts.filter((x) => x.id !== id);
  renderToasts();
  renderNav();
  if (a && t?.action) t.action.run();
});

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
  const on = isOn(), busy = isBusy() || ui.powerBusy;
  const bad = status.state === 'error' || (on && status.health && !status.health.ok);
  const isSub = p.kind === 'sub';
  const multi = p.servers.length > 1 || isSub;
  const pp = pings[p.id] || {};
  const livePing = liveOn(p) && status.health ? (status.health.ok ? status.health.ms : null) : undefined;
  const activePing = ui.pinging ? '…' : pp[p.active] !== undefined ? pingText(pp[p.active]) : livePing !== undefined ? pingText(livePing) : '—';
  const activePingColor = ui.pinging ? DIM : pp[p.active] !== undefined ? pingColor(pp[p.active]) : livePing !== undefined ? pingColor(livePing) : DIM;
  const server = p.servers.find((s) => s.name === p.active) || p.servers[0];
  const subLine = isSub || multi ? server?.name || '' : 'IP: ' + (server?.host || '').replace(/:\d+$/, '');
  const statusLabel = on ? 'время подключения' : status.state === 'connecting' ? 'подключение…' : status.state === 'reconnecting' ? 'переподключение…' : 'не подключено';
  const powerCls = on ? 'on' : busy ? 'busy' : status.state === 'error' ? 'bad' : '';

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

  const notice = status.state === 'error' && status.error
    ? `<div class="errbox"><div class="dot" style="background:var(--red);margin-top:5px"></div><div>${esc(status.error)}</div></div>`
    : status.warning ? `<div class="errbox warn"><div class="dot" style="background:var(--orange);margin-top:5px"></div><div>${esc(status.warning)}</div></div>`
    : on && status.health && !status.health.ok ? `<div class="errbox warn"><div class="dot" style="background:var(--orange);margin-top:5px"></div><div>Туннель поднят, но через «${esc(status.server || '')}» ничего не открывается. Выберите другой сервер.</div></div>` : '';

  return `
    <div class="status"><div class="lbl">${statusLabel}</div><div class="timer" id="timer">${timerText()}</div></div>
    <div class="power-wrap">
      ${on && !bad ? '<div class="waves"><div class="grid"></div><div class="wave"></div><div class="ringfx"></div></div>' : ''}
      <button class="power ${powerCls}" data-act="power" title="${on || busy ? 'Отключить' : 'Подключить'}"><span class="disc">${ic('power', 34)}</span></button>
    </div>
    <button class="modechip press" data-act="goMode"><span class="d" style="background:${on ? (bad ? RED : GREEN) : busy ? ORANGE : DIM}"></span>${esc(modeLabel())}</button>
    ${notice}
    ${profiles().length > 1 ? `<div class="chips">${profiles().map((x) => `<button class="chip press${x.id === p.id ? ' on' : ''}" data-profile="${x.id}">${esc(x.name)}</button>`).join('')}</div>` : ''}
    <div class="pcard" style="margin-top:${profiles().length > 1 ? 10 : 34}px">
      <div class="card">
        <div class="phead" data-act="toggleExpand">
          <div class="picon">${ic(isSub ? 'globe' : 'rules2', 24)}</div>
          <div style="flex:1;min-width:0">
            <div class="pname">${esc(p.name)}</div>
            <div class="psub"><span class="s">${esc(subLine)}</span><span class="sep"></span><span class="p" style="color:${activePingColor}">${activePing}</span></div>
          </div>
          <button class="menubtn press${ui.menuOpen ? ' on' : ''}" data-act="openMenu" title="Действия">${ic('dots', 20)}</button>
        </div>
        ${trafficHtml}
        ${expandHtml}
      </div>
      ${menuHtml}
    </div>
    <div class="speed">
      <div class="card"><div class="hd">Чтение${ic('cloud-down', 22)}</div><div class="v" id="dlText">${fmtRate(on ? traffic.down : 0)}</div><div class="s" id="dlTotal">всего ${fmtBytes(traffic.downTotal || 0)}</div></div>
      <div class="card"><div class="hd">Загрузка${ic('cloud-up', 22)}</div><div class="v" id="ulText">${fmtRate(on ? traffic.up : 0)}</div><div class="s" id="ulTotal">всего ${fmtBytes(traffic.upTotal || 0)}</div></div>
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

function renderRules() {
  const s = S();
  const summary = s.routeMode === 'global' ? 'Сейчас режим «Глобально»: всё идёт через VPN, исключения ниже не применяются. Изменить — в настройках режима.'
    : s.routeMode === 'direct' ? 'Сейчас режим «Напрямую»: VPN не используется ни для чего. Правила ниже сохраняются, но не действуют.'
    : 'По умолчанию весь трафик идёт через VPN. Ниже — что должно идти напрямую или блокироваться. Правила для приложений важнее правил для сайтов.';
  const presets = [['ru', 'Домены .ru и .рф — напрямую', 'Госуслуги, банки, Яндекс без VPN'], ['lan', 'Локальная сеть — напрямую', 'Принтеры, NAS, 192.168.x.x'], ['geoip', 'Российские IP — напрямую', 'По базе GeoIP, даже без .ru в адресе']];
  const seg = (kind, i, cur) => `<div class="seg small">${ACTIONS.map(([v, l]) => `<button class="press${cur === v ? ' on' : ''}" data-act="ruleAction" data-kind="${kind}" data-i="${i}" data-v="${v}">${l}</button>`).join('')}</div>`;
  const list = ui.rulesTab === 'sites'
    ? `<div class="addline"><input class="field" id="siteInput" placeholder="domain.com  или  .ru" value="${esc(ui.siteInput)}" spellcheck="false" /><button class="sq press" data-act="addSite">${ic('plus', 22)}</button></div>
      <div class="rules">${s.sites.map((r, i) => `<div class="rule"><div class="top"><div class="pat mono">${esc(r.pattern)}</div><button class="xbtn press" data-act="removeRule" data-kind="sites" data-i="${i}">${ic('close', 14)}</button></div>${seg('sites', i, r.action)}</div>`).join('') || '<div class="empty-note">Своих правил для сайтов пока нет.</div>'}</div>`
    : `<button class="addwide press" data-act="pickApps" data-v="rules">${ic('plus', 20)}Выбрать приложение…</button>
      <div class="rules">${s.apps.map((r, i) => `<div class="rule"><div class="top"><div class="tile-ic" style="background:${TINTS[i % TINTS.length]}">${esc((r.name || r.exe)[0].toUpperCase())}</div>
        <div style="flex:1;min-width:0"><div class="t14" style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(r.name)}</div><div class="mono" style="font-size:11px;color:var(--dim2)">${esc(r.exe)}</div></div>
        <button class="xbtn press" data-act="removeRule" data-kind="apps" data-i="${i}">${ic('close', 14)}</button></div>${seg('apps', i, r.action)}</div>`).join('') || '<div class="empty-note">Своих правил для приложений пока нет.</div>'}</div>`;
  return `<div class="h1">Маршрутизация</div>
    <div class="summary"><div class="dot" style="background:${isOn() ? GREEN : DIM}"></div><div class="tx">${summary}</div></div>
    <div class="label">Быстрые исключения</div>
    <div class="card mt">${presets.map(([k, t, d], i) => `${i ? '<div class="divider"></div>' : ''}<div class="row"><div class="grow"><div class="t14">${t}</div><div class="t12">${d}</div></div><div class="toggle${s.presets[k] ? ' on' : ''}" data-act="preset" data-v="${k}"></div></div>`).join('')}</div>
    <div class="seg" style="margin-top:22px"><button class="press${ui.rulesTab === 'sites' ? ' on' : ''}" data-act="rulesTab" data-v="sites">Сайты</button><button class="press${ui.rulesTab === 'apps' ? ' on' : ''}" data-act="rulesTab" data-v="apps">Приложения</button></div>
    <div class="hint">${ui.rulesTab === 'sites' ? 'Домен и все его поддомены. Начните с точки (.ru), чтобы задать правило для всей зоны.' : 'Правило действует на весь трафик программы, независимо от сайтов. Надёжнее всего — в режиме VPN (TUN).'}</div>
    ${list}`;
}

// ─────────── Настройки ───────────

const backBtn = () => `<button class="back press" data-act="back">${ic('chevron', 16)}Настройки</button>`;
const chevRow = (act, title, sub) => `<div class="row click" data-act="${act}"><div class="grow"><div class="t14">${title}</div><div class="t12">${esc(sub)}</div></div>${ic('chevron', 16, 'color:var(--dim2)')}</div>`;

function themeSummary() {
  const t = T.load();
  const name = T.THEMES.find((x) => x[0] === t.theme)[1];
  return name + (t.theme === 'custom' ? ' · ' + T.BASES.find((b) => b[0] === t.base)[1] : t.theme === 'system' ? ' · сейчас ' + (T.systemDark() ? 'тёмная' : 'светлая') : '');
}

function renderSettings() {
  const s = S();
  if (ui.sub === 'mode') return renderMode();
  if (ui.sub === 'kill') return renderKill();
  if (ui.sub === 'theme') return renderTheme();
  if (ui.sub === 'logs') return renderLogs();
  const on = s.ksApps.filter((a) => a.on).length;
  const general = [['autostart', 'Запускать с Windows', 'Свёрнутым в трей и сразу подключаться', ov.autostart], ['autoUpdate', 'Обновлять подписки', 'Каждые 12 часов в фоне', s.autoUpdate], ['notifyDrops', 'Уведомлять об обрывах', 'Всплывающее окно при потере соединения', s.notifyDrops]];
  return `<div class="h1">Настройки</div>
    <div class="label">Подключение</div>
    <div class="card mt">${chevRow('goModeSub', 'Режим подключения', modeLabel())}<div class="divider"></div>${ksIssue
      ? `<div class="row click alert" data-act="goKill"><div class="grow"><div class="t14">Kill Switch</div><div class="t12" style="color:var(--orange)">Не применился — подробности внутри</div></div><span class="adot"></span>${ic('chevron', 16, 'color:var(--dim2)')}</div>`
      : chevRow('goKill', 'Kill Switch', s.killSwitch ? `Вкл · ${on} ${plural(on, 'приложение', 'приложения', 'приложений')}` : 'Выкл')}</div>
    <div class="label">Оформление</div>
    <div class="card mt">${chevRow('goTheme', 'Тема', themeSummary())}</div>
    <div class="label">Общие</div>
    <div class="card mt">${general.map(([k, t, d, v], i) => `${i ? '<div class="divider"></div>' : ''}<div class="row"><div class="grow"><div class="t14">${t}</div><div class="t12">${d}</div></div><div class="toggle${v ? ' on' : ''}" data-act="general" data-v="${k}"></div></div>`).join('')}</div>
    <div class="label">Диагностика</div>
    <div class="card mt">${chevRow('goLogs', 'Логи подключения', `${logs.length} ${plural(logs.length, 'запись', 'записи', 'записей')}`)}<div class="divider"></div>
      <div class="row"><div class="grow"><div class="t14">Ядро Mihomo</div><div class="t12">${esc(ov.mihomoVersion || 'не найдено')} · kl!ck ${esc(ov.appVersion)}</div></div><div class="dot" style="background:${ov.mihomoVersion ? GREEN : RED}"></div></div>
      <div class="divider"></div>${chevRow('openData', 'Папка данных', 'Настройки, подключения, конфиг и лог ядра')}</div>`;
}

function renderMode() {
  const s = S();
  const defs = [
    ['proxy', 'Proxy', 'ручная настройка', `Открывает прокси на 127.0.0.1:${s.proxyPort}. Через VPN пойдут только программы, где вы сами укажете этот адрес. Остальные — напрямую.`, 'Для браузеров с расширением, торрентов, отдельных программ'],
    ['sysproxy', 'Системный proxy', 'большинство программ', 'Windows сообщает адрес прокси всем программам. Браузеры, Telegram, магазины подхватят его сами. Игры и часть приложений его игнорируют.', 'При отключении прежние настройки прокси вернутся'],
    ['tun', 'VPN (TUN)', 'весь трафик', 'Создаёт виртуальный сетевой адаптер — через него идёт трафик всех программ без исключения, включая игры и UDP.', 'Рекомендуется'],
  ];
  const routeDesc = {
    rule: 'Рекомендуется. Трафик идёт через VPN, кроме исключений на экране «Маршрутизация»: .ru-сайты, локальная сеть и выбранные вами сайты и приложения.',
    global: 'Абсолютно всё через VPN — правила и исключения игнорируются. Полезно, если что-то не открывается.',
    direct: 'Ничего не идёт через VPN, но ядро работает и Kill Switch продолжает действовать. Для отладки.',
  };
  return `${backBtn()}
    <div class="h1" style="margin-top:12px">Режим подключения</div>
    <div class="lead">Определяет, какие программы будут ходить через подключение.</div>
    <div style="margin-top:16px;display:flex;flex-direction:column;gap:8px">
      ${defs.map(([k, t, tag, d, h]) => `<div class="modecard${s.mode === k ? ' on' : ''}" data-act="mode" data-v="${k}"><div class="radio${s.mode === k ? ' on' : ''}"><i></i></div>
        <div style="flex:1;min-width:0"><div style="display:flex;align-items:center;gap:8px"><div class="ti">${t}</div><div class="tag">${tag}</div></div><div class="ds">${d}</div><div class="hn">${h}</div></div></div>`).join('')}
    </div>
    ${s.mode !== 'tun' ? `<div class="card mt" style="margin-top:8px"><div class="row"><div class="grow"><div class="t14">Порт прокси</div><div class="t12">SOCKS5 и HTTP на 127.0.0.1</div></div><input class="field" id="portInput" type="number" min="1024" max="65535" value="${s.proxyPort}" style="width:96px;height:36px;text-align:right;background:var(--elem)" /></div></div>` : ''}
    <div class="label" style="margin-top:26px">Куда направлять трафик</div>
    <div class="seg" style="margin-top:10px">${[['rule', 'По правилам'], ['global', 'Глобально'], ['direct', 'Напрямую']].map(([k, l]) => `<button class="press${s.routeMode === k ? ' on' : ''}" data-act="routeMode" data-v="${k}">${l}</button>`).join('')}</div>
    <div class="note" style="margin-top:10px">${routeDesc[s.routeMode]}</div>`;
}

function renderKill() {
  const s = S();
  const on = s.ksApps.filter((a) => a.on).length;
  return `${backBtn()}
    <div class="h1" style="margin-top:12px">Kill Switch</div>
    ${ksIssue ? `<div class="errbox warn"><div class="dot" style="background:var(--orange);margin-top:5px"></div><div style="flex:1;min-width:0">${esc(ksIssue)}</div><button class="retry press" data-act="retryKs">Повторить</button></div>` : ''}
    <div class="card" style="margin-top:16px;padding:14px;display:flex;align-items:center;gap:12px">
      <div style="flex:1;min-width:0"><div style="font-size:15px;font-weight:600">${s.killSwitch ? 'Включён' : 'Выключен'}</div><div style="font-size:12px;color:var(--text3);margin-top:4px;line-height:1.45;text-wrap:pretty">Если VPN выключен или соединение оборвалось, выбранные программы остаются без интернета — их данные не уйдут напрямую через провайдера.</div></div>
      <div class="toggle big${s.killSwitch ? ' on' : ''}" data-act="toggleKill"></div>
    </div>
    <div style="margin-top:22px;display:flex;justify-content:space-between;align-items:baseline"><div class="label" style="margin:0">Защищённые приложения</div><div style="font-size:12px;color:var(--dim)">${on} из ${s.ksApps.length}</div></div>
    <div class="card mt" style="opacity:${s.killSwitch ? 1 : 0.45};transition:opacity .2s">
      ${s.ksApps.map((a, i) => `${i ? '<div class="divider"></div>' : ''}<div class="row"><div class="tile-ic" style="background:${TINTS[(i + 2) % TINTS.length]}">${esc((a.name || a.exe)[0].toUpperCase())}</div>
        <div class="grow"><div class="t14">${esc(a.name)}</div><div class="mono" style="font-size:11px;color:var(--dim2)">${esc(a.exe)}</div></div>
        <button class="xbtn press" data-act="removeKs" data-i="${i}" title="Убрать">${ic('close', 14)}</button>
        <div class="toggle${a.on ? ' on' : ''}" data-act="ksApp" data-i="${i}"></div></div>`).join('')}
      ${s.ksApps.length ? '<div class="divider"></div>' : ''}
      <div class="row click" style="justify-content:center;gap:8px;padding:16px 14px;color:var(--text2);font-size:13px;font-weight:500" data-act="pickApps" data-v="kill">${ic('plus', 18)}Добавить приложение</div>
    </div>
    <div class="hint" style="color:var(--dim2);margin-top:12px">Приложения, не отмеченные здесь, при выключенном VPN работают как обычно — напрямую. Блокировка остаётся и после выхода из kl!ck, пока VPN не включён.</div>`;
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

// ─────────── Шторка: выбор программ ───────────

function renderSheet() {
  const box = $('sheet');
  if (!ui.picker) { box.innerHTML = ''; return; }
  const s = S();
  const taken = new Set((ui.picker === 'rules' ? s.apps : s.ksApps).map((a) => a.exe.toLowerCase()));
  const n = ui.pickerSel.length;
  box.innerHTML = `<div class="sheet"><div class="bg" data-act="closePicker"></div><div class="body">
    <div class="grip"></div>
    <div class="top"><div style="flex:1;min-width:0"><div style="font-size:20px;font-weight:600">Выбрать приложение</div>
      <div style="font-size:12px;color:var(--dim);margin-top:4px;line-height:1.45;text-wrap:pretty">${ui.picker === 'rules' ? 'Отметьте программы — затем для каждой выберите: через VPN, напрямую или блок.' : 'Отмеченные программы останутся без интернета, пока VPN выключен.'}</div></div>
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
    await save({ apps: [...s.apps, ...apps.map((a) => ({ name: a.name, exe: a.exe, action: 'proxy' }))] });
    toast(apps.length === 1 ? 'Приложение добавлено' : 'Добавлено приложений: ' + apps.length, 'По умолчанию — через VPN. Измените в карточке.', GREEN);
  } else {
    await save({ ksApps: [...s.ksApps, ...apps.map((a) => ({ name: a.name, exe: a.exe, path: a.path, on: true }))] });
    toast(apps.length === 1 ? 'Приложение добавлено' : 'Добавлено приложений: ' + apps.length, 'Kill Switch защищает их при выключенном VPN.', GREEN);
  }
  ui.picker = null;
  renderSheet();
  render();
}

// ─────────── Навигация и отрисовка ───────────

const NAV = [['home', 'home', 'Главная'], ['rules', 'rules2', 'Маршрутизация'], ['add', 'plus', 'Добавить'], ['settings', 'settings', 'Настройки']];
function render() {
  if (!ov) return;
  const html = ui.screen === 'home' ? renderHome() : ui.screen === 'add' ? renderAdd() : ui.screen === 'rules' ? renderRules() : renderSettings();
  $('screen').innerHTML = html;
  renderNav();
  if (ui.screen === 'add' && ui.addTab === 'link') updateAddDynamic();
  if (ui.screen === 'home') updateLive();
}
function renderNav() {
  // Точка — когда тост о проблеме уже ушёл, а раздел ещё не открывали.
  const badge = ksIssue && !ksSeen && !toasts.some((t) => t.id === ksToastId);
  $('nav').innerHTML = NAV.map(([k, i, t]) => `<button class="${ui.screen === k ? 'on' : ''}" data-nav="${k}" title="${t}">${ic(i, 24)}${k === 'settings' && badge ? '<i class="badge"></i>' : ''}</button>`).join('');
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
  ui.screen = screen; ui.sub = sub; ui.menuOpen = false; ui.confirmDel = false;
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
  if ((ov.killSwitchIssue || null) !== ksIssue) setKsIssue(ov.killSwitchIssue);
  status = ov.status;
  traffic = ov.traffic || traffic;
  render();
}

async function save(patch) {
  try {
    const r = await kl.updateSettings(patch);
    ov.settings = r.settings;
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
  if (isOn() || isBusy()) {
    await kl.disconnect();
    const n = S().ksApps.filter((a) => a.on).length;
    if (S().killSwitch && n) toast('Kill Switch активен', `${n} ${plural(n, 'приложение', 'приложения', 'приложений')} без интернета до включения VPN.`, ORANGE);
    return;
  }
  ui.powerBusy = true; render();
  try { await kl.connect(); } catch { /* причину показывает карточка ошибки */ }
  ui.powerBusy = false; render();
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
  preset: (el) => { const k = el.dataset.v; save({ presets: { ...S().presets, [k]: !S().presets[k] } }); },
  addSite: () => {
    const v = ui.siteInput.trim();
    if (!v) return $('siteInput')?.focus();
    ui.siteInput = '';
    save({ sites: [{ pattern: v, action: 'direct' }, ...S().sites.filter((r) => r.pattern !== v.toLowerCase())] });
  },
  ruleAction: (el) => {
    const kind = el.dataset.kind, i = +el.dataset.i;
    const list = S()[kind].map((r, j) => (j === i ? { ...r, action: el.dataset.v } : r));
    save({ [kind]: list });
  },
  removeRule: (el) => {
    const kind = el.dataset.kind, i = +el.dataset.i;
    const item = S()[kind][i];
    save({ [kind]: S()[kind].filter((_, j) => j !== i) });
    toast('Правило удалено', item.pattern || item.name, DIM, {
      label: 'Отменить', run: () => { const arr = [...S()[kind]]; arr.splice(Math.min(i, arr.length), 0, item); save({ [kind]: arr }); },
    });
  },
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
  openData: () => kl.openDataDir(),
  general: async (el) => {
    const k = el.dataset.v;
    if (k === 'autostart') {
      try { ov.autostart = await kl.setAutostart(!ov.autostart); } catch (e) { toast('Автозапуск не изменился', errText(e), RED); }
      render();
    } else save({ [k]: !S()[k] });
  },
  mode: (el) => { if (el.dataset.v !== S().mode) save({ mode: el.dataset.v }); },
  routeMode: (el) => { if (el.dataset.v !== S().routeMode) save({ routeMode: el.dataset.v }); },
  toggleKill: () => save({ killSwitch: !S().killSwitch }),
  retryKs: async (el) => {
    el.disabled = true;
    el.textContent = 'Применяю…';
    try { setKsIssue(await kl.retryKillSwitch()); } catch (e) { toast('Не получилось', errText(e), RED); }
    render();
  },
  ksApp: (el) => { const i = +el.dataset.i; save({ ksApps: S().ksApps.map((a, j) => (j === i ? { ...a, on: !a.on } : a)) }); },
  removeKs: (el) => {
    const i = +el.dataset.i, item = S().ksApps[i];
    save({ ksApps: S().ksApps.filter((_, j) => j !== i) });
    toast('Приложение убрано', item.name, DIM, { label: 'Отменить', run: () => { const arr = [...S().ksApps]; arr.splice(Math.min(i, arr.length), 0, item); save({ ksApps: arr }); } });
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
    ui.expanded = false;
    kl.selectProfile(prof.dataset.profile).then(refresh).catch((err) => toast('Не переключилось', errText(err), RED));
    ov.activeProfile = prof.dataset.profile;
    render();
    return;
  }
  const srv = e.target.closest('[data-server]');
  if (srv) {
    const p = active(), name = srv.dataset.server;
    if (name === p.active) return;
    p.active = name;
    render();
    kl.selectServer(p.id, name).catch((err) => toast('Сервер не переключился', errText(err), RED));
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
  else if (id === 'pickerQuery') {
    ui.pickerQuery = e.target.value;
    const s = S();
    $('pickerList').innerHTML = pickerList(new Set((ui.picker === 'rules' ? s.apps : s.ksApps).map((a) => a.exe.toLowerCase())));
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
  if (e.key === 'Enter' && e.target.id === 'linkInput' && (e.ctrlKey || !ui.linkInput.includes('\n'))) { e.preventDefault(); doAddLink(); }
  if (e.key === 'Escape') {
    if (ui.picker) { ui.picker = null; renderSheet(); }
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
  const p = active();
  if (p && status.profileId === p.id) { pings[p.id] = { ...(pings[p.id] || {}), ...m }; if (ui.screen === 'home' && !ui.menuOpen) render(); }
});
kl.on('profiles-changed', refresh);
kl.on('killswitch', (issue) => setKsIssue(issue));

setInterval(() => { if (isOn() && ui.screen === 'home') updateLive(); }, 1000);

(async () => {
  await refresh();
  logs = await kl.getLogs();
  if (ui.screen === 'settings') render();
})();
