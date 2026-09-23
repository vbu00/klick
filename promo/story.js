// kl!ck — ролик для TikTok в формате истории (9:16, 1080×1920, 30 к/с).
// Всё рисует draw(t): кадр зависит только от времени. Сверху — полоски
// «историй», как в сторис; контент держится в безопасной зоне TikTok
// (сверху ~200 px под вкладки, снизу ~440 px под подпись и кнопки).
'use strict';

const W = 1080, H = 1920, FPS = 30;
const cv = document.getElementById('c');
const ctx = cv.getContext('2d');
const FONT = '"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif';
const C = {
  bg: '#0a0c11', card: '#1a1a1d', card2: '#222226', line: '#2c2c31', text: '#f2f2f4', dim: '#8e8e93', dim2: '#6e6e73',
  blue: '#007AFF', green: '#22C38A', accent: '#30d158', red: '#EA2556', orange: '#FF9D00',
};

// Сцены — они же сегменты полоски сверху.
const SCENES = [
  { id: 'hook', from: 0, to: 4.4, glow: C.red },
  { id: 'loop', from: 4.4, to: 6.8, glow: C.orange },
  { id: 'logo', from: 6.8, to: 10.0, glow: C.blue },
  { id: 'app', from: 10.0, to: 15.0, glow: C.accent },
  { id: 'route', from: 15.0, to: 20.4, glow: C.blue },
  { id: 'kill', from: 20.4, to: 24.8, glow: C.red },
  { id: 'more', from: 24.8, to: 28.0, glow: C.green },
  { id: 'cta', from: 28.0, to: 32.0, glow: C.blue },
];
const DUR = SCENES[SCENES.length - 1].to;

// ─────────── Помощники ───────────

const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const prog = (t, a, b) => clamp((t - a) / (b - a));
const lerp = (a, b, k) => a + (b - a) * k;
const E = {
  out: (x) => 1 - Math.pow(1 - x, 3),
  inOut: (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2),
  back: (x) => { const c1 = 1.70158, c3 = c1 + 1; return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2); },
  bounce: (x) => {
    const n = 7.5625, d = 2.75;
    if (x < 1 / d) return n * x * x;
    if (x < 2 / d) return n * (x -= 1.5 / d) * x + 0.75;
    if (x < 2.5 / d) return n * (x -= 2.25 / d) * x + 0.9375;
    return n * (x -= 2.625 / d) * x + 0.984375;
  },
};
const hexA = (hex, a) => { const n = parseInt(hex.slice(1), 16); return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`; };

function rrect(x, y, w, h, r) {
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
}

/** Текст: \n — перенос; align — left|center|right; parts — цветные куски. */
function text(str, x, y, o = {}) {
  const { size = 48, weight = 600, color = C.text, align = 'center', alpha = 1, lh = 1.18 } = o;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.font = `${weight} ${size}px ${FONT}`;
  ctx.textAlign = align;
  ctx.textBaseline = 'alphabetic';
  ctx.fillStyle = color;
  String(str).split('\n').forEach((line, i) => ctx.fillText(line, x, y + i * size * lh));
  ctx.restore();
}

/** Слово из цветных кусков по центру: [['kl', white], ['!', blue], ['ck', white]].
 *  count — сколько кусков показать (остальные держат место). */
function parts(list, cx, y, size, weight = 700, alpha = 1, count = list.length) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.font = `${weight} ${size}px ${FONT}`;
  ctx.textBaseline = 'alphabetic';
  const widths = list.map(([s]) => ctx.measureText(s).width);
  let x = cx - widths.reduce((a, b) => a + b, 0) / 2;
  list.forEach(([s, color], i) => { if (i < count) { ctx.fillStyle = color; ctx.textAlign = 'left'; ctx.fillText(s, x, y); } x += widths[i]; });
  ctx.restore();
}

const logoWord = (cx, y, size, alpha = 1) => parts([['kl', C.text], ['!', C.blue], ['ck', C.text]], cx, y, size, 700, alpha);

// ─────────── Картинки ───────────

const IMG = {};
const SRC = {
  key: '../src-tauri/icons/icon.png',
  off: 'assets/off.png', on: 'assets/on.png', rules: 'assets/rules.png', ks: 'assets/ks.png', tray: 'assets/tray.png',
};
const loadImages = () => Promise.all(Object.entries(SRC).map(([k, src]) => new Promise((ok, fail) => {
  const im = new Image();
  im.onload = () => { IMG[k] = im; ok(); };
  im.onerror = () => fail(new Error('не загрузилась ' + src));
  im.src = src;
})));

/** Окно приложения со скриншота: скруглённое, с тенью и рамкой. */
function appWindow(img, x, y, w, alpha = 1) {
  const h = (w * img.height) / img.width;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.shadowColor = 'rgba(0,0,0,.55)';
  ctx.shadowBlur = 60;
  ctx.shadowOffsetY = 24;
  rrect(x, y, w, h, 34);
  ctx.fillStyle = C.card;
  ctx.fill();
  ctx.shadowColor = 'transparent';
  ctx.clip();
  ctx.drawImage(img, x, y, w, h);
  ctx.restore();
  ctx.save();
  ctx.globalAlpha *= alpha;
  rrect(x, y, w, h, 34);
  ctx.strokeStyle = 'rgba(255,255,255,.08)';
  ctx.lineWidth = 2;
  ctx.stroke();
  ctx.restore();
  return h;
}

// ─────────── Значки, нарисованные кодом ───────────

function monitor(cx, cy, s, color) {
  ctx.save();
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.lineWidth = s * 0.09;
  rrect(cx - s * 0.5, cy - s * 0.4, s, s * 0.66, s * 0.1);
  ctx.stroke();
  rrect(cx - s * 0.22, cy + s * 0.36, s * 0.44, s * 0.08, s * 0.04);
  ctx.fill();
  ctx.restore();
}

function globe(cx, cy, r, color) {
  ctx.save();
  ctx.strokeStyle = color;
  ctx.lineWidth = r * 0.14;
  ctx.beginPath(); ctx.arc(cx, cy, r, 0, Math.PI * 2); ctx.stroke();
  ctx.beginPath(); ctx.ellipse(cx, cy, r * 0.45, r, 0, 0, Math.PI * 2); ctx.stroke();
  ctx.beginPath(); ctx.moveTo(cx - r, cy); ctx.lineTo(cx + r, cy); ctx.stroke();
  ctx.restore();
}

function shield(cx, cy, s, color, check) {
  ctx.save();
  ctx.beginPath();
  ctx.moveTo(cx, cy - s * 0.5);
  ctx.lineTo(cx + s * 0.4, cy - s * 0.34);
  ctx.bezierCurveTo(cx + s * 0.4, cy + s * 0.1, cx + s * 0.25, cy + s * 0.38, cx, cy + s * 0.52);
  ctx.bezierCurveTo(cx - s * 0.25, cy + s * 0.38, cx - s * 0.4, cy + s * 0.1, cx - s * 0.4, cy - s * 0.34);
  ctx.closePath();
  ctx.fillStyle = hexA(color, 0.16);
  ctx.fill();
  ctx.strokeStyle = color;
  ctx.lineWidth = s * 0.06;
  ctx.lineJoin = 'round';
  ctx.stroke();
  if (check > 0) {
    ctx.beginPath();
    const pts = [[-0.16, 0.02], [-0.03, 0.15], [0.19, -0.1]];
    ctx.moveTo(cx + pts[0][0] * s, cy + pts[0][1] * s);
    const k = clamp(check * 2), k2 = clamp(check * 2 - 1);
    ctx.lineTo(cx + lerp(pts[0][0], pts[1][0], k) * s, cy + lerp(pts[0][1], pts[1][1], k) * s);
    if (k2 > 0) ctx.lineTo(cx + lerp(pts[1][0], pts[2][0], k2) * s, cy + lerp(pts[1][1], pts[2][1], k2) * s);
    ctx.lineCap = 'round';
    ctx.lineWidth = s * 0.08;
    ctx.stroke();
  }
  ctx.restore();
}

function lock(cx, cy, s, color) {
  ctx.save();
  ctx.fillStyle = color;
  ctx.strokeStyle = color;
  ctx.lineWidth = s * 0.13;
  ctx.beginPath(); ctx.arc(cx, cy - s * 0.12, s * 0.24, Math.PI, 0); ctx.stroke();
  rrect(cx - s * 0.36, cy - s * 0.14, s * 0.72, s * 0.56, s * 0.1);
  ctx.fill();
  ctx.restore();
}

// ─────────── Фон и «история» ───────────

function background(t) {
  ctx.fillStyle = C.bg;
  ctx.fillRect(0, 0, W, H);
  // Два мягких пятна цвета текущей сцены, медленно плывут.
  const i = SCENES.findIndex((s) => t < s.to);
  const s = SCENES[Math.max(0, i)];
  const blobs = [[0.25 + 0.08 * Math.sin(t * 0.4), 0.3 + 0.05 * Math.cos(t * 0.3), s.glow, 0.22], [0.8 + 0.06 * Math.cos(t * 0.35), 0.72 + 0.05 * Math.sin(t * 0.5), C.blue, 0.12]];
  for (const [bx, by, color, a] of blobs) {
    const g = ctx.createRadialGradient(bx * W, by * H, 0, bx * W, by * H, 760);
    g.addColorStop(0, hexA(color, a));
    g.addColorStop(1, hexA(color, 0));
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, W, H);
  }
  // Точечная сетка, как на Главной приложения.
  ctx.save();
  ctx.fillStyle = 'rgba(255,255,255,.055)';
  for (let y = 18; y < H; y += 36) for (let x = 18; x < W; x += 36) ctx.fillRect(x, y, 3, 3);
  ctx.restore();
}

function storyChrome(t) {
  // Полоски историй.
  const x0 = 56, x1 = W - 56, gap = 10, y = 196, n = SCENES.length;
  const w = (x1 - x0 - gap * (n - 1)) / n;
  SCENES.forEach((s, i) => {
    const x = x0 + i * (w + gap);
    rrect(x, y, w, 7, 4);
    ctx.fillStyle = 'rgba(255,255,255,.22)';
    ctx.fill();
    const k = prog(t, s.from, s.to);
    if (k > 0) {
      rrect(x, y, Math.max(7, w * k), 7, 4);
      ctx.fillStyle = '#fff';
      ctx.fill();
    }
  });
  // Шапка: аватар-клавиша, имя, «сейчас».
  ctx.save();
  ctx.beginPath(); ctx.arc(94, 262, 38, 0, Math.PI * 2);
  ctx.fillStyle = '#fff';
  ctx.fill();
  ctx.clip();
  ctx.drawImage(IMG.key, 62, 230, 64, 64);
  ctx.restore();
  ctx.save();
  ctx.font = `700 34px ${FONT}`;
  ctx.textAlign = 'left';
  ctx.fillStyle = '#fff';
  ctx.fillText('kl', 148, 274);
  let x = 148 + ctx.measureText('kl').width;
  ctx.fillStyle = C.blue; ctx.fillText('!', x, 274); x += ctx.measureText('!').width;
  ctx.fillStyle = '#fff'; ctx.fillText('ck', x, 274); x += ctx.measureText('ck').width;
  ctx.font = `500 30px ${FONT}`;
  ctx.fillStyle = 'rgba(255,255,255,.6)';
  ctx.fillText('  ·  VPN для Windows', x, 274);
  ctx.restore();
}

// ─────────── Сцены ───────────

/** Карточка браузера с ошибкой. */
function errorCard(y, addr, iconColor, title, sub, alpha, shake) {
  const x = 110 + shake, w = 860, h = 560;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.shadowColor = 'rgba(0,0,0,.5)'; ctx.shadowBlur = 50; ctx.shadowOffsetY = 20;
  rrect(x, y, w, h, 36); ctx.fillStyle = '#f5f5f7'; ctx.fill();
  ctx.shadowColor = 'transparent';
  // Строка браузера.
  rrect(x, y, w, 96, [36, 36, 0, 0]); ctx.fillStyle = '#e4e4e9'; ctx.fill();
  ['#ff5f57', '#febc2e', '#28c840'].forEach((c, i) => { ctx.beginPath(); ctx.arc(x + 44 + i * 34, y + 48, 11, 0, Math.PI * 2); ctx.fillStyle = c; ctx.fill(); });
  rrect(x + 170, y + 24, w - 210, 48, 24); ctx.fillStyle = '#fff'; ctx.fill();
  text(addr, x + 200, y + 58, { size: 28, weight: 500, color: '#5f5f66', align: 'left' });
  // Значок и текст.
  ctx.beginPath(); ctx.arc(x + w / 2, y + 245, 70, 0, Math.PI * 2); ctx.fillStyle = hexA(iconColor, 0.14); ctx.fill();
  ctx.beginPath(); ctx.arc(x + w / 2, y + 245, 44, 0, Math.PI * 2); ctx.fillStyle = iconColor; ctx.fill();
  text('!', x + w / 2, y + 268, { size: 62, weight: 800, color: '#fff' });
  text(title, x + w / 2, y + 400, { size: 50, weight: 700, color: '#1c1c1e' });
  text(sub, x + w / 2, y + 460, { size: 32, weight: 500, color: '#7c7c83' });
  ctx.restore();
}

function sceneHook(lt) {
  const swap = 2.2;
  const a1 = E.out(prog(lt, 0.05, 0.4)) * (1 - prog(lt, swap - 0.2, swap));
  const a2 = E.out(prog(lt, swap, swap + 0.35));
  text('Включил VPN —', 540, 470 - 30 * (1 - E.out(prog(lt, 0, 0.4))), { size: 84, weight: 800, alpha: a1 });
  text('Выключил VPN —', 540, 470 - 30 * (1 - E.out(prog(lt, swap, swap + 0.4))), { size: 84, weight: 800, alpha: a2 });
  const shake1 = lt > 1.0 && lt < 1.5 ? Math.sin(lt * 90) * 14 * (1 - prog(lt, 1.0, 1.5)) : 0;
  const shake2 = lt > swap + 1.0 && lt < swap + 1.5 ? Math.sin(lt * 90) * 14 * (1 - prog(lt, swap + 1.0, swap + 1.5)) : 0;
  const up1 = 80 * (1 - E.back(prog(lt, 0.3, 0.8)));
  const up2 = 80 * (1 - E.back(prog(lt, swap + 0.25, swap + 0.75)));
  if (a1 > 0) errorCard(600 + up1, 'bank.ru', C.red, 'Доступ ограничен', 'Отключите VPN и обновите страницу', a1 * prog(lt, 0.3, 0.5), shake1);
  if (a2 > 0) errorCard(600 + up2, 'youtube.com', C.dim, 'Не удаётся открыть сайт', 'ERR_CONNECTION_RESET', a2 * prog(lt, swap + 0.25, swap + 0.45), shake2);
  text('Госуслуги и банки не пускают\nс VPN', 540, 1300, { size: 42, weight: 500, color: 'rgba(255,255,255,.72)', alpha: a1 * prog(lt, 1.2, 1.5) });
  text('а без VPN не открывается\nполовина интернета', 540, 1300, { size: 42, weight: 500, color: 'rgba(255,255,255,.72)', alpha: a2 * prog(lt, swap + 1.1, swap + 1.4) });
}

function sceneLoop(lt) {
  const a = E.out(prog(lt, 0, 0.3));
  text('И так —', 540, 560, { size: 84, weight: 800, alpha: a });
  text('каждый день', 540, 660, { size: 84, weight: 800, color: C.orange, alpha: E.out(prog(lt, 0.25, 0.55)) });
  // Переключатель щёлкает всё быстрее.
  const beats = [0.5, 0.8, 1.05, 1.25, 1.42, 1.56, 1.68, 1.78, 1.87, 1.95, 2.02, 2.08, 2.14];
  const n = beats.filter((b) => lt >= b).length;
  const on = n % 2 === 1;
  const last = beats[n - 1] ?? 0;
  const k = n ? E.out(prog(lt, last, last + 0.12)) : 0;
  const knob = on ? k : 1 - k;
  const tw = 340, th = 190, tx = 540 - tw / 2, ty = 820;
  ctx.save();
  ctx.globalAlpha = E.out(prog(lt, 0.2, 0.45));
  rrect(tx, ty, tw, th, th / 2);
  ctx.fillStyle = on ? C.accent : '#3a3a40';
  ctx.fill();
  ctx.shadowColor = 'rgba(0,0,0,.4)'; ctx.shadowBlur = 20; ctx.shadowOffsetY = 6;
  ctx.beginPath(); ctx.arc(tx + th / 2 + knob * (tw - th), ty + th / 2, th / 2 - 14, 0, Math.PI * 2);
  ctx.fillStyle = '#fff'; ctx.fill();
  ctx.restore();
  text(on ? 'VPN вкл' : 'VPN выкл', 540, 1120, { size: 52, weight: 700, color: on ? C.accent : C.dim, alpha: E.out(prog(lt, 0.45, 0.6)) });
  text(`× ${n * 3}`, 540, 1260, { size: 110, weight: 800, color: 'rgba(255,255,255,.14)', alpha: n ? 1 : 0 });
}

function sceneLogo(lt) {
  // Клавиша падает и отскакивает.
  const drop = E.bounce(prog(lt, 0.05, 0.95));
  const y = lerp(-400, 860, drop);
  const land = prog(lt, 0.5, 1.6);
  if (land > 0 && land < 1) {
    for (let i = 0; i < 2; i++) {
      const r = lerp(180, 620, clamp(land * 1.4 - i * 0.3));
      const a = (1 - clamp(land * 1.4 - i * 0.3)) * 0.6;
      ctx.beginPath(); ctx.arc(540, 860, r, 0, Math.PI * 2);
      ctx.strokeStyle = hexA(C.blue, a); ctx.lineWidth = 4; ctx.stroke();
    }
  }
  const g = ctx.createRadialGradient(540, 860, 0, 540, 860, 420);
  g.addColorStop(0, hexA(C.blue, 0.35 * prog(lt, 0.4, 1)));
  g.addColorStop(1, hexA(C.blue, 0));
  ctx.fillStyle = g; ctx.fillRect(0, 400, W, 900);
  const s = 440, wob = Math.sin(lt * 2.2) * 0.03 * prog(lt, 1, 1.5);
  ctx.save();
  ctx.translate(540, y);
  ctx.rotate(wob);
  ctx.shadowColor = 'rgba(0,40,120,.6)'; ctx.shadowBlur = 60; ctx.shadowOffsetY = 30;
  ctx.drawImage(IMG.key, -s / 2, -s / 2, s, s);
  ctx.restore();
  // Слово по буквам.
  const letters = [['k', C.text], ['l', C.text], ['!', C.blue], ['c', C.text], ['k', C.text]];
  const shown = Math.floor(prog(lt, 1.0, 1.6) * letters.length + 0.001);
  if (shown > 0) parts(letters, 540, 1260, 170, 800, 1, shown);
  text('VPN-клиент, который сам знает,\nкуда что пускать', 540, 1360, { size: 44, weight: 500, color: 'rgba(255,255,255,.75)', alpha: E.out(prog(lt, 1.7, 2.1)) });
}

function sceneApp(lt) {
  const tap = 1.4;
  const enter = E.out(prog(lt, 0, 0.5));
  const trayIn = E.out(prog(lt, 3.0, 3.5));
  const w = 540, x = 270 - 170 * trayIn, y = 520 + 120 * (1 - enter);
  // Подпись.
  text('Одна кнопка —', 540, 400, { size: 72, weight: 800, alpha: enter * (1 - prog(lt, 2.8, 3.0)) });
  text('и весь ПК под VPN', 540, 480, { size: 60, weight: 700, color: C.accent, alpha: E.out(prog(lt, tap + 0.3, tap + 0.6)) * (1 - prog(lt, 2.8, 3.0)) });
  text('Скорость, серверы и Kill Switch —', 540, 400, { size: 52, weight: 700, alpha: E.out(prog(lt, 3.0, 3.3)) });
  text('прямо из трея', 540, 470, { size: 52, weight: 700, color: C.accent, alpha: E.out(prog(lt, 3.1, 3.4)) });
  const cross = prog(lt, tap + 0.05, tap + 0.4);
  appWindow(IMG.off, x, y, w, enter * (1 - cross * 0.999));
  appWindow(IMG.on, x, y, w, enter * cross);
  // Нажатие на кнопку питания: в окне 380×720 она в точке (190, 214).
  const s = w / 380, px = x + 190 * s, py = y + 214 * s;
  const fingerA = prog(lt, 0.8, 1.0) * (1 - prog(lt, tap + 0.4, tap + 0.7));
  if (fingerA > 0) {
    const press = lt > tap - 0.1 && lt < tap + 0.15 ? 0.85 : 1;
    ctx.save();
    ctx.globalAlpha = fingerA;
    ctx.beginPath(); ctx.arc(px + 20, py + 30, 46 * press, 0, Math.PI * 2);
    ctx.fillStyle = 'rgba(255,255,255,.85)'; ctx.fill();
    ctx.restore();
  }
  const rip = prog(lt, tap, tap + 0.9);
  if (rip > 0 && rip < 1) {
    ctx.beginPath(); ctx.arc(px, py, lerp(60, 330, E.out(rip)), 0, Math.PI * 2);
    ctx.strokeStyle = hexA(C.accent, 0.7 * (1 - rip)); ctx.lineWidth = 6; ctx.stroke();
  }
  // Меню трея выезжает справа.
  if (trayIn > 0) {
    const tw = 460, tx = lerp(W + 40, 560, trayIn), ty = 700;
    appWindow(IMG.tray, tx, ty, tw, trayIn);
  }
}

/** Путь-кривая с бегущими штрихами и «пакетами». */
function flow(pts, color, lt, speed, width = 8, alpha = 1) {
  const [a, b, c, d] = pts;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.beginPath(); ctx.moveTo(...a); ctx.bezierCurveTo(...b, ...c, ...d);
  ctx.strokeStyle = hexA(color, 0.18); ctx.lineWidth = width + 10; ctx.lineCap = 'round'; ctx.stroke();
  ctx.setLineDash([22, 26]);
  ctx.lineDashOffset = -lt * speed;
  ctx.strokeStyle = color; ctx.lineWidth = width; ctx.stroke();
  ctx.setLineDash([]);
  for (let i = 0; i < 3; i++) {
    const u = (lt * 0.45 + i / 3) % 1;
    const m = 1 - u;
    const px = m * m * m * a[0] + 3 * m * m * u * b[0] + 3 * m * u * u * c[0] + u * u * u * d[0];
    const py = m * m * m * a[1] + 3 * m * m * u * b[1] + 3 * m * u * u * c[1] + u * u * u * d[1];
    ctx.beginPath(); ctx.arc(px, py, 12, 0, Math.PI * 2); ctx.fillStyle = color; ctx.fill();
  }
  ctx.restore();
}

function tile(cx, cy, size, bg, draw, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  rrect(cx - size / 2, cy - size / 2, size, size, size * 0.26);
  ctx.fillStyle = bg; ctx.fill();
  draw();
  ctx.restore();
}

function pill(cx, cy, label, color, alpha = 1, bg) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.font = `700 34px ${FONT}`;
  const w = ctx.measureText(label).width + 90;
  rrect(cx - w / 2, cy - 36, w, 72, 36);
  ctx.fillStyle = bg || hexA(color, 0.18); ctx.fill();
  ctx.strokeStyle = hexA(color, 0.55); ctx.lineWidth = 3; ctx.stroke();
  ctx.beginPath(); ctx.arc(cx - w / 2 + 38, cy, 9, 0, Math.PI * 2); ctx.fillStyle = color; ctx.fill();
  text(label, cx + 18, cy + 12, { size: 34, weight: 700, color: '#fff' });
  ctx.restore();
}

function sceneRoute(lt) {
  const a0 = E.out(prog(lt, 0, 0.4));
  text('Российские сайты —', 540, 410, { size: 64, weight: 800, alpha: a0 });
  text('напрямую', 540, 490, { size: 64, weight: 800, color: '#fff', alpha: a0 });
  text('Остальное — через VPN', 540, 590, { size: 64, weight: 800, color: C.blue, alpha: E.out(prog(lt, 0.4, 0.8)) });
  const pc = [170, 1010], top = [870, 800], bot = [870, 1220];
  const aTop = E.out(prog(lt, 0.9, 1.4)), aBot = E.out(prog(lt, 1.6, 2.1));
  flow([pc, [340, 1010], [360, 800], [540, 800]], C.blue, lt, 140, 8, aTop);
  flow([[540, 800], [690, 800], [700, 800], [top[0] - 90, top[1]]], C.blue, lt, 140, 8, aTop);
  flow([pc, [340, 1010], [360, 1220], [540, 1220]], '#d9dde6', lt, 110, 7, aBot);
  flow([[540, 1220], [690, 1220], [700, 1220], [bot[0] - 90, bot[1]]], '#d9dde6', lt, 110, 7, aBot);
  tile(pc[0], pc[1], 170, '#23262e', () => monitor(pc[0], pc[1] - 8, 88, '#fff'), E.out(prog(lt, 0.6, 0.9)));
  text('твой ПК', pc[0], pc[1] + 140, { size: 34, weight: 600, color: 'rgba(255,255,255,.7)', alpha: E.out(prog(lt, 0.6, 0.9)) });
  pill(540, 800, 'VPN', C.blue, aTop, '#0d2a55');
  pill(540, 1220, 'напрямую', '#d9dde6', aBot, '#23262e');
  tile(top[0], top[1], 150, hexA(C.blue, 0.22), () => globe(top[0], top[1], 44, '#fff'), aTop);
  text('YouTube · Discord\nChatGPT · Instagram', top[0] - 20, top[1] + 130, { size: 30, weight: 600, color: 'rgba(255,255,255,.8)', alpha: aTop });
  tile(bot[0], bot[1], 150, '#23262e', () => globe(bot[0], bot[1], 44, '#fff'), aBot);
  text('Госуслуги · банки\nмаркетплейсы · .ru', bot[0] - 20, bot[1] + 130, { size: 30, weight: 600, color: 'rgba(255,255,255,.8)', alpha: aBot });
  const a3 = E.out(prog(lt, 3.2, 3.6));
  text('Само. Без переключений.', 540, 1500 - 20 * (1 - a3), { size: 52, weight: 800, color: C.green, alpha: a3 });
}

function sceneKill(lt) {
  const drop = 0.9;
  const a0 = E.out(prog(lt, 0, 0.35));
  text('VPN оборвался?', 540, 420, { size: 84, weight: 800, alpha: a0 });
  // Плашка статуса: подключено → оборвался.
  const broke = lt >= drop;
  const shake = lt > drop && lt < drop + 0.4 ? Math.sin(lt * 80) * 12 * (1 - prog(lt, drop, drop + 0.4)) : 0;
  pill(540 + shake, 560, broke ? 'VPN оборвался' : 'VPN подключён', broke ? C.red : C.accent, a0);
  // Щит.
  const sOn = prog(lt, drop + 0.4, drop + 0.9);
  const g = ctx.createRadialGradient(540, 860, 0, 540, 860, 360);
  g.addColorStop(0, hexA(C.green, 0.3 * sOn)); g.addColorStop(1, hexA(C.green, 0));
  ctx.fillStyle = g; ctx.fillRect(0, 500, W, 760);
  shield(540, 850, 300 * (1 + 0.08 * Math.sin(prog(lt, drop + 0.4, drop + 0.9) * Math.PI)), sOn > 0 ? C.green : C.dim2, E.out(prog(lt, drop + 0.6, drop + 1.1)));
  text('Kill Switch', 540, 1080, { size: 58, weight: 800, color: sOn > 0 ? C.green : C.dim, alpha: E.out(prog(lt, 0.3, 0.6)) });
  // Программы остаются без интернета.
  const apps = [['q', '#a8c7fa', 'торрент'], ['T', '#9ad9c0', 'мессенджер'], ['D', '#c4b5fd', 'браузер']];
  apps.forEach(([ch, tint, label], i) => {
    const cx = 300 + i * 240, cy = 1240;
    const a = E.out(prog(lt, 0.5 + i * 0.1, 0.8 + i * 0.1));
    tile(cx, cy, 150, tint, () => text(ch, cx, cy + 26, { size: 72, weight: 800, color: '#1a1a1d' }), a);
    const l = E.back(prog(lt, drop + 0.9 + i * 0.15, drop + 1.25 + i * 0.15));
    if (l > 0) {
      ctx.save();
      ctx.translate(cx + 60, cy - 60); ctx.scale(l, l);
      ctx.beginPath(); ctx.arc(0, 0, 38, 0, Math.PI * 2); ctx.fillStyle = C.red; ctx.fill();
      lock(0, 4, 40, '#fff');
      ctx.restore();
    }
    text(label, cx, cy + 120, { size: 30, weight: 600, color: 'rgba(255,255,255,.7)', alpha: a });
  });
  text('не утекут мимо VPN', 540, 1480, { size: 50, weight: 700, color: '#fff', alpha: E.out(prog(lt, drop + 1.6, drop + 2.0)) });
}

function sceneMore(lt) {
  text('А ещё', 540, 420, { size: 84, weight: 800, alpha: E.out(prog(lt, 0, 0.3)) });
  const chips = [
    ['VLESS · Reality · XTLS', C.blue], ['Hysteria2 · TUIC · Trojan', C.blue], ['Подписки Remnawave · Marzban · 3x-ui', C.green],
    ['Kill Switch и для сайтов', C.red], ['Трей со скоростью и пингом', C.accent], ['4 темы · 6 акцентов', C.orange],
    ['Ядро mihomo без изменений', C.blue], ['Свежая база GeoIP сама', C.green],
  ];
  chips.forEach(([label, color], i) => {
    const a = E.back(prog(lt, 0.3 + i * 0.14, 0.65 + i * 0.14));
    if (a <= 0) return;
    const y = 560 + i * 118;
    ctx.save();
    ctx.globalAlpha = clamp(a);
    ctx.translate(540, y);
    ctx.scale(0.8 + 0.2 * a, 0.8 + 0.2 * a);
    ctx.font = `700 40px ${FONT}`;
    const w = ctx.measureText(label).width + 110;
    rrect(-w / 2, -44, w, 88, 44);
    ctx.fillStyle = '#1d2027'; ctx.fill();
    ctx.strokeStyle = hexA(color, 0.5); ctx.lineWidth = 3; ctx.stroke();
    ctx.beginPath(); ctx.arc(-w / 2 + 44, 0, 11, 0, Math.PI * 2); ctx.fillStyle = color; ctx.fill();
    text(label, 20, 14, { size: 40, weight: 700 });
    ctx.restore();
  });
}

function sceneCta(lt) {
  const a = E.out(prog(lt, 0, 0.5));
  const s = 320 * (0.8 + 0.2 * E.back(prog(lt, 0, 0.6)));
  const fy = 720 + Math.sin(lt * 2) * 12;
  const g = ctx.createRadialGradient(540, fy, 0, 540, fy, 360);
  g.addColorStop(0, hexA(C.blue, 0.35 * a)); g.addColorStop(1, hexA(C.blue, 0));
  ctx.fillStyle = g; ctx.fillRect(0, 300, W, 900);
  ctx.save();
  ctx.globalAlpha = a;
  ctx.drawImage(IMG.key, 540 - s / 2, fy - s / 2, s, s);
  ctx.restore();
  logoWord(540, 1040, 150, E.out(prog(lt, 0.2, 0.6)));
  text('Бесплатно · Windows 10/11 · открытый код', 540, 1130, { size: 38, weight: 500, color: 'rgba(255,255,255,.75)', alpha: E.out(prog(lt, 0.4, 0.8)) });
  const b = E.back(prog(lt, 0.7, 1.1));
  if (b > 0) {
    ctx.save();
    ctx.translate(540, 1270);
    ctx.scale(b, b);
    rrect(-340, -58, 680, 116, 58);
    ctx.fillStyle = C.blue; ctx.fill();
    text('github.com/vbu00/klick', 0, 15, { size: 44, weight: 700, color: '#fff' });
    ctx.restore();
  }
  const bob = Math.sin(lt * 6) * 8;
  text('ссылка в профиле ↓', 540, 1430 + bob, { size: 40, weight: 600, color: 'rgba(255,255,255,.7)', alpha: E.out(prog(lt, 1.2, 1.6)) });
}

const DRAW = { hook: sceneHook, loop: sceneLoop, logo: sceneLogo, app: sceneApp, route: sceneRoute, kill: sceneKill, more: sceneMore, cta: sceneCta };

// ─────────── Кадр ───────────

function draw(t) {
  t = clamp(t, 0, DUR - 1e-6);
  background(t);
  const s = SCENES.find((x) => t >= x.from && t < x.to);
  const lt = t - s.from;
  // Выход сцены: лёгкий уход вверх и затухание за 0,25 с до конца.
  const out = s === SCENES[SCENES.length - 1] ? 0 : prog(t, s.to - 0.25, s.to);
  ctx.save();
  ctx.globalAlpha = 1 - out;
  ctx.translate(0, -40 * E.inOut(out));
  DRAW[s.id](lt);
  ctx.restore();
  storyChrome(t);
}

// ─────────── Запись в MP4 (render.mjs) и просмотр ───────────

async function encodeStory(onProgress) {
  const muxer = new Mp4Muxer.Muxer({ target: new Mp4Muxer.ArrayBufferTarget(), video: { codec: 'avc', width: W, height: H, frameRate: FPS }, fastStart: 'in-memory' });
  let failure = null;
  const enc = new VideoEncoder({ output: (chunk, meta) => muxer.addVideoChunk(chunk, meta), error: (e) => { failure = e; } });
  const config = { codec: 'avc1.640028', width: W, height: H, bitrate: 12_000_000, framerate: FPS, avc: { format: 'avc' } };
  const sup = await VideoEncoder.isConfigSupported(config);
  if (!sup.supported) throw new Error('H.264 1080×1920 не поддерживается кодеком браузера');
  enc.configure(config);
  const frames = Math.round(DUR * FPS);
  for (let i = 0; i < frames; i++) {
    if (failure) throw failure;
    draw(i / FPS);
    const f = new VideoFrame(cv, { timestamp: Math.round((i * 1e6) / FPS), duration: Math.round(1e6 / FPS) });
    enc.encode(f, { keyFrame: i % (FPS * 2) === 0 });
    f.close();
    while (enc.encodeQueueSize > 8) await new Promise((r) => setTimeout(r, 1));
    if (i % 30 === 0) onProgress?.(i, frames);
  }
  await enc.flush();
  if (failure) throw failure;
  muxer.finalize();
  const blob = new Blob([muxer.target.buffer], { type: 'video/mp4' });
  return await new Promise((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result).split(',')[1]); fr.readAsDataURL(blob); });
}

function frameAt(t) {
  draw(t);
  return cv.toDataURL('image/png').split(',')[1];
}

(async () => {
  await document.fonts.load(`700 40px ${FONT}`);
  await loadImages();
  window.encodeStory = encodeStory;
  window.frameAt = frameAt;
  window.storyReady = true;
  const q = new URLSearchParams(location.search);
  if (q.has('render')) { draw(0); return; }
  if (q.has('t')) { draw(parseFloat(q.get('t'))); return; }
  let t0 = performance.now(), paused = false, pausedAt = 0;
  document.addEventListener('keydown', (e) => {
    if (e.code !== 'Space') return;
    paused = !paused;
    if (paused) pausedAt = performance.now(); else t0 += performance.now() - pausedAt;
  });
  const loop = () => {
    if (!paused) draw(((performance.now() - t0) / 1000) % DUR);
    requestAnimationFrame(loop);
  };
  loop();
})();
