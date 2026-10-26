// Круглые флаги стран: упрощённые SVG 30×30 (без рамки — круг даёт тот, кто рисует). Отдельно от
// компонента: их же берёт сборка iOS-беты (ui/scripts/export-flags.mjs) — модуль без зависимостей.

/** Полосы: каждая следующая закрашивает от своего края до конца — без щелей между полосами. */
function bands(dir: 'h' | 'v', colors: string[], weights: number[] = colors.map(() => 1)): string {
  const total = weights.reduce((a, b) => a + b, 0);
  let at = 0;
  return colors
    .map((c, i) => {
      const o = +at.toFixed(2);
      at += (30 * weights[i]) / total;
      return dir === 'h' ? `<rect y="${o}" width="30" height="${30 - o}" fill="${c}"/>` : `<rect x="${o}" width="${30 - o}" height="30" fill="${c}"/>`;
    })
    .join('');
}
const h = (...c: string[]) => bands('h', c);
const v = (...c: string[]) => bands('v', c);
const bg = (c: string) => `<rect width="30" height="30" fill="${c}"/>`;

/** Скандинавский крест, сдвинутый к древку. */
const nordic = (back: string, cross: string, inner?: string) =>
  bg(back) +
  `<path d="M8 0h6v12h16v6H14v12H8V18H0v-6h8z" fill="${cross}"/>` +
  (inner ? `<path d="M9.5 0h3v13.5H30v3H12.5V30h-3V16.5H0v-3h9.5z" fill="${inner}"/>` : '');

function star(cx: number, cy: number, r: number, fill: string): string {
  const pts = Array.from({ length: 10 }, (_, i) => {
    const a = -Math.PI / 2 + (i * Math.PI) / 5;
    const d = i % 2 ? r * 0.4 : r;
    return `${(cx + d * Math.cos(a)).toFixed(1)},${(cy + d * Math.sin(a)).toFixed(1)}`;
  });
  return `<polygon points="${pts.join(' ')}" fill="${fill}"/>`;
}

/** Солнце с лучами: Казахстан, Киргизия. */
const sun = (x: number, y: number, r: number, c: string) =>
  `<circle cx="${x}" cy="${y}" r="${r}" fill="${c}"/><circle cx="${x}" cy="${y}" r="${r + 2.2}" fill="none" stroke="${c}" stroke-width="1.8" stroke-dasharray="1.3 1.4"/>`;

export const FLAGS: Record<string, string> = {
  RU: h('#fff', '#0039a6', '#d52b1e'),
  BY:
    bands('h', ['#c8313e', '#4aa657'], [2, 1]) +
    '<rect width="8" height="30" fill="#fff"/><path d="M4.5 3.5l2.4 3.2-2.4 3.2-2.4-3.2zM4.5 11.8l2.4 3.2-2.4 3.2-2.4-3.2zM4.5 20.1l2.4 3.2-2.4 3.2-2.4-3.2z" fill="#c8313e"/>',
  UA: h('#0057b7', '#ffd700'),
  KZ: bg('#00afca') + sun(16, 12.5, 4, '#fec50c') + '<path d="M8.5 19.5q7.5 4.2 15 0-7.5 2.2-15 0z" fill="#fec50c"/><rect x="3" width="1.6" height="30" fill="#fec50c"/>',
  KG: bg('#e8112d') + sun(15, 15, 5, '#ffef00'),
  UZ: h('#0099b5', '#fff', '#1eb53a') + '<rect y="9.6" width="30" height=".8" fill="#ce1126"/><rect y="19.6" width="30" height=".8" fill="#ce1126"/>',
  AM: h('#d90012', '#0033a0', '#f2a800'),
  AZ: h('#00b5e2', '#ef3340', '#509e2f') + '<circle cx="14" cy="15" r="3.4" fill="#fff"/><circle cx="15" cy="15" r="2.8" fill="#ef3340"/>' + star(18.6, 15, 1.6, '#fff'),
  GE:
    bg('#fff') +
    '<path d="M12.5 0h5v12.5H30v5H17.5V30h-5V17.5H0v-5h12.5z" fill="#ff0000"/><path d="M5.5 5h2v1.5H9v2H7.5V10h-2V8.5H4v-2h1.5zM22.5 5h2v1.5H26v2h-1.5V10h-2V8.5H21v-2h1.5zM5.5 20h2v1.5H9v2H7.5V25h-2v-1.5H4v-2h1.5zM22.5 20h2v1.5H26v2h-1.5V25h-2v-1.5H21v-2h1.5z" fill="#ff0000"/>',
  MD: v('#0046ae', '#ffd200', '#cc092f'),
  DE: h('#000', '#dd0000', '#ffce00'),
  NL: h('#ae1c28', '#fff', '#21468b'),
  FI: nordic('#fff', '#002f6c'),
  SE: nordic('#006aa7', '#fecc00'),
  NO: nordic('#ba0c2f', '#fff', '#00205b'),
  DK: nordic('#c8102e', '#fff'),
  IS: nordic('#02529c', '#fff', '#dc1e35'),
  FR: v('#0055a4', '#fff', '#ef4135'),
  IT: v('#009246', '#fff', '#ce2b37'),
  IE: v('#169b62', '#fff', '#ff883e'),
  BE: v('#000', '#fdda24', '#ef3340'),
  RO: v('#002b7f', '#fcd116', '#ce1126'),
  PL: h('#fff', '#dc143c'),
  AT: h('#ed2939', '#fff', '#ed2939'),
  LV: bands('h', ['#9e3039', '#fff', '#9e3039'], [2, 1, 2]),
  LT: h('#fdb913', '#006a44', '#c1272d'),
  EE: h('#0072ce', '#000', '#fff'),
  HU: h('#ce2939', '#fff', '#477050'),
  BG: h('#fff', '#00966e', '#d62612'),
  LU: h('#ed2939', '#fff', '#00a1de'),
  RS: h('#c6363c', '#0c4076', '#fff'),
  HR: h('#ff0000', '#fff', '#171796'),
  SK: h('#fff', '#0b4ea2', '#ee1c25') + '<path d="M6.5 9h8v7q0 4-4 5.6-4-1.6-4-5.6z" fill="#ee1c25" stroke="#fff" stroke-width=".9"/><path d="M10 11h1v2h2v1h-2v1.5h2.5v1H11V19h-1v-2.5H7.5v-1H10V14H8v-1h2z" fill="#fff"/>',
  SI: h('#fff', '#005da4', '#ed1c24') + '<path d="M6 6.5h6v5q0 3.5-3 4.6-3-1.1-3-4.6z" fill="#005da4" stroke="#ed1c24" stroke-width=".8"/>',
  CZ: h('#fff', '#d7141a') + '<path d="M0 0l15 15L0 30z" fill="#11457e"/>',
  ES: bands('h', ['#aa151b', '#f1bf00', '#aa151b'], [1, 2, 1]),
  PT: bg('#ff0000') + '<rect width="12" height="30" fill="#006600"/><circle cx="12" cy="15" r="4.6" fill="#ffcc00"/><circle cx="12" cy="15" r="2.7" fill="#ff0000" stroke="#fff" stroke-width=".8"/>',
  GR:
    bands('h', Array.from({ length: 9 }, (_, i) => (i % 2 ? '#fff' : '#0d5eaf'))) +
    '<rect width="16.7" height="16.7" fill="#0d5eaf"/><path d="M6.7 0h3.3v6.7h6.7V10H10v6.7H6.7V10H0V6.7h6.7z" fill="#fff"/>',
  CH: bg('#da291c') + '<path d="M12.5 7h5v5.5H23v5h-5.5V23h-5v-5.5H7v-5h5.5z" fill="#fff"/>',
  GB:
    bg('#012169') +
    '<path d="M0 0l30 30M30 0L0 30" stroke="#fff" stroke-width="6"/><path d="M0 0l30 30M30 0L0 30" stroke="#c8102e" stroke-width="2"/><path d="M15 0v30M0 15h30" stroke="#fff" stroke-width="10"/><path d="M15 0v30M0 15h30" stroke="#c8102e" stroke-width="6"/>',
  US:
    bands('h', Array.from({ length: 13 }, (_, i) => (i % 2 ? '#fff' : '#b22234'))) +
    '<rect width="15" height="16.2" fill="#3c3b6e"/><g fill="#fff">' +
    [3, 7.5, 12].flatMap((x) => [3.2, 8.1, 13].map((y) => `<circle cx="${x}" cy="${y}" r=".95"/>`)).join('') +
    '</g>',
  CA:
    bands('v', ['#d80621', '#fff', '#d80621'], [1, 2, 1]) +
    '<path d="M15 7l1.5 3 1.6-.6-.6 3.6 2.6-1.8.5 1.4 2.1-.3-1 2.9.9.6-3.6 2.5.4 1.3-3.6-.5V23h-1.6v-3.9l-3.6.5.4-1.3-3.6-2.5.9-.6-1-2.9 2.1.3.5-1.4 2.6 1.8-.6-3.6 1.6.6z" fill="#d80621"/>',
  TR: bg('#e30a17') + '<circle cx="12.5" cy="15" r="7" fill="#fff"/><circle cx="14.3" cy="15" r="5.6" fill="#e30a17"/>' + star(20, 15, 3.1, '#fff'),
  AE: h('#00732f', '#fff', '#000') + '<rect width="9" height="30" fill="#ff0000"/>',
  IL:
    bg('#fff') +
    '<rect y="4.5" width="30" height="3.5" fill="#0038b8"/><rect y="22" width="30" height="3.5" fill="#0038b8"/><path d="M15 9.6l4.7 8.1h-9.4zM15 20.4l-4.7-8.1h9.4z" fill="none" stroke="#0038b8" stroke-width="1.1"/>',
  IN: h('#ff9933', '#fff', '#138808') + '<circle cx="15" cy="15" r="3.2" fill="none" stroke="#000080" stroke-width=".9"/>',
  JP: bg('#fff') + '<circle cx="15" cy="15" r="7.5" fill="#bc002d"/>',
  KR: bg('#fff') + '<circle cx="15" cy="15" r="6.5" fill="#cd2e3a"/><path d="M9.4 18.2a6.5 6.5 0 0 0 11.2-6.5 3.25 3.25 0 0 1-5.6 3.3 3.25 3.25 0 0 0-5.6 3.2z" fill="#0047a0"/>',
  CN: bg('#ee1c25') + star(9.5, 10, 4.6, '#ffde00'),
  VN: bg('#da251d') + star(15, 15.5, 7, '#ffff00'),
  HK: bg('#de2910') + '<g fill="#fff">' + [0, 72, 144, 216, 288].map((a) => `<ellipse cx="15" cy="10.6" rx="2.3" ry="4.4" transform="rotate(${a} 15 15)"/>`).join('') + '</g>',
  SG: h('#ef3340', '#fff') + '<circle cx="8.5" cy="8" r="4.3" fill="#fff"/><circle cx="10.3" cy="8" r="4" fill="#ef3340"/><g fill="#fff"><circle cx="12.6" cy="5" r=".9"/><circle cx="14.6" cy="8" r=".9"/><circle cx="12.6" cy="11" r=".9"/></g>',
  TH: bands('h', ['#a51931', '#f4f5f8', '#2d2a4a', '#f4f5f8', '#a51931'], [1, 1, 2, 1, 1]),
  ID: h('#ff0000', '#fff'),
  EU: bg('#003399') + '<g fill="#ffcc00">' + Array.from({ length: 12 }, (_, i) => star(15 + 8 * Math.cos((i * Math.PI) / 6), 15 + 8 * Math.sin((i * Math.PI) / 6), 1.5, '#ffcc00')).join('') + '</g>',
};

/** Готовый SVG флага (для файлов); нет флага — null. */
export function flagSvg(code: string): string | null {
  const art = FLAGS[code];
  return art ? `<svg width="30" height="30" viewBox="0 0 30 30" xmlns="http://www.w3.org/2000/svg">${art}</svg>` : null;
}
