// Меню трея. Отдельным файлом: строгая CSP (script-src 'self') не пускает
// инлайновые скрипты, а окно исполняется под администратором.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const ic = (name) => `<span class="ic" style="--i:url(assets/icons/${name}.svg)"></span>`;

let state = null;
let serversOpen = false;

function render() {
  const s = state;
  const on = s.state === 'on';
  const busy = s.state === 'connecting' || s.state === 'reconnecting';
  const meta = [s.server, s.mode].filter(Boolean).map(esc).join(' · ');
  const sw = s.servers.length > 1
    ? `<button class="item ${serversOpen ? 'open' : ''}" data-act="servers">${ic('server')}<span>Сервер</span><span class="ic chev" style="--i:url(assets/icons/chevron.svg)"></span></button>` +
      (serversOpen ? `<div class="sub">${s.servers.map((x) => `<button class="item" data-act="server:${esc(x.name)}"><span>${esc(x.name)}</span>${x.active ? '<span class="mark"></span>' : ''}</button>`).join('')}</div>` : '')
    : '';
  document.getElementById('card').innerHTML = `
    <div class="head ${s.look}">
      <button class="power ${on ? 'on' : busy ? 'busy' : ''}" data-act="toggle" title="${on || busy ? 'Отключить' : 'Подключить'}" ${s.profile ? '' : 'disabled'}>${ic('power')}</button>
      <div class="ht">
        <div class="state"><i></i>${esc(s.label)}</div>
        <div class="name">${esc(s.profile || 'Нет подключений')}</div>
        <div class="meta"><span>${meta}</span>${on && s.ms != null ? `<span class="ms">${s.ms} мс</span>` : ''}</div>
      </div>
    </div>
    <div class="sep"></div>
    <div class="items">
      ${sw}
      <button class="item" data-act="open">${ic('home')}<span>Открыть kl!ck</span></button>
    </div>
    <div class="sep"></div>
    <div class="items"><button class="item danger" data-act="quit">${ic('close')}<span>Выйти</span></button></div>
    <div class="sep"></div>
    <div class="foot"><span>kl!ck ${esc(s.version)}</span><span>mihomo</span></div>`;
}

async function reportSize() {
  await new Promise((r) => requestAnimationFrame(() => r()));
  const r = document.querySelector('.wrap').getBoundingClientRect();
  await invoke('tray_menu_ready', { width: Math.ceil(r.width), height: Math.ceil(r.height) });
}

async function refresh(reset) {
  window.klickTheme?.apply();
  if (reset) serversOpen = false;
  state = await invoke('tray_menu_state');
  render();
  await reportSize();
}

document.getElementById('card').addEventListener('click', async (e) => {
  const btn = e.target.closest('[data-act]');
  if (!btn || btn.disabled) return;
  if (btn.dataset.act === 'servers') {
    serversOpen = !serversOpen;
    render();
    await reportSize();
    return;
  }
  await invoke('tray_menu_action', { id: btn.dataset.act });
});
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') invoke('tray_menu_hide'); });
document.addEventListener('contextmenu', (e) => e.preventDefault());

listen('tray-menu-open', () => refresh(true));
listen('tray-menu-update', () => refresh(false));
refresh(true);
