// Тема kl!ck — как в макете: системная, светлая, тёмная или своя (основа +
// акцент). Общая для окна и меню трея: обе страницы читают один ключ.
(function () {
  const PALETTES = {
    graphite: { page: '#101012', win: '#1a1a1d', card: '#222226', cardHover: '#26262b', hover: '#2a2a2f', elem: '#2e2e33', elemHover: '#36363c', line: '#2c2c31', raised: '#3a3a40', ring: '#4a4a50', text: '#f2f2f4', text2: '#c7c7cc', text3: '#a1a1a8', dim: '#8e8e93', dim2: '#6e6e73', sunken: '#141416', sunken2: '#1e1e22', dots: '#3c3c42', toast: '#2c2c31', onText: '#111111', shadow: 'rgba(0,0,0,.6)' },
    midnight: { page: '#0b0d14', win: '#121521', card: '#1a1e2d', cardHover: '#1e2334', hover: '#222739', elem: '#262c40', elemHover: '#2d3449', line: '#242a3c', raised: '#333a52', ring: '#454d68', text: '#eef0f7', text2: '#c3c8d8', text3: '#9aa1b7', dim: '#8990a8', dim2: '#676e86', sunken: '#0e1019', sunken2: '#161a27', dots: '#30364a', toast: '#242a3c', onText: '#0b0d14', shadow: 'rgba(0,0,0,.6)' },
    oled: { page: '#000000', win: '#000000', card: '#111113', cardHover: '#161618', hover: '#1a1a1d', elem: '#1e1e21', elemHover: '#26262a', line: '#1c1c1f', raised: '#2c2c30', ring: '#3e3e44', text: '#f5f5f7', text2: '#c7c7cc', text3: '#a1a1a8', dim: '#8e8e93', dim2: '#6e6e73', sunken: '#0a0a0b', sunken2: '#0c0c0e', dots: '#2a2a2e', toast: '#1c1c1f', onText: '#000000', shadow: 'rgba(0,0,0,.8)' },
    light: { page: '#e7e7eb', win: '#f5f5f7', card: '#ffffff', cardHover: '#f7f7f9', hover: '#efeff2', elem: '#ededf1', elemHover: '#e3e3e8', line: '#e4e4e9', raised: '#e3e3e8', ring: '#c7c7cc', text: '#1c1c1e', text2: '#3a3a3c', text3: '#5f5f66', dim: '#7c7c83', dim2: '#a0a0a7', sunken: '#ffffff', sunken2: '#fafafb', dots: '#d2d2d8', toast: '#ffffff', onText: '#ffffff', shadow: 'rgba(0,0,0,.14)' },
  };
  const ACCENTS = ['#30d158', '#0a84ff', '#bf5af2', '#ff9f0a', '#64d2ff', '#ff375f'];
  const BASES = [['graphite', 'Графит'], ['midnight', 'Полночь'], ['oled', 'OLED'], ['light', 'Светлая']];
  const THEMES = [['system', 'Системная'], ['light', 'Светлая'], ['dark', 'Тёмная'], ['custom', 'Своя']];
  const KEY = 'klick-theme';
  const DEF = { theme: 'dark', base: 'graphite', accent: '#30d158' };
  const lum = (hex) => {
    const n = parseInt(hex.slice(1), 16);
    return (0.2126 * ((n >> 16) & 255)) / 255 + (0.7152 * ((n >> 8) & 255)) / 255 + (0.0722 * (n & 255)) / 255;
  };
  const mq = matchMedia('(prefers-color-scheme: dark)');

  function load() {
    try { return Object.assign({}, DEF, JSON.parse(localStorage.getItem(KEY)) || {}); } catch { return Object.assign({}, DEF); }
  }
  function resolve(t) {
    let base, accent;
    if (t.theme === 'custom') { base = t.base; accent = t.accent; }
    else {
      const dark = t.theme === 'dark' || (t.theme === 'system' && mq.matches);
      base = dark ? 'graphite' : 'light';
      accent = dark ? '#30d158' : '#1fa34a';
    }
    const light = base === 'light';
    return Object.assign({}, PALETTES[base] || PALETTES.graphite, {
      accent,
      onAccent: light || lum(accent) < 0.45 ? '#ffffff' : '#0b0b0c',
      // Тени плавающих панели вкладок и тостов: на светлом фоне тёмная
      // тень 45 % смотрелась грязным пятном — там она едва заметная.
      navShadow: light ? '0 6px 20px rgba(0,0,0,.07), 0 1px 3px rgba(0,0,0,.05)' : '0 10px 30px rgba(0,0,0,.45)',
      popShadow: light ? '0 8px 24px rgba(0,0,0,.10), 0 1px 3px rgba(0,0,0,.06)' : '0 10px 30px rgba(0,0,0,.5)',
    });
  }
  function apply() {
    const v = resolve(load());
    for (const k in v) document.documentElement.style.setProperty('--' + k, v[k]);
    return v;
  }
  function save(patch) {
    const t = Object.assign(load(), patch);
    try { localStorage.setItem(KEY, JSON.stringify(t)); } catch {}
    apply();
    return t;
  }
  mq.addEventListener('change', apply);
  window.addEventListener('storage', (e) => { if (e.key === KEY) apply(); });
  window.klickTheme = { PALETTES, ACCENTS, BASES, THEMES, load, save, apply, resolve, systemDark: () => mq.matches };
  apply();
})();
