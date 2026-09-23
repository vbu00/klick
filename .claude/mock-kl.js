// Стаб window.kl для стенда вёрстки: данные как в макете, без Tauri.
(function () {
  const L = {};
  const emit = (ev, p) => (L[ev] || []).forEach((cb) => cb(p));
  const now = () => Math.floor(Date.now() / 1000);
  const clock = () => new Date().toTimeString().slice(0, 8);
  const GB = 1024 ** 3;
  const profiles = [
    { id: 'p1', kind: 'sub', name: 'Remnawave · Alex', hasUrl: true, active: 'Нидерланды · Amsterdam', updatedAt: now() - 3 * 3600,
      info: { upload: 2.1 * GB, download: 40.5 * GB, total: 200 * GB, expire: now() + 52 * 86400 },
      servers: [
        { name: 'Нидерланды · Amsterdam', proto: 'VLESS · Reality', host: '203.0.113.10:443' },
        { name: 'Германия · Frankfurt', proto: 'VLESS · Reality', host: '203.0.113.11:443' },
        { name: 'Финляндия · Helsinki', proto: 'Hysteria2', host: '203.0.113.12:8443' },
        { name: 'США · New York', proto: 'Trojan · gRPC', host: '203.0.113.13:443' },
      ] },
    { id: 'p2', kind: 'single', name: 'grpc', hasUrl: true, active: 'grpc', updatedAt: now(), info: null, servers: [{ name: 'grpc', proto: 'VLESS · gRPC', host: '198.51.100.81:443' }] },
  ];
  const settings = {
    activeProfile: 'p1', mode: 'tun', routeMode: 'rule', proxyPort: 7890, presets: { ru: true, lan: true, geoip: false },
    sites: [{ pattern: 'gosuslugi.ru', action: 'direct' }, { pattern: 'youtube.com', action: 'proxy' }, { pattern: 'ads.example.net', action: 'block' }],
    apps: [{ name: 'Telegram', exe: 'Telegram.exe', action: 'proxy' }, { name: 'Steam', exe: 'steam.exe', action: 'direct' }, { name: 'Discord', exe: 'Discord.exe', action: 'proxy' }],
    killSwitch: true,
    ksSites: [{ pattern: 'sberbank.ru', on: true }, { pattern: 'mail.google.com', on: true }, { pattern: 'github.com', on: false }],
    ksApps: [{ name: 'qBittorrent', exe: 'qbittorrent.exe', path: 'C:\\x', on: true }, { name: 'Telegram', exe: 'Telegram.exe', path: 'C:\\x', on: true }, { name: 'Discord', exe: 'Discord.exe', path: 'C:\\x', on: true }, { name: 'Firefox', exe: 'firefox.exe', path: 'C:\\x', on: false }],
    autoUpdate: true, notifyDrops: true, connectOnLaunch: true,
  };
  // ?promo=1 — данные для ролика (promo/): геймер, CS2 и Discord.
  //   &apps=after — правило для CS2 уже добавлено.
  const PQ = new URLSearchParams(location.search);
  if (PQ.get('promo')) {
    settings.apps = PQ.get('apps') === 'after'
      ? [{ name: 'Counter-Strike 2', exe: 'cs2.exe', action: 'direct' }, { name: 'Discord', exe: 'Discord.exe', action: 'proxy' }]
      : [{ name: 'Discord', exe: 'Discord.exe', action: 'proxy' }];
    settings.sites = [];
  }
  // &mode=proxy|sysproxy|tun — режим подключения на стенде.
  if (PQ.get('mode')) settings.mode = PQ.get('mode');
  let status = { state: 'off', error: null, warning: null, profileId: 'p1', server: 'Нидерланды · Amsterdam', mode: 'tun', since: null, health: null };
  let autostart = true;
  let logs = [
    { time: '12:04:11', level: 'INFO', text: 'Start initial configuration in progress' },
    { time: '12:04:11', level: 'INFO', text: 'Mixed(http+socks) proxy listening at: 127.0.0.1:7890' },
    { time: '12:04:12', level: 'WARN', text: '[TCP] dial PROXY (match GeoIP/ru) 127.0.0.1:52011 --> api.example.net:443 error: i/o timeout' },
  ];
  let timer = null, tot = { up: 0, down: 0 }, t = 0;
  const set = (p) => { status = { ...status, ...p }; emit('core-status', status); };
  const log = (level, text) => { const e = { time: clock(), level, text }; logs.push(e); emit('logs', [e]); };
  const act = () => profiles.find((p) => p.id === settings.activeProfile) || profiles[0];
  const clone = (x) => JSON.parse(JSON.stringify(x));
  window.__mock = { scenario: location.hash.slice(1) };

  window.kl = {
    overview: async () => ({ status, traffic: { up: 0, down: 0, upTotal: tot.up, downTotal: tot.down }, settings: clone(settings), profiles: window.__mock.scenario === 'empty' ? [] : clone(profiles), activeProfile: settings.activeProfile, autostart, appVersion: '0.2.0', mihomoVersion: 'v1.19.31', killSwitchIssue: window.__mock.ksIssue || null }),
    connect: async () => {
      const p = act();
      set({ state: 'connecting', error: null, profileId: p.id, server: p.active, mode: settings.mode });
      log('INFO', `Подключение → ${p.active} [${settings.mode}/${settings.routeMode}]`);
      await new Promise((r) => setTimeout(r, 1000));
      if (window.__mock.scenario === 'fail') { set({ state: 'error', error: 'Не удалось создать VPN-адаптер: нет прав администратора. Запустите kl!ck от администратора или выберите режим Proxy.' }); throw 'fail'; }
      set({ state: 'on', since: now(), health: null });
      log('INFO', 'Туннель поднят');
      setTimeout(() => { set({ health: { ok: window.__mock.scenario !== 'dead', ms: window.__mock.scenario === 'dead' ? null : 48, at: now() } }); if (window.__mock.scenario !== 'dead') emit('pings', { [p.active]: 48 }); }, 700);
      tot = { up: 0, down: 0 };
      timer = setInterval(() => {
        t++;
        const base = 60 + 70 * Math.sin(t / 7) + 40 * Math.sin(t / 2.3);
        const dl = Math.max(0.2, base + (Math.random() - 0.5) * 40 + (t % 23 === 0 ? 180 : 0)) * 125000;
        const ul = Math.max(0.1, 14 + 12 * Math.sin(t / 5 + 1)) * 125000;
        tot.down += dl; tot.up += ul;
        emit('traffic', { up: ul, down: dl, upTotal: tot.up, downTotal: tot.down });
      }, 1000);
    },
    disconnect: async () => { clearInterval(timer); set({ state: 'off', since: null, health: null }); log('INFO', 'Отключено, VPN-адаптер удалён'); },
    selectProfile: async (id) => {
      const live = status.state === 'on' && status.profileId !== id;
      settings.activeProfile = id;
      if (live) { await window.kl.disconnect(); await window.kl.connect(); }
      else set({ profileId: id, server: act().active });
    },
    selectServer: async (pid, name) => { profiles.find((p) => p.id === pid).active = name; if (status.state === 'on') set({ server: name }); },
    ping: async (pid) => {
      await new Promise((r) => setTimeout(r, 1200));
      const p = profiles.find((x) => x.id === pid);
      const fixed = [48, 61, 39, 142];
      return Object.fromEntries(p.servers.map((s, i) => [s.name, location.search ? fixed[i % 4] : Math.random() < 0.18 ? null : Math.round(30 + Math.random() * 160)]));
    },
    addLink: async (input, name) => {
      await new Promise((r) => setTimeout(r, 800));
      const sub = /^https?:/.test(input.trim());
      const id = 'n' + Date.now();
      profiles.push(sub
        ? { id, kind: 'sub', name: name || 'Подписка panel.example.com', hasUrl: true, active: 'Турция · Istanbul', updatedAt: now(), info: { upload: 0, download: 0, total: 100 * GB, expire: now() + 99 * 86400 }, servers: [{ name: 'Турция · Istanbul', proto: 'VLESS · WS', host: '203.0.113.40:443' }, { name: 'Польша · Warsaw', proto: 'VLESS · Reality', host: '203.0.113.41:443' }] }
        : { id, kind: 'single', name: (input.split('#')[1] || 'сервер').slice(0, 24), hasUrl: true, active: 'сервер', updatedAt: now(), info: null, servers: [{ name: 'сервер', proto: 'VLESS · Reality', host: '203.0.113.9:443' }] });
      settings.activeProfile = id;
      return { id, kind: sub ? 'sub' : 'single', servers: sub ? 2 : 1 };
    },
    addFile: async (path) => { const id = 'f' + Date.now(); profiles.push({ id, kind: 'file', name: path.split(/[\\/]/).pop(), hasUrl: false, active: 'A', updatedAt: now(), info: null, servers: [{ name: 'A', proto: 'Shadowsocks', host: '203.0.113.50:8388' }] }); settings.activeProfile = id; return { id, kind: 'file', servers: 1 }; },
    refreshSub: async () => { await new Promise((r) => setTimeout(r, 1200)); if (Math.random() < 0.3) throw 'Сервер подписки ответил 502. Проверьте ссылку или повторите позже.'; profiles[0].info.download += 1.3 * GB; profiles[0].updatedAt = now(); return 4; },
    removeProfile: async (id) => { const i = profiles.findIndex((p) => p.id === id); const [p] = profiles.splice(i, 1); settings.activeProfile = profiles[0]?.id || null; return { profile: p, index: i, was_live: false }; },
    restoreProfile: async (p, i) => { profiles.splice(i, 0, p); settings.activeProfile = p.id; },
    profileLink: async () => 'https://panel.example.com/sub/a1b2c3',
    updateSettings: async (patch) => {
      Object.assign(settings, clone(patch));
      if ('killSwitch' in patch && window.__mock.scenario === 'ksfail') setTimeout(() => { window.__mock.ksIssue = 'Правило брандмауэра не записалось: доступ запрещён.'; emit('killswitch', window.__mock.ksIssue); }, 300);
      return { settings: clone(settings), warning: null };
    },
    setAutostart: async (v) => (autostart = v),
    retryKillSwitch: async () => { await new Promise((r) => setTimeout(r, 600)); window.__mock.ksIssue = null; return null; },
    // Иконки в стенде — цветной квадрат с буквой: в сеть стенд не ходит.
    favicon: async (host) => {
      await new Promise((r) => setTimeout(r, 200));
      if (host.startsWith('ads.')) return null;
      const c = ['#4285f4', '#ea4335', '#34a853', '#ff9f0a', '#bf5af2'][host.length % 5];
      return 'data:image/svg+xml;base64,' + btoa(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16" rx="4" fill="${c}"/><text x="8" y="12" font-size="10" font-family="Arial" font-weight="700" fill="#fff" text-anchor="middle">${host[0].toUpperCase()}</text></svg>`);
    },
    appInfo: async () => ({ version: '0.2.0', build: '2026.09.23', mihomo: 'v1.19.31', system: 'Windows 11 · x64', dataDir: '%LOCALAPPDATA%\\com.vbu00.klick', geo: { updated: now() - 3 * 86400, size: 7780891, fresh: false }, repo: 'https://github.com/vbu00/klick' }),
    updateGeo: async () => { await new Promise((r) => setTimeout(r, 1500)); return { updated: now(), size: 7780891, fresh: true }; },
    checkUpdate: async () => { await new Promise((r) => setTimeout(r, 900)); return { current: '0.2.0', latest: '0.2.0', newer: false, url: 'https://github.com/vbu00/klick/releases/tag/v0.2.0' }; },
    openUrl: async () => {},
    licenses: async () => 'MIT License\n\nCopyright (c) 2026 vbu00 (kl!ck)\n\nPermission is hereby granted, free of charge, …\n\n────────\n\n# Сторонние компоненты kl!ck\n\n## mihomo — ядро\n- Лицензия: MIT',
    runningApps: async () => {
      await new Promise((r) => setTimeout(r, 300));
      const on = status.state === 'on';
      if (PQ.get('promo')) {
        return [['Counter-Strike 2', 'cs2.exe', 18240, 12.6], ['Steam', 'steam.exe', 5316, 1.9], ['Google Chrome', 'chrome.exe', 11284, 1.2], ['Discord', 'Discord.exe', 7732, 0.4], ['OBS Studio', 'obs64.exe', 9904, 0.2], ['Spotify', 'Spotify.exe', 14020, 0.1], ['Telegram', 'Telegram.exe', 9120, 0.05], ['FACEIT', 'FACEIT.exe', 6620, 0]]
          .map(([name, exe, pid, mbs]) => ({ name, exe, pid, path: 'C:\\Games\\' + exe, activity: on ? mbs * 1024 * 1024 : null }));
      }
      return [['Google Chrome', 'chrome.exe', 11284, 3.4], ['Telegram', 'Telegram.exe', 9120, 0.8], ['Discord', 'Discord.exe', 7732, 0.4], ['Steam', 'steam.exe', 5316, 1.9], ['Spotify', 'Spotify.exe', 14020, 0.3], ['qBittorrent', 'qbittorrent.exe', 6604, 12.6], ['Firefox', 'firefox.exe', 15872, 0.6], ['Visual Studio Code', 'Code.exe', 4480, 0.1], ['Zoom', 'Zoom.exe', 12760, 0]]
        .map(([name, exe, pid, mbs]) => ({ name, exe, pid, path: 'C:\\Program Files\\' + exe, activity: on ? mbs * 1024 * 1024 : null }))
        .sort((a, b) => (b.activity || 0) - (a.activity || 0));
    },
    appFromFile: async (p) => ({ name: 'Obsidian', exe: 'Obsidian.exe', pid: 0, path: p }),
    getLogs: async () => logs.slice(),
    clearLogs: async () => { logs = []; },
    restoreLogs: async (l) => { logs = [...l, ...logs]; },
    copyText: async () => {},
    openDataDir: async () => {},
    minimize: async () => {},
    close: async () => {},
    pickConfig: async () => 'C:\\Users\\user\\Downloads\\config.yaml',
    pickExe: async () => 'C:\\Program Files\\Obsidian\\Obsidian.exe',
    onFileDrop: () => {},
    on: (ev, cb) => { (L[ev] ||= []).push(cb); },
  };

  // Съёмка скриншотов для README (tools/screenshots.ps1): параметры в адресе.
  //   ?theme=light|dark|system | custom:<основа>:<акцент>  &state=on  &screen=rules|add|settings|kill|theme|mode|about  &expand=1  &ks=sites  &info=ksInfo
  //   ролик: &promo=1 [&apps=after] &rtab=apps &picker=1 &pick=cs2.exe &scroll=end|px
  //   пролёт: &mode=sysproxy  &link=<текст в поле «Добавить»>
  const q = new URLSearchParams(location.search);
  if (q.get('frame')) {
    // Окно 380×720 посреди страницы — как в макете; безголовый браузер уже
    // ~500 px окно не делает.
    const st = document.createElement('style');
    st.textContent = 'html,body{height:auto;overflow:visible;background:var(--page)}body{min-height:100vh;display:flex;align-items:center;justify-content:center}.window{flex:none;width:380px;height:720px;border:1px solid var(--line);border-radius:14px;box-shadow:0 30px 80px var(--shadow)}';
    document.head.appendChild(st);
  }
  if (q.get('theme')) {
    const [theme, base, accent] = q.get('theme').split(':');
    window.klickTheme.save(theme === 'custom' ? { theme, base, accent: '#' + accent } : { theme });
  }
  const ready = () => new Promise((r) => { const t = setInterval(() => { if (document.querySelector('[data-nav]')) { clearInterval(t); r(); } }, 20); });
  const pause = (ms) => new Promise((r) => setTimeout(r, ms));
  if (location.search) window.addEventListener('load', async () => {
    await ready();
    const click = async (sel) => { document.querySelector(sel)?.click(); await pause(120); };
    if (q.get('state') === 'on') { await click('[data-act=power]'); await pause(20000); }
    if (q.get('expand')) await click('[data-act=toggleExpand]');
    if (q.get('ping')) { await click('[data-act=ping]'); await pause(2000); }
    const scr = q.get('screen');
    if (scr === 'rules' || scr === 'add' || scr === 'settings') await click(`[data-nav=${scr}]`);
    if (['kill', 'theme', 'mode', 'about'].includes(scr)) { await click('[data-nav=settings]'); await click({ kill: '[data-act=goKill]', theme: '[data-act=goTheme]', mode: '[data-act=goModeSub]', about: '[data-act=goAbout]' }[scr]); }
    if (q.get('ks')) { await click('[data-act=ksTab][data-v=sites]'); await pause(800); }
    if (q.get('rtab')) await click(`[data-act=rulesTab][data-v=${q.get('rtab')}]`);
    if (q.get('link')) {
      const ta = document.getElementById('linkInput');
      ta.value = q.get('link');
      ta.dispatchEvent(new Event('input', { bubbles: true }));
      await pause(200);
    }
    if (q.get('picker')) { await click('[data-act=pickApps]'); await pause(800); }
    if (q.get('pick')) await click(`[data-pick="${q.get('pick')}"]`);
    if (q.get('scroll')) { const sc = document.getElementById('scroll'); sc.scrollTop = q.get('scroll') === 'end' ? sc.scrollHeight : +q.get('scroll'); await pause(400); }
    if (q.get('info')) await click(`[data-act=${q.get('info')}]`);
  });
})();
