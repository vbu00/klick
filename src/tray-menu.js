// Меню трея. Отдельным файлом: строгая CSP (script-src 'self') не пускает
// инлайновые скрипты, а окно исполняется под администратором.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const ic = (name, extra = '') => `<span class="ic" style="--i:url(assets/icons/${name}.svg)${extra}"></span>`;
const plural = (n, a, b, c) => { const m10 = n % 10, m100 = n % 100; return m10 === 1 && m100 !== 11 ? a : m10 >= 2 && m10 <= 4 && (m100 < 10 || m100 >= 20) ? b : c; };
const pingColor = (ms) => (ms == null ? 'var(--red)' : ms < 80 ? 'var(--accent)' : ms < 160 ? 'var(--text)' : 'var(--orange)');
const fmtRate = (bytes) => { const mb = (bytes * 8) / 1e6; return mb >= 1000 ? [(mb / 1000).toFixed(2), 'Gb/s'] : [mb.toFixed(mb >= 100 ? 0 : mb >= 10 ? 1 : 2), 'Mb/s']; };
// Щит — значок Kill Switch; в наборе иконок макета такого нет, рисуем сами.
const SHIELD = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l7 3v5.5c0 4.3-2.9 8-7 9.5-4.1-1.5-7-5.2-7-9.5V6z"/><path d="M9 12l2 2 4-4"/></svg>';

let state = null;
let pinging = false;
// Скорость за последние 40 секунд — для графика. Окно меню живёт всё время
// (только прячется), поэтому история копится и пока меню закрыто.
const hist = { dl: new Array(40).fill(0), ul: new Array(40).fill(0) };
let rate = { down: 0, up: 0 };

const isOn = () => state?.state === 'on';
const isBusy = () => state?.state === 'connecting' || state?.state === 'reconnecting';

function timerText() {
  const h = isOn() && state.since ? Math.max(0, Math.floor(Date.now() / 1000 - state.since)) : 0;
  return `${Math.floor(h / 3600)}:${String(Math.floor(h / 60) % 60).padStart(2, '0')}:${String(h % 60).padStart(2, '0')}`;
}

function sparkPaths() {
  const max = Math.max(1, ...hist.dl, ...hist.ul) * 1.15;
  const pts = (arr) => arr.map((v, i) => `${(i / (arr.length - 1)) * 100} ${30 - (v / max) * 26}`);
  const d = 'M' + pts(hist.dl).join(' L');
  return { a: d + ' L100 30 L0 30 Z', l: d, u: 'M' + pts(hist.ul).join(' L') };
}

function updateLive() {
  if ($('timer')) $('timer').textContent = timerText();
  const [dv, du] = fmtRate(rate.down), [uv, uu] = fmtRate(rate.up);
  if ($('dl')) $('dl').innerHTML = `${dv}<small>${du}</small>`;
  if ($('ul')) $('ul').innerHTML = `${uv}<small>${uu}</small>`;
  const sp = $('spark');
  if (sp) {
    const p = sparkPaths();
    sp.querySelector('.a').setAttribute('d', p.a);
    sp.querySelector('.l').setAttribute('d', p.l);
    sp.querySelector('.u').setAttribute('d', p.u);
  }
}

function render() {
  const s = state;
  const on = isOn(), busy = isBusy();
  const has = !!s.profile;
  const bad = s.look === 'bad', warn = s.look === 'warn';
  const powerCls = warn ? 'warn' : on ? 'on' : busy ? 'busy' : bad ? 'bad' : '';
  const ping = on && s.ms != null ? `<div class="pingpill" style="color:${pingColor(s.ms)}"><i></i>${s.ms} мс</div>` : '';
  const big = on ? `<div class="big" id="timer">${timerText()}</div>` : `<div class="big name">${esc(s.profile || 'Нет подключений')}</div>`;
  // Название подключения видно на чипах, если их несколько.
  const meta = on || busy ? (s.profiles.length > 1 ? esc(s.server || '') : `${esc(s.profile)} · ${esc(s.server || '')}`) : has ? `${esc(s.mode)} · ${esc(s.routeMode)}` : 'Добавьте подписку в окне kl!ck';

  const speed = on ? `<div class="speed">
      <svg class="spark" id="spark" viewBox="0 0 100 30" preserveAspectRatio="none"><path class="a"/><path class="u"/><path class="l"/></svg>
      <div class="sp"><div class="k">${ic('cloud-down')}Чтение</div><div class="v" id="dl"></div></div>
      <div class="sp"><div class="k">${ic('cloud-up')}Загрузка</div><div class="v" id="ul"></div></div>
    </div>` : '';

  const profiles = s.profiles.length > 1 ? `<div class="sec"><div class="lbl">Подключение</div><div class="chips">${s.profiles.map((p) =>
    `<button class="chip${p.active ? ' on' : ''}" data-act="profile:${esc(p.id)}">${p.live ? '<i class="live"></i>' : ''}${esc(p.name)}</button>`).join('')}</div></div>` : '';

  const servers = s.servers.length ? `<div class="sec">
      <div class="lbl"><span>${s.servers.length > 1 ? 'Сервер · ' + s.servers.length : 'Сервер'}</span>
        <button class="mini${pinging ? ' spin' : ''}" data-act="ping">${ic('refresh')}${pinging ? 'Проверяем…' : 'Задержка'}</button></div>
      <div class="list">${s.servers.map((x) => {
        const ms = pinging ? '…' : x.ms === undefined ? '' : x.ms === null ? 'нет ответа' : x.ms + ' мс';
        const color = pinging || x.ms === undefined ? 'var(--dim)' : pingColor(x.ms);
        return `<button class="srv${x.active ? ' on' : ''}" data-act="server:${esc(x.name)}"><span class="r"></span><span class="n">${esc(x.name)}</span><span class="ms" style="color:${color}">${ms}</span></button>`;
      }).join('')}</div></div>` : '';

  const ksDesc = s.killSwitch
    ? [s.ksApps ? `${s.ksApps} ${plural(s.ksApps, 'приложение', 'приложения', 'приложений')}` : '', s.ksSites ? `${s.ksSites} ${plural(s.ksSites, 'сайт', 'сайта', 'сайтов')}` : ''].filter(Boolean).join(', ') || 'Список пуст'
    : 'Выключен';
  const ks = `<div class="ks${s.killSwitch ? ' on' : ''}" data-act="killswitch"><div class="ki">${SHIELD}</div>
      <div><div class="t">Kill Switch</div><div class="d">${esc(ksDesc)}</div></div><div class="tg"></div></div>`;

  $('card').innerHTML = `
    <div class="hero ${s.look}">
      <div class="dots"></div><div class="glow"></div>
      <div class="top"><div class="brand"><span class="key"><span class="ic" style="--i:url(assets/mark-face.png)"></span><span class="ic" style="--i:url(assets/mark.png)"></span></span><span>kl<b>!</b>ck</span></div>${ping}</div>
      <div class="main">
        <button class="power ${powerCls}" data-act="toggle" title="${on || busy ? 'Отключить' : 'Подключить'}" ${has ? '' : 'disabled'}>${ic('power')}</button>
        <div class="ht"><div class="state"><i></i>${esc(s.label)}</div>${big}<div class="meta">${meta}</div></div>
      </div>
    </div>
    ${speed}${profiles}${servers}${ks}
    <div class="foot">
      <button class="btn open" data-act="open">${ic('home')}Открыть kl!ck</button>
      <button class="btn quit" data-act="quit" title="Выйти из kl!ck">${ic('close')}</button>
    </div>
    <div class="ver"><span>kl!ck ${esc(s.version)}</span><span>${s.mihomo ? 'mihomo ' + esc(s.mihomo) : ''}</span></div>`;
  updateLive();
}

async function reportSize() {
  await new Promise((r) => requestAnimationFrame(() => r()));
  const r = document.querySelector('.wrap').getBoundingClientRect();
  await invoke('tray_menu_ready', { width: Math.ceil(r.width), height: Math.ceil(r.height) });
}

async function refresh() {
  window.klickTheme?.apply();
  state = await invoke('tray_menu_state');
  pinging = false;
  if (!isOn()) { hist.dl.fill(0); hist.ul.fill(0); rate = { down: 0, up: 0 }; }
  else if (!rate.down && !rate.up) rate = { down: state.traffic.down, up: state.traffic.up };
  render();
  await reportSize();
}

$('card').addEventListener('click', async (e) => {
  const btn = e.target.closest('[data-act]');
  if (!btn || btn.disabled) return;
  const act = btn.dataset.act;
  if (act === 'ping') {
    if (pinging) return;
    pinging = true;
    render();
  }
  await invoke('tray_menu_action', { id: act });
});
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') invoke('tray_menu_hide'); });
document.addEventListener('contextmenu', (e) => e.preventDefault());

listen('traffic', (e) => {
  rate = e.payload;
  hist.dl.push(rate.down); hist.dl.shift();
  hist.ul.push(rate.up); hist.ul.shift();
  if (state) updateLive();
});
setInterval(() => { if (isOn()) updateLive(); }, 1000);

listen('tray-menu-open', () => refresh());
listen('tray-menu-update', () => refresh());
refresh();
