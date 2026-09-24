// Мост окна установщика к Rust (window.ks). Без Tauri — стенд с тестовыми
// данными: ui/index.html#<сценарий>
//   (пусто) — первая установка      #old — стоит старее      #same — та же версия
//   #new — стоит новее              #uninstall — из «Удалить» в параметрах Windows
//   #fail — установка падает        #nospace — мало места
(function () {
  if (window.__TAURI__) {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;
    window.ks = {
      info: () => invoke('info'),
      freeSpace: (path) => invoke('free_space', { path }),
      pickFolder: (current) => invoke('pick_folder', { current }),
      install: (opts) => invoke('install', { opts }),
      cancel: () => invoke('cancel_install'),
      uninstall: (wipe) => invoke('uninstall', { wipe }),
      license: () => invoke('license'),
      minimize: () => invoke('minimize'),
      finish: (launch) => invoke('finish', { launch: launch || null }),
      onProgress: (cb) => listen('progress', (e) => cb(e.payload)),
    };
    return;
  }
  const sc = location.hash.slice(1);
  const GB = 1024 ** 3;
  const inst = { old: '0.2.1', same: '0.2.2', new: '0.3.0', uninstall: '0.2.2' }[sc];
  let listener = () => {};
  let cancelled = false;
  const run = (steps, fail) => new Promise((ok, bad) => {
    let pct = 0, stage = 0;
    cancelled = false;
    const t = setInterval(() => {
      if (cancelled) { clearInterval(t); setTimeout(() => bad('cancelled'), 600); return; }
      pct = Math.min(100, pct + 1.4 + Math.random() * 1.6);
      stage = Math.min(steps - 1, Math.floor((pct / 100) * steps));
      listener({ pct, stage });
      if (fail && pct > 55) { clearInterval(t); bad('Установщик kl!ck завершился с кодом 2.'); }
      if (pct >= 100) { clearInterval(t); setTimeout(ok, 300); }
    }, 60);
  });
  window.ks = {
    info: async () => ({
      version: '0.2.2',
      installed: inst ? { version: inst, path: 'C:\\Program Files\\kl!ck' } : null,
      compare: inst ? (inst < '0.2.2' ? -1 : inst === '0.2.2' ? 0 : 1) : null,
      defaultPath: 'C:\\Program Files\\kl!ck',
      required: 48.6 * 1024 * 1024,
      free: sc === 'nospace' ? 20 * 1024 * 1024 : 212 * GB,
      uninstall: sc === 'uninstall',
    }),
    freeSpace: async (p) => (p.startsWith('Z:') ? null : 180 * GB),
    pickFolder: async () => 'D:\\Games\\kl!ck',
    install: () => run(4, sc === 'fail'),
    cancel: async () => { cancelled = true; },
    uninstall: () => run(4, false),
    license: async () => 'MIT License\n\nCopyright (c) 2026 vbu00 (kl!ck)\n\nPermission is hereby granted, free of charge, to any person obtaining a copy…',
    minimize: async () => {},
    finish: async (launch) => { document.title = launch ? 'запуск: ' + launch : 'закрыто'; },
    onProgress: (cb) => { listener = cb; },
  };
})();
