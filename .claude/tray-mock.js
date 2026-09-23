// Стаб window.__TAURI__ для меню трея на стенде: /src/tray?state=on|off|connecting|bad&theme=…
(function () {
  const q = new URLSearchParams(location.search);
  const st = q.get('state') || 'on';
  if (q.get('theme')) {
    try { localStorage.setItem('klick-theme', JSON.stringify({ theme: q.get('theme'), base: 'graphite', accent: '#30d158' })); } catch {}
  }
  const servers = ['Нидерланды · Amsterdam', 'Германия · Frankfurt', 'Финляндия · Helsinki', 'США · New York'];
  const base = {
    on: { look: 'ok', label: 'Подключено', state: 'on', ms: 48 },
    off: { look: 'idle', label: 'Не подключено', state: 'off', ms: null },
    connecting: { look: 'warn', label: 'Подключаюсь…', state: 'connecting', ms: null },
    bad: { look: 'bad', label: 'Сервер не отвечает', state: 'on', ms: null },
  }[st];
  let active = servers[0];
  const now = Math.floor(Date.now() / 1000);
  const state = () => ({
    ...base, profile: 'Remnawave · Alex', server: active, mode: 'VPN (TUN)', routeMode: 'по правилам',
    since: st === 'on' || st === 'bad' ? now - 2 * 3600 - 17 * 60 : null,
    traffic: { down: st === 'on' ? 3.4e6 : 0, up: st === 'on' ? 0.41e6 : 0, downTotal: 1.9e9, upTotal: 0.21e9 },
    killSwitch: true, ksApps: 3, ksSites: 2,
    profiles: [{ id: 'p1', name: 'Remnawave · Alex', active: true, live: st !== 'off' }, { id: 'p2', name: 'grpc', active: false, live: false }],
    servers: servers.map((n, i) => ({ name: n, active: n === active, ms: [48, 61, null, 142][i] })),
    version: '0.2.0', mihomo: 'v1.19.31',
  });
  window.__TAURI__ = {
    core: {
      invoke: async (cmd, args) => {
        if (cmd === 'tray_menu_state') return state();
        if (cmd === 'tray_menu_action' && args.id.startsWith('server:')) active = args.id.slice(7);
        console.log('invoke', cmd, JSON.stringify(args || {}));
        return null;
      },
    },
    event: {
      listen: async (ev, cb) => {
        // Живая скорость на стенде.
        if (ev === 'traffic' && (st === 'on')) {
          let t = 0;
          setInterval(() => { t++; const d = (40 + 30 * Math.sin(t / 3) + 20 * Math.random()) * 125000; cb({ payload: { down: d, up: d / 7, downTotal: 0, upTotal: 0 } }); }, 400);
        }
        return () => {};
      },
    },
  };
  document.documentElement.style.background = q.get('bg') || '#202020';
})();
