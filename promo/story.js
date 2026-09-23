// kl!ck — ролик для TikTok «POV: ты зашёл в катку» (9:16, 1080×1920, 30 к/с,
// со звуком). Картинку рисует draw(t), звук собирает buildAudio() — и то и
// другое зависит только от времени, поэтому просмотр в браузере и MP4 из
// render.mjs совпадают кадр в кадр. Сверху — полоски «историй», контент —
// в безопасной зоне TikTok (сверху ~200 px вкладки, снизу ~440 px подпись).
'use strict';

const W = 1080, H = 1920, FPS = 30, RATE = 48000;
const cv = document.getElementById('c');
const ctx = cv.getContext('2d');
const FONT = '"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif';
const C = {
  bg: '#0a0c11', card: '#1a1a1d', text: '#f2f2f4', dim: '#8e8e93', dim2: '#6e6e73',
  blue: '#007AFF', green: '#22C38A', accent: '#30d158', red: '#EA2556', orange: '#FF9D00', voice: '#2b2d31',
};

// Сцены — они же сегменты полоски сверху.
const SCENES = [
  { id: 'hook', from: 0, to: 3.2, glow: C.red },
  { id: 'trap', from: 3.2, to: 9.6, glow: C.orange },
  { id: 'logo', from: 9.6, to: 11.2, glow: C.blue },
  { id: 'rules', from: 11.2, to: 17.6, glow: C.accent },
  { id: 'viz', from: 17.6, to: 20.6, glow: C.blue },
  { id: 'tray', from: 20.6, to: 25.4, glow: C.accent },
  { id: 'kill', from: 25.4, to: 29.8, glow: C.red },
  { id: 'setup', from: 29.8, to: 32.4, glow: C.blue },
  { id: 'cta', from: 32.4, to: 36.4, glow: C.blue },
];
const DUR = SCENES[SCENES.length - 1].to;

// Ключевые моменты — по ним сведены и картинка, и звук.
const T = {
  lag: [0.35, 0.95, 1.7, 2.4],
  vpnOnChip: 3.35, bubble1: 4.3, bubble2: 5.1,
  vpnOff: 6.0, split: 8.2, orNot: 9.0,
  keyLand: 10.25, letters: [10.45, 10.55, 10.65, 10.75, 10.85],
  winIn: 11.3, tapCs2: 12.35, tapAdd: 13.05, swap: 13.4, hiCs2: 13.8, hiDiscord: 14.4, pingFrom: 15.0, pingTo: 16.2, success: 16.3,
  vizIn: 17.6, vizNode1: 18.05, vizNode2: 18.65,
  trayIn: 21.1, tapPing: 22.1, pingsDone: 22.9, tapServer: 23.3,
  nightIn: 25.4, vpnDrop: 26.8, lockOn: 27.35,
  typeFrom: 30.2, typeTo: 31.2, tapPaste: 31.4, rows: [31.65, 31.8, 31.95, 32.1],
  themes: [32.5, 32.72, 32.94, 33.16], finalKey: 33.45,
};

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
/** Детерминированный ГПСЧ — кадры и звук не зависят от Math.random. */
function rng(seed) {
  return () => { seed |= 0; seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
const hash = (i) => rng(i * 7919 + 13)();

function rrect(x, y, w, h, r) { ctx.beginPath(); ctx.roundRect(x, y, w, h, r); }

function text(str, x, y, o = {}) {
  const { size = 48, weight = 600, color = C.text, align = 'center', alpha = 1, lh = 1.18 } = o;
  if (alpha <= 0) return;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.font = `${weight} ${size}px ${FONT}`;
  ctx.textAlign = align;
  ctx.fillStyle = color;
  String(str).split('\n').forEach((line, i) => ctx.fillText(line, x, y + i * size * lh));
  ctx.restore();
}

/** Слово из цветных кусков по центру; count — сколько показать (остальные держат место). */
function parts(list, cx, y, size, weight = 700, alpha = 1, count = list.length) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.font = `${weight} ${size}px ${FONT}`;
  const widths = list.map(([s]) => ctx.measureText(s).width);
  let x = cx - widths.reduce((a, b) => a + b, 0) / 2;
  ctx.textAlign = 'left';
  list.forEach(([s, color], i) => { if (i < count) { ctx.fillStyle = color; ctx.fillText(s, x, y); } x += widths[i]; });
  ctx.restore();
}
const LOGO = [['k', C.text], ['l', C.text], ['!', C.blue], ['c', C.text], ['k', C.text]];

/** Заголовок сцены: въезжает снизу. */
function title(str, y, lt, at = 0, o = {}) {
  const k = E.out(prog(lt, at, at + 0.35));
  text(str, 540, y + 26 * (1 - k), { size: 72, weight: 800, ...o, alpha: k * (o.alpha ?? 1) });
}

function chip(cx, cy, label, color, o = {}) {
  const { alpha = 1, size = 34, bg, scale = 1 } = o;
  if (alpha <= 0) return;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.translate(cx, cy);
  ctx.scale(scale, scale);
  ctx.font = `700 ${size}px ${FONT}`;
  const w = ctx.measureText(label).width + size * 2.6;
  rrect(-w / 2, -size * 1.05, w, size * 2.1, size * 1.05);
  ctx.fillStyle = bg || hexA(color, 0.18); ctx.fill();
  ctx.strokeStyle = hexA(color, 0.6); ctx.lineWidth = 3; ctx.stroke();
  ctx.beginPath(); ctx.arc(-w / 2 + size * 1.1, 0, size * 0.27, 0, Math.PI * 2); ctx.fillStyle = color; ctx.fill();
  text(label, size * 0.55, size * 0.36, { size, weight: 700 });
  ctx.restore();
}

// ─────────── Картинки ───────────

const IMG = {};
const SRC = {
  key: '../src-tauri/icons/icon.png',
  picker: 'assets/picker.png', rules: 'assets/rules.png', trayBefore: 'assets/tray-before.png', trayAfter: 'assets/tray-after.png',
  thLight: '../docs/screenshots/home-light.png', thMid: '../docs/screenshots/home-midnight.png', thOled: '../docs/screenshots/rules-oled.png', thGraph: '../docs/screenshots/theme-graphite.png',
};
const loadImages = () => Promise.all(Object.entries(SRC).map(([k, src]) => new Promise((ok, fail) => {
  const im = new Image();
  im.onload = () => { IMG[k] = im; ok(); };
  im.onerror = () => fail(new Error('не загрузилась ' + src));
  im.src = src;
})));

/** Картинка в скруглённом «окне» с тенью; crop — [sx, sy, sw, sh] исходника. */
function framed(img, x, y, w, o = {}) {
  const { alpha = 1, crop = [0, 0, img.width, img.height], radius = 30 } = o;
  const h = (w * crop[3]) / crop[2];
  if (alpha <= 0) return h;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.shadowColor = 'rgba(0,0,0,.6)'; ctx.shadowBlur = 60; ctx.shadowOffsetY = 24;
  rrect(x, y, w, h, radius); ctx.fillStyle = C.card; ctx.fill();
  ctx.shadowColor = 'transparent';
  ctx.clip();
  ctx.drawImage(img, crop[0], crop[1], crop[2], crop[3], x, y, w, h);
  ctx.restore();
  ctx.save();
  ctx.globalAlpha *= alpha;
  rrect(x, y, w, h, radius); ctx.strokeStyle = 'rgba(255,255,255,.1)'; ctx.lineWidth = 2; ctx.stroke();
  ctx.restore();
  return h;
}
// Меню трея — карточка внутри скриншота 720×1150; окна документации сняты
// в кадре 1040×1680.
const TRAY_CROP = [20, 16, 640, 1094];
const DOCS_CROP = [140, 120, 760, 1440];

/** Нажатие: точка-палец и расходящееся кольцо. */
function tap(x, y, t, at, color = '#fff') {
  const f = prog(t, at - 0.35, at - 0.1) * (1 - prog(t, at + 0.25, at + 0.45));
  if (f > 0) {
    const press = t > at - 0.06 && t < at + 0.12 ? 0.82 : 1;
    ctx.save(); ctx.globalAlpha = f * 0.9;
    ctx.beginPath(); ctx.arc(x, y, 34 * press, 0, Math.PI * 2); ctx.fillStyle = color; ctx.fill();
    ctx.restore();
  }
  const r = prog(t, at, at + 0.5);
  if (r > 0 && r < 1) {
    ctx.beginPath(); ctx.arc(x, y, lerp(30, 130, E.out(r)), 0, Math.PI * 2);
    ctx.strokeStyle = hexA(C.accent, 0.8 * (1 - r)); ctx.lineWidth = 6; ctx.stroke();
  }
}

// ─────────── Значки кодом ───────────

function monitor(cx, cy, s, color) {
  ctx.save(); ctx.strokeStyle = color; ctx.fillStyle = color; ctx.lineWidth = s * 0.09;
  rrect(cx - s * 0.5, cy - s * 0.4, s, s * 0.66, s * 0.1); ctx.stroke();
  rrect(cx - s * 0.22, cy + s * 0.36, s * 0.44, s * 0.08, s * 0.04); ctx.fill();
  ctx.restore();
}
function crosshairIcon(cx, cy, s, color) {
  ctx.save(); ctx.strokeStyle = color; ctx.lineWidth = s * 0.1; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.arc(cx, cy, s * 0.36, 0, Math.PI * 2); ctx.stroke();
  for (const [dx, dy] of [[0, -1], [0, 1], [-1, 0], [1, 0]]) { ctx.beginPath(); ctx.moveTo(cx + dx * s * 0.2, cy + dy * s * 0.2); ctx.lineTo(cx + dx * s * 0.52, cy + dy * s * 0.52); ctx.stroke(); }
  ctx.restore();
}
function headset(cx, cy, s, color) {
  ctx.save(); ctx.strokeStyle = color; ctx.fillStyle = color; ctx.lineWidth = s * 0.1;
  ctx.beginPath(); ctx.arc(cx, cy + s * 0.05, s * 0.38, Math.PI, 0); ctx.stroke();
  rrect(cx - s * 0.46, cy, s * 0.2, s * 0.34, s * 0.06); ctx.fill();
  rrect(cx + s * 0.26, cy, s * 0.2, s * 0.34, s * 0.06); ctx.fill();
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
  ctx.fillStyle = hexA(color, 0.16); ctx.fill();
  ctx.strokeStyle = color; ctx.lineWidth = s * 0.06; ctx.lineJoin = 'round'; ctx.stroke();
  if (check > 0) {
    const p = [[-0.16, 0.02], [-0.03, 0.15], [0.19, -0.1]];
    const k = clamp(check * 2), k2 = clamp(check * 2 - 1);
    ctx.beginPath();
    ctx.moveTo(cx + p[0][0] * s, cy + p[0][1] * s);
    ctx.lineTo(cx + lerp(p[0][0], p[1][0], k) * s, cy + lerp(p[0][1], p[1][1], k) * s);
    if (k2 > 0) ctx.lineTo(cx + lerp(p[1][0], p[2][0], k2) * s, cy + lerp(p[1][1], p[2][1], k2) * s);
    ctx.lineCap = 'round'; ctx.lineWidth = s * 0.08; ctx.stroke();
  }
  ctx.restore();
}
function lock(cx, cy, s, color) {
  ctx.save(); ctx.fillStyle = color; ctx.strokeStyle = color; ctx.lineWidth = s * 0.13;
  ctx.beginPath(); ctx.arc(cx, cy - s * 0.12, s * 0.24, Math.PI, 0); ctx.stroke();
  rrect(cx - s * 0.36, cy - s * 0.14, s * 0.72, s * 0.56, s * 0.1); ctx.fill();
  ctx.restore();
}

// ─────────── Фон, игра, «история» ───────────

function background(t) {
  ctx.fillStyle = C.bg;
  ctx.fillRect(0, 0, W, H);
  const s = SCENES.find((x) => t < x.to) || SCENES[SCENES.length - 1];
  const blobs = [[0.25 + 0.08 * Math.sin(t * 0.4), 0.32 + 0.05 * Math.cos(t * 0.3), s.glow, 0.22], [0.8 + 0.06 * Math.cos(t * 0.35), 0.7 + 0.05 * Math.sin(t * 0.5), C.blue, 0.12]];
  for (const [bx, by, color, a] of blobs) {
    const g = ctx.createRadialGradient(bx * W, by * H, 0, bx * W, by * H, 760);
    g.addColorStop(0, hexA(color, a)); g.addColorStop(1, hexA(color, 0));
    ctx.fillStyle = g; ctx.fillRect(0, 0, W, H);
  }
  ctx.fillStyle = 'rgba(255,255,255,.05)';
  for (let y = 18; y < H; y += 36) for (let x = 18; x < W; x += 36) ctx.fillRect(x, y, 3, 3);
}

/** Стилизованный кадр шутера: коридор в пыльной пустыне, прицел, HUD.
 *  lag — сила «лага» (рывки кадра и полосы), ping — число в неткрафе,
 *  net — показывать ли неткрафу (где пинг и так крупно — не нужна). */
function game(t, o = {}) {
  const { lag = 0, ping = 180, dim = 0, net = true } = o;
  ctx.save();
  const step = Math.floor(t * 9);
  const jx = lag * (hash(step) - 0.5) * 70, jy = lag * (hash(step + 99) - 0.5) * 30;
  ctx.translate(jx, jy);
  const sky = ctx.createLinearGradient(0, 0, 0, 900);
  sky.addColorStop(0, '#8fb6dc'); sky.addColorStop(1, '#f0d3a0');
  ctx.fillStyle = sky; ctx.fillRect(-60, -60, W + 120, 1000);
  const ground = ctx.createLinearGradient(0, 1100, 0, H);
  ground.addColorStop(0, '#c99a62'); ground.addColorStop(1, '#8e6238');
  ctx.fillStyle = ground; ctx.fillRect(-60, 1100, W + 120, H);
  const poly = (pts, color) => { ctx.beginPath(); ctx.moveTo(...pts[0]); pts.slice(1).forEach((p) => ctx.lineTo(...p)); ctx.closePath(); ctx.fillStyle = color; ctx.fill(); };
  poly([[330, 640], [750, 640], [750, 1180], [330, 1180]], '#e2bf8b');
  poly([[470, 860], [610, 860], [610, 1180], [470, 1180]], '#4e3b28');
  ctx.beginPath(); ctx.arc(540, 860, 70, Math.PI, 0); ctx.fillStyle = '#4e3b28'; ctx.fill();
  poly([[-60, 280], [330, 640], [330, 1180], [-60, 1700]], '#d5a86f');
  poly([[-60, 280], [330, 640], [330, 700], [-60, 420]], '#b98c56');
  poly([[W + 60, 280], [750, 640], [750, 1180], [W + 60, 1700]], '#c99a60');
  poly([[W + 60, 280], [750, 640], [750, 700], [W + 60, 420]], '#a8794a');
  poly([[640, 1110], [800, 1110], [800, 1250], [640, 1250]], '#8a6a44');
  ctx.strokeStyle = '#6e5234'; ctx.lineWidth = 8;
  ctx.strokeRect(648, 1118, 144, 124);
  ctx.beginPath(); ctx.moveTo(648, 1118); ctx.lineTo(792, 1242); ctx.stroke();
  const vg = ctx.createRadialGradient(540, 960, 300, 540, 960, 1150);
  vg.addColorStop(0, 'rgba(0,0,0,0)'); vg.addColorStop(1, 'rgba(0,0,0,.55)');
  ctx.fillStyle = vg; ctx.fillRect(-60, -60, W + 120, H + 120);
  ctx.restore();
  if (lag > 0.3) {
    for (let i = 0; i < 4; i++) {
      const r = hash(step * 7 + i);
      ctx.fillStyle = `rgba(255,255,255,${0.06 + 0.1 * r})`;
      ctx.fillRect(0, 300 + r * 1200, W, 10 + r * 40);
    }
  }
  ctx.save();
  ctx.strokeStyle = 'rgba(0,0,0,.6)'; ctx.lineWidth = 9;
  const cross = () => { for (const [dx, dy] of [[0, -1], [0, 1], [-1, 0], [1, 0]]) { ctx.beginPath(); ctx.moveTo(540 + dx * 12, 960 + dy * 12); ctx.lineTo(540 + dx * 36, 960 + dy * 36); ctx.stroke(); } };
  cross(); ctx.strokeStyle = '#52ff52'; ctx.lineWidth = 5; cross();
  ctx.restore();
  rrect(390, 330, 300, 86, 16); ctx.fillStyle = 'rgba(10,12,16,.72)'; ctx.fill();
  text('7', 450, 390, { size: 48, weight: 800, color: '#8fc3ff' });
  text('1:24', 540, 388, { size: 40, weight: 700 });
  text('9', 630, 390, { size: 48, weight: 800, color: '#ffc36b' });
  const pc = ping >= 120 ? '#ff5a5a' : ping >= 70 ? '#ffd166' : '#52ff52';
  if (net) text(`ping ${Math.round(ping)} ms   loss ${ping >= 120 ? 4 : 0}%`, 70, 470, { size: 34, weight: 700, color: pc, align: 'left' });
  text('✚ 100', 70, 1400, { size: 56, weight: 800, align: 'left' });
  text('30 / 90', 900, 1400, { size: 56, weight: 800, align: 'right' });
  if (dim > 0) { ctx.fillStyle = `rgba(6,8,12,${dim})`; ctx.fillRect(0, 0, W, H); }
}

function storyChrome(t) {
  const x0 = 56, x1 = W - 56, gap = 10, y = 196, n = SCENES.length;
  const w = (x1 - x0 - gap * (n - 1)) / n;
  SCENES.forEach((s, i) => {
    const x = x0 + i * (w + gap);
    rrect(x, y, w, 7, 4); ctx.fillStyle = 'rgba(255,255,255,.25)'; ctx.fill();
    const k = prog(t, s.from, s.to);
    if (k > 0) { rrect(x, y, Math.max(7, w * k), 7, 4); ctx.fillStyle = '#fff'; ctx.fill(); }
  });
  ctx.save();
  ctx.shadowColor = 'rgba(0,0,0,.5)'; ctx.shadowBlur = 12;
  ctx.beginPath(); ctx.arc(94, 262, 38, 0, Math.PI * 2); ctx.fillStyle = '#fff'; ctx.fill();
  ctx.restore();
  ctx.drawImage(IMG.key, 64, 232, 60, 60);
  ctx.save();
  ctx.shadowColor = 'rgba(0,0,0,.6)'; ctx.shadowBlur = 10;
  ctx.font = `700 34px ${FONT}`; ctx.textAlign = 'left';
  let x = 148;
  for (const [s, c] of [['kl', '#fff'], ['!', C.blue], ['ck', '#fff']]) { ctx.fillStyle = c; ctx.fillText(s, x, 274); x += ctx.measureText(s).width; }
  ctx.font = `500 30px ${FONT}`; ctx.fillStyle = 'rgba(255,255,255,.75)';
  ctx.fillText('  ·  VPN для Windows', x, 274);
  ctx.restore();
}

// ─────────── Голосовой чат ───────────

const MATES = [['ты', '#7c8cff'], ['dimon', '#f5a97f'], ['kolyan', '#8bd5ca'], ['s1mple_fan', '#f5bde6']];
function voiceCard(x, y, w, t, state, alpha = 1) {
  if (alpha <= 0) return;
  ctx.save();
  ctx.globalAlpha *= alpha;
  const h = 530;
  ctx.shadowColor = 'rgba(0,0,0,.55)'; ctx.shadowBlur = 50; ctx.shadowOffsetY = 20;
  rrect(x, y, w, h, 30); ctx.fillStyle = C.voice; ctx.fill();
  ctx.shadowColor = 'transparent';
  headset(x + 58, y + 62, 44, '#b5bac1');
  text('Discord · голосовой «катка»', x + 104, y + 76, { size: 34, weight: 700, color: '#f2f3f5', align: 'left' });
  const off = state === 'reconnect';
  MATES.forEach(([nick, color], i) => {
    const ry = y + 150 + i * 78;
    const speaking = !off && hash(Math.floor(t * 6) + i * 31) > 0.55;
    ctx.globalAlpha = alpha * (off ? 0.4 : 1);
    if (speaking) { ctx.beginPath(); ctx.arc(x + 62, ry, 32, 0, Math.PI * 2); ctx.strokeStyle = '#3ba55c'; ctx.lineWidth = 5; ctx.stroke(); }
    ctx.beginPath(); ctx.arc(x + 62, ry, 26, 0, Math.PI * 2); ctx.fillStyle = color; ctx.fill();
    text(nick[0].toUpperCase(), x + 62, ry + 11, { size: 30, weight: 800, color: '#1e1f22' });
    text(nick, x + 110, ry + 12, { size: 34, weight: 600, color: '#dbdee1', align: 'left' });
    ctx.globalAlpha = alpha;
  });
  rrect(x + 24, y + h - 84, w - 48, 60, 16);
  if (off) {
    ctx.fillStyle = 'rgba(237,66,69,.16)'; ctx.fill();
    text('Переподключение' + '.'.repeat(1 + (Math.floor(t * 3) % 3)), x + 50, y + h - 42, { size: 32, weight: 700, color: '#ed4245', align: 'left' });
  } else {
    ctx.fillStyle = 'rgba(59,165,92,.16)'; ctx.fill();
    text('Голосовая связь подключена', x + 50, y + h - 42, { size: 32, weight: 700, color: '#3ba55c', align: 'left' });
  }
  ctx.restore();
}

function bubble(x, y, str, t, at, size = 44) {
  const k = E.back(prog(t, at, at + 0.3));
  if (k <= 0) return;
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(k, k);
  ctx.font = `800 ${size}px ${FONT}`;
  const w = ctx.measureText(str).width + 70;
  rrect(-w / 2, -size - 10, w, size * 1.9, 26); ctx.fillStyle = '#fff'; ctx.fill();
  ctx.beginPath(); ctx.moveTo(-30, size * 0.85); ctx.lineTo(-6, size * 0.85); ctx.lineTo(-40, size * 1.5); ctx.closePath(); ctx.fill();
  text(str, 0, size * 0.28, { size, weight: 800, color: '#1c1c1e' });
  ctx.restore();
}

// ─────────── Сцены ───────────

function sceneHook(lt, t) {
  const lag = 1 - prog(lt, 2.6, 3.0) * 0.5;
  game(t, { lag, ping: 180 + 20 * Math.sin(t * 13) });
  const k = E.back(prog(lt, 0.15, 0.5));
  const flick = T.lag.some((x) => lt > x && lt < x + 0.08) ? 0.35 : 1;
  ctx.save();
  ctx.fillStyle = `rgba(6,8,12,${0.45 * prog(lt, 0.1, 0.4)})`; ctx.fillRect(0, 0, W, H);
  ctx.translate(540, 900); ctx.scale(k, k);
  text('PING', 0, -40, { size: 110, weight: 900, color: '#fff', alpha: flick });
  text('180', 0, 170, { size: 290, weight: 900, color: '#ff4d5e', alpha: flick });
  ctx.restore();
  text('Потому что CS2 шла через VPN\nв Германии', 540, 1230, { size: 50, weight: 700, alpha: E.out(prog(lt, 0.9, 1.3)) });
}

function sceneTrap(lt, t) {
  const off = t >= T.vpnOff;
  if (t < T.split) {
    // Сверху — игра с пингом, снизу — голосовой чат.
    ctx.save();
    rrect(60, 330, 960, 560, 34); ctx.clip();
    ctx.translate(60, 330); ctx.scale(960 / W, 560 / 1100); ctx.translate(0, -420);
    game(t, { lag: off ? 0 : 0.6, net: false });
    ctx.restore();
    chip(540, 330, off ? 'VPN выключен' : 'VPN включён', off ? C.dim : C.accent, { alpha: E.out(prog(lt, 0.05, 0.3)), bg: '#16181d' });
    const pingK = off ? E.back(prog(t, T.vpnOff + 0.1, T.vpnOff + 0.4)) : 1;
    chip(540, 800, off ? 'пинг 32' : 'пинг 180', off ? C.accent : C.red, { size: 44, bg: '#16181d', scale: pingK, alpha: E.out(prog(lt, 0.2, 0.45)) });
    voiceCard(90, 960, 900, t, off ? 'reconnect' : 'ok', E.out(prog(lt, 0.3, 0.6)));
    if (!off) {
      bubble(700, 1030, 'ты лагаешь!!', t, T.bubble1, 40);
      bubble(430, 1180, 'ТЫ ЛАГАЕШЬ', t, T.bubble2, 50);
    }
    return;
  }
  // Выбор без выбора: слева Discord, справа пинг.
  const k = E.out(prog(t, T.split, T.split + 0.35));
  title('Выбери одно', 440, t - T.split);
  const card = (x, icon, head, sub1, ok1, sub2, ok2) => {
    ctx.save();
    ctx.globalAlpha = k;
    rrect(x, 560, 440, 620, 36); ctx.fillStyle = '#1b1e25'; ctx.fill();
    ctx.strokeStyle = 'rgba(255,255,255,.08)'; ctx.lineWidth = 2; ctx.stroke();
    icon(x + 220, 700);
    text(head, x + 220, 840, { size: 50, weight: 800 });
    text((ok1 ? '✓ ' : '✕ ') + sub1, x + 220, 950, { size: 42, weight: 700, color: ok1 ? C.accent : C.red });
    text((ok2 ? '✓ ' : '✕ ') + sub2, x + 220, 1030, { size: 42, weight: 700, color: ok2 ? C.accent : C.red });
    ctx.restore();
  };
  card(70 - 60 * (1 - k), (x, y) => headset(x, y, 110, '#fff'), 'VPN вкл', 'Discord', true, 'пинг 180', false);
  card(570 + 60 * (1 - k), (x, y) => crosshairIcon(x, y, 110, '#fff'), 'VPN выкл', 'пинг 32', true, 'Discord', false);
  text('или', 540, 880, { size: 44, weight: 800, color: 'rgba(255,255,255,.6)', alpha: k });
  const n = E.back(prog(t, T.orNot, T.orNot + 0.3));
  if (n > 0) {
    ctx.save(); ctx.translate(540, 1330); ctx.scale(n, n);
    text('…или нет.', 0, 0, { size: 84, weight: 900, color: C.blue });
    ctx.restore();
  }
}

function sceneLogo(lt, t) {
  const drop = E.bounce(prog(t, T.keyLand - 0.55, T.keyLand + 0.35));
  const y = lerp(-420, 840, drop);
  const ring = prog(t, T.keyLand, T.keyLand + 0.9);
  if (ring > 0 && ring < 1) {
    for (let i = 0; i < 2; i++) {
      const q = clamp(ring * 1.3 - i * 0.25);
      ctx.beginPath(); ctx.arc(540, 840, lerp(200, 640, q), 0, Math.PI * 2);
      ctx.strokeStyle = hexA(C.blue, 0.6 * (1 - q)); ctx.lineWidth = 5; ctx.stroke();
    }
  }
  const g = ctx.createRadialGradient(540, 840, 0, 540, 840, 440);
  g.addColorStop(0, hexA(C.blue, 0.4 * prog(t, T.keyLand - 0.2, T.keyLand + 0.3))); g.addColorStop(1, hexA(C.blue, 0));
  ctx.fillStyle = g; ctx.fillRect(0, 380, W, 940);
  const squash = t > T.keyLand && t < T.keyLand + 0.12 ? 0.92 : 1;
  ctx.save();
  ctx.translate(540, y); ctx.scale(1 / squash, squash);
  ctx.shadowColor = 'rgba(0,40,120,.6)'; ctx.shadowBlur = 60; ctx.shadowOffsetY = 30;
  ctx.drawImage(IMG.key, -230, -230, 460, 460);
  ctx.restore();
  const shown = T.letters.filter((x) => t >= x).length;
  if (shown) parts(LOGO, 540, 1240, 180, 800, 1, shown);
  text('VPN, который понимает геймеров', 540, 1350, { size: 44, weight: 600, color: 'rgba(255,255,255,.78)', alpha: E.out(prog(lt, 1.2, 1.5)) });
}

function sceneRules(lt, t) {
  title('Каждой программе —\nсвой маршрут', 400, lt, 0, { size: 64 });
  const inK = E.out(prog(t, T.winIn, T.winIn + 0.45));
  const x = 80, y = 560 + 100 * (1 - inK), w = 470;
  const s = w / 380;
  const swapK = prog(t, T.swap, T.swap + 0.3);
  if (swapK < 1) {
    framed(IMG.picker, x, y, w, { alpha: inK * (1 - swapK) });
    // До нажатия флажок у CS2 пуст: закрываем готовую галочку.
    if (t < T.tapCs2) {
      ctx.save(); ctx.globalAlpha = inK;
      rrect(x + 16 * s, y + 305 * s, 21 * s, 21 * s, 6 * s); ctx.fillStyle = '#1f1f23'; ctx.fill();
      ctx.strokeStyle = '#4a4a50'; ctx.lineWidth = 3; ctx.stroke();
      ctx.restore();
    }
    tap(x + 190 * s, y + 315 * s, t, T.tapCs2);
    tap(x + 236 * s, y + 682 * s, t, T.tapAdd);
  }
  if (swapK > 0) {
    framed(IMG.rules, x, y, w, { alpha: swapK });
    const hi = (cy, cx, at, color) => {
      const k = E.out(prog(t, at, at + 0.3));
      if (k <= 0) return;
      ctx.save(); ctx.globalAlpha = k;
      rrect(x + (cx - 58) * s, y + (cy - 18) * s, 116 * s, 36 * s, 10 * s);
      ctx.strokeStyle = color; ctx.lineWidth = 6; ctx.shadowColor = color; ctx.shadowBlur = 24; ctx.stroke();
      ctx.restore();
    };
    hi(478, 190, T.hiCs2, C.accent);
    hi(592, 75, T.hiDiscord, C.blue);
  }
  const side = (cy, k, head, sub, color) => {
    if (k <= 0) return;
    ctx.save(); ctx.translate(800, cy); ctx.scale(k, k);
    rrect(-230, -90, 460, 180, 30); ctx.fillStyle = '#171a21'; ctx.fill();
    ctx.strokeStyle = hexA(color, 0.6); ctx.lineWidth = 3; ctx.stroke();
    text(head, 0, -12, { size: 44, weight: 800 });
    text(sub, 0, 50, { size: 36, weight: 700, color });
    ctx.restore();
  };
  side(760, E.back(prog(t, T.hiCs2, T.hiCs2 + 0.35)), 'CS2', 'напрямую', C.accent);
  side(980, E.back(prog(t, T.hiDiscord, T.hiDiscord + 0.35)), 'Discord', 'через VPN', C.blue);
  // Пинг падает 180 → 32.
  const pk = prog(t, T.pingFrom, T.pingTo);
  const ping = Math.round(lerp(180, 32, E.inOut(pk)));
  const pa = E.out(prog(t, T.pingFrom - 0.2, T.pingFrom + 0.1));
  if (pa > 0) {
    ctx.save(); ctx.globalAlpha = pa;
    rrect(570, 1120, 460, 250, 34); ctx.fillStyle = '#12151b'; ctx.fill();
    ctx.strokeStyle = hexA(pk >= 1 ? C.accent : C.red, 0.7); ctx.lineWidth = 4; ctx.stroke();
    text('ПИНГ В CS2', 800, 1180, { size: 32, weight: 700, color: C.dim });
    text(String(ping), 800, 1320, { size: 130, weight: 900, color: ping > 100 ? '#ff4d5e' : ping > 60 ? '#ffd166' : C.accent });
    ctx.restore();
  }
  text('Discord — работает ✓', 540, 1500, { size: 46, weight: 800, color: C.accent, alpha: E.out(prog(t, T.success, T.success + 0.3)) });
}

function flow(pts, color, t, speed, width = 8, alpha = 1) {
  if (alpha <= 0) return;
  const [a, b, c, d] = pts;
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.beginPath(); ctx.moveTo(...a); ctx.bezierCurveTo(...b, ...c, ...d);
  ctx.strokeStyle = hexA(color, 0.18); ctx.lineWidth = width + 10; ctx.lineCap = 'round'; ctx.stroke();
  ctx.setLineDash([22, 26]); ctx.lineDashOffset = -t * speed;
  ctx.strokeStyle = color; ctx.lineWidth = width; ctx.stroke();
  ctx.setLineDash([]);
  for (let i = 0; i < 3; i++) {
    const u = (t * 0.5 + i / 3) % 1, m = 1 - u;
    const px = m * m * m * a[0] + 3 * m * m * u * b[0] + 3 * m * u * u * c[0] + u * u * u * d[0];
    const py = m * m * m * a[1] + 3 * m * m * u * b[1] + 3 * m * u * u * c[1] + u * u * u * d[1];
    ctx.beginPath(); ctx.arc(px, py, 12, 0, Math.PI * 2); ctx.fillStyle = color; ctx.fill();
  }
  ctx.restore();
}
function tile(cx, cy, size, bg, draw, alpha = 1) {
  if (alpha <= 0) return;
  ctx.save(); ctx.globalAlpha *= alpha;
  rrect(cx - size / 2, cy - size / 2, size, size, size * 0.26); ctx.fillStyle = bg; ctx.fill();
  draw();
  ctx.restore();
}

function sceneViz(lt, t) {
  title('Не разбираешься?', 400, lt);
  title('Покажем схемой', 490, lt, 0.15, { color: C.blue });
  const pc = [170, 1000], top = [880, 780], bot = [880, 1220];
  const a1 = E.out(prog(t, T.vizNode1, T.vizNode1 + 0.4)), a2 = E.out(prog(t, T.vizNode2, T.vizNode2 + 0.4));
  flow([pc, [360, 1000], [380, 780], [top[0] - 90, top[1]]], C.accent, t, 150, 8, a1);
  flow([pc, [340, 1000], [360, 1220], [540, 1220]], C.blue, t, 150, 8, a2);
  flow([[540, 1220], [690, 1220], [700, 1220], [bot[0] - 90, bot[1]]], C.blue, t, 150, 8, a2);
  tile(pc[0], pc[1], 170, '#23262e', () => monitor(pc[0], pc[1] - 8, 88, '#fff'), E.out(prog(lt, 0.2, 0.5)));
  text('твой ПК', pc[0], pc[1] + 140, { size: 34, weight: 600, color: 'rgba(255,255,255,.75)', alpha: E.out(prog(lt, 0.2, 0.5)) });
  chip(560, 880, 'напрямую', C.accent, { alpha: a1, bg: '#11261c', size: 30 });
  chip(540, 1220, 'VPN', C.blue, { alpha: a2, bg: '#0d2a55', size: 34 });
  tile(top[0], top[1], 150, hexA(C.accent, 0.22), () => crosshairIcon(top[0], top[1], 80, '#fff'), a1);
  text('сервер CS2', top[0] - 10, top[1] + 130, { size: 32, weight: 700, color: 'rgba(255,255,255,.85)', alpha: a1 });
  tile(bot[0], bot[1], 150, hexA(C.blue, 0.22), () => headset(bot[0], bot[1], 80, '#fff'), a2);
  text('Discord', bot[0] - 10, bot[1] + 130, { size: 32, weight: 700, color: 'rgba(255,255,255,.85)', alpha: a2 });
  text('Кнопка «ⓘ» в приложении —\nтакая схема для каждого режима', 540, 1470, { size: 38, weight: 600, color: 'rgba(255,255,255,.75)', alpha: E.out(prog(lt, 1.6, 2.0)) });
}

function sceneTray(lt, t) {
  game(t, { lag: 0, net: false, dim: 0.35 });
  title('Сервер подлагивает?', 470, lt, 0, { size: 66 });
  title('Меняй прямо в игре', 550, lt, 0.2, { size: 66, color: C.accent });
  const k = E.out(prog(t, T.trayIn, T.trayIn + 0.45));
  const w = 560, s = w / TRAY_CROP[2];
  const x = 480, y = lerp(1920, 630, k);
  framed(t >= T.tapServer ? IMG.trayAfter : IMG.trayBefore, x, y, w, { crop: TRAY_CROP, radius: 36, alpha: k });
  // Точки на карточке меню (2x-скриншот): кнопка «Задержка» и строка Amsterdam.
  const bx = (px) => x + (px - TRAY_CROP[0]) * s, by = (py) => y + (py - TRAY_CROP[1]) * s;
  tap(bx(536), by(486), t, T.tapPing);
  // Пока меряем — многоточие вместо чисел.
  if (t > T.tapPing && t < T.pingsDone) {
    for (let i = 0; i < 4; i++) {
      rrect(bx(560), by(546 + i * 68), 100 * s, 44 * s, 8); ctx.fillStyle = '#232327'; ctx.fill();
      text('…', bx(610), by(582 + i * 68), { size: 30, weight: 700, color: C.dim });
    }
  }
  tap(bx(300), by(564), t, T.tapServer);
  text('без alt-tab', 245, 1010, { size: 52, weight: 800, alpha: E.out(prog(t, T.tapServer + 0.3, T.tapServer + 0.6)) });
  text('96 → 31 мс', 245, 1090, { size: 46, weight: 800, color: C.accent, alpha: E.out(prog(t, T.tapServer + 0.4, T.tapServer + 0.7)) });
}

function sceneKill(lt, t) {
  const dropped = t >= T.vpnDrop;
  text('02:10', 540, 470, { size: 150, weight: 800, color: 'rgba(255,255,255,.9)', alpha: E.out(prog(lt, 0, 0.4)) });
  text('ты спишь, качается обновление', 540, 560, { size: 42, weight: 600, color: 'rgba(255,255,255,.65)', alpha: E.out(prog(lt, 0.3, 0.6)) });
  const shake = dropped && t < T.vpnDrop + 0.4 ? Math.sin(t * 80) * 12 * (1 - prog(t, T.vpnDrop, T.vpnDrop + 0.4)) : 0;
  chip(540 + shake, 690, dropped ? 'VPN оборвался' : 'VPN подключён', dropped ? C.red : C.accent, { alpha: E.out(prog(lt, 0.4, 0.7)), bg: '#16181d' });
  const k = E.out(prog(lt, 0.5, 0.9));
  const x = 90, y = 820, w = 900;
  ctx.save(); ctx.globalAlpha = k;
  rrect(x, y, w, 330, 30); ctx.fillStyle = '#1b1e25'; ctx.fill();
  ctx.strokeStyle = 'rgba(255,255,255,.08)'; ctx.lineWidth = 2; ctx.stroke();
  rrect(x + 36, y + 40, 90, 90, 22); ctx.fillStyle = '#a8c7fa'; ctx.fill();
  text('q', x + 81, y + 104, { size: 60, weight: 800, color: '#1a1a1d' });
  text('торрент-клиент', x + 150, y + 78, { size: 38, weight: 700, align: 'left' });
  text('карты из мастерской + моды · 4,2 ГБ', x + 150, y + 124, { size: 32, weight: 500, color: C.dim, align: 'left' });
  const frozen = t >= T.lockOn;
  const pr = frozen ? 0.47 : lerp(0.31, 0.47, prog(t, SCENES[6].from, T.lockOn));
  rrect(x + 36, y + 190, w - 72, 22, 11); ctx.fillStyle = '#2c2f37'; ctx.fill();
  rrect(x + 36, y + 190, (w - 72) * pr, 22, 11); ctx.fillStyle = frozen ? C.dim : C.accent; ctx.fill();
  text(`${Math.round(pr * 100)}%`, x + 36, y + 272, { size: 36, weight: 700, align: 'left' });
  text(frozen ? '↓ 0 Б/с — нет сети' : dropped ? '↓ 8,4 МБ/с — напрямую?!' : '↓ 8,4 МБ/с через VPN', x + w - 36, y + 272, { size: 34, weight: 700, color: frozen ? C.dim : dropped ? C.red : C.accent, align: 'right' });
  ctx.restore();
  const l = E.back(prog(t, T.lockOn, T.lockOn + 0.35));
  if (l > 0) {
    ctx.save(); ctx.translate(x + w - 40, y + 30); ctx.scale(l, l);
    ctx.beginPath(); ctx.arc(0, 0, 56, 0, Math.PI * 2); ctx.fillStyle = C.red; ctx.fill();
    lock(0, 6, 60, '#fff');
    ctx.restore();
  }
  const sk = E.out(prog(t, T.lockOn + 0.2, T.lockOn + 0.6));
  if (sk > 0) {
    shield(540, 1290, 170, C.green, prog(t, T.lockOn + 0.3, T.lockOn + 0.8));
    text('Kill Switch', 540, 1450, { size: 56, weight: 900, color: C.green, alpha: sk });
    text('ни байта мимо VPN', 540, 1520, { size: 44, weight: 700, alpha: E.out(prog(t, T.lockOn + 0.6, T.lockOn + 1.0)) });
  }
}

const PROTOS = ['VLESS', 'Reality', 'Hysteria2', 'TUIC', 'Trojan', 'Shadowsocks', 'VMess', 'AnyTLS', 'gRPC', 'XHTTP', 'WebSocket'];
const LINK = 'https://panel.твой-vpn.com/sub/k7f2…';

function sceneSetup(lt, t) {
  // Бегущие строки протоколов на фоне.
  for (let row = 0; row < 3; row++) {
    const y = 1180 + row * 110, dir = row % 2 ? 1 : -1;
    let x = ((t * 120 * dir) % 1400) - 200 * row;
    ctx.save(); ctx.globalAlpha = 0.16 * E.out(prog(lt, 0, 0.4));
    ctx.font = `800 64px ${FONT}`; ctx.fillStyle = '#fff'; ctx.textAlign = 'left';
    for (let i = 0; i < 16; i++) {
      const word = PROTOS[(i + row * 3) % PROTOS.length] + '  ·  ';
      ctx.fillText(word, x - 1400, y); x += ctx.measureText(word).width;
    }
    ctx.restore();
  }
  title('Свой VPN?', 420, lt, 0, { size: 72 });
  title('Вставь ссылку — и всё', 510, lt, 0.15, { size: 64, color: C.blue });
  const k = E.out(prog(lt, 0.2, 0.5));
  ctx.save(); ctx.globalAlpha = k;
  rrect(90, 620, 900, 120, 26); ctx.fillStyle = '#1b1e25'; ctx.fill();
  ctx.strokeStyle = t >= T.typeTo ? hexA(C.accent, 0.8) : 'rgba(255,255,255,.14)'; ctx.lineWidth = 3; ctx.stroke();
  const n = Math.floor(LINK.length * prog(t, T.typeFrom, T.typeTo));
  ctx.font = `600 38px ui-monospace, Consolas, monospace`; ctx.textAlign = 'left'; ctx.fillStyle = '#e6e6ea';
  ctx.fillText(LINK.slice(0, n) + (Math.floor(t * 2) % 2 && t < T.tapPaste ? '|' : ''), 126, 694);
  ctx.restore();
  const servers = [['Нидерланды · Amsterdam', 31], ['Финляндия · Helsinki', 44], ['Германия · Frankfurt', 96], ['США · New York', 142]];
  servers.forEach(([name, ms], i) => {
    const a = E.back(prog(t, T.rows[i], T.rows[i] + 0.25));
    if (a <= 0) return;
    ctx.save(); ctx.translate(540, 820 + i * 100); ctx.scale(1, a);
    rrect(-450, -40, 900, 84, 20); ctx.fillStyle = '#171a21'; ctx.fill();
    text(name, -410, 14, { size: 36, weight: 600, align: 'left' });
    text(`${ms} мс`, 410, 14, { size: 36, weight: 700, color: ms < 80 ? C.accent : ms < 120 ? '#fff' : C.orange, align: 'right' });
    ctx.restore();
  });
  tap(540, 680, t, T.tapPaste);
}

function sceneCta(lt, t) {
  // Четыре темы веером.
  const themes = [[IMG.thLight, -1.5], [IMG.thMid, -0.5], [IMG.thOled, 0.5], [IMG.thGraph, 1.5]];
  const gone = prog(t, T.finalKey - 0.25, T.finalKey + 0.1);
  if (gone < 1) {
    themes.forEach(([img, pos], i) => {
      const k = E.back(prog(t, T.themes[i], T.themes[i] + 0.3));
      if (k <= 0) return;
      ctx.save();
      ctx.globalAlpha = 1 - gone;
      ctx.translate(540 + pos * 190, 960 + Math.abs(pos) * 40);
      ctx.rotate(pos * 0.1);
      ctx.scale(k * (1 - 0.3 * gone), k * (1 - 0.3 * gone));
      framed(img, -170, -320, 340, { crop: DOCS_CROP, radius: 24 });
      ctx.restore();
    });
    text('4 темы · 6 акцентов', 540, 430, { size: 56, weight: 800, alpha: E.out(prog(t, T.themes[0], T.themes[0] + 0.3)) * (1 - gone) });
  }
  if (t < T.finalKey - 0.25) return;
  const a = E.out(prog(t, T.finalKey - 0.2, T.finalKey + 0.2));
  const fy = 700 + Math.sin(t * 2) * 10;
  const g = ctx.createRadialGradient(540, fy, 0, 540, fy, 380);
  g.addColorStop(0, hexA(C.blue, 0.38 * a)); g.addColorStop(1, hexA(C.blue, 0));
  ctx.fillStyle = g; ctx.fillRect(0, 300, W, 900);
  const s = 320 * (0.7 + 0.3 * E.back(prog(t, T.finalKey - 0.2, T.finalKey + 0.25)));
  ctx.save(); ctx.globalAlpha = a;
  ctx.drawImage(IMG.key, 540 - s / 2, fy - s / 2, s, s);
  ctx.restore();
  parts(LOGO, 540, 1030, 150, 800, E.out(prog(t, T.finalKey, T.finalKey + 0.4)));
  text('Бесплатно · Windows 10/11 · открытый код', 540, 1120, { size: 38, weight: 600, color: 'rgba(255,255,255,.78)', alpha: E.out(prog(t, T.finalKey + 0.2, T.finalKey + 0.6)) });
  const b = E.back(prog(t, T.finalKey + 0.5, T.finalKey + 0.9));
  if (b > 0) {
    ctx.save(); ctx.translate(540, 1260); ctx.scale(b, b);
    rrect(-340, -58, 680, 116, 58); ctx.fillStyle = C.blue; ctx.fill();
    text('github.com/vbu00/klick', 0, 15, { size: 44, weight: 700, color: '#fff' });
    ctx.restore();
  }
  text('ссылка в профиле ↓', 540, 1420 + Math.sin(t * 6) * 8, { size: 40, weight: 600, color: 'rgba(255,255,255,.72)', alpha: E.out(prog(t, T.finalKey + 1.0, T.finalKey + 1.4)) });
}

const DRAW = { hook: sceneHook, trap: sceneTrap, logo: sceneLogo, rules: sceneRules, viz: sceneViz, tray: sceneTray, kill: sceneKill, setup: sceneSetup, cta: sceneCta };

function draw(t) {
  t = clamp(t, 0, DUR - 1e-6);
  background(t);
  const s = SCENES.find((x) => t >= x.from && t < x.to);
  const out = s === SCENES[SCENES.length - 1] ? 0 : prog(t, s.to - 0.22, s.to);
  ctx.save();
  ctx.globalAlpha = 1 - out;
  ctx.translate(0, -40 * E.inOut(out));
  DRAW[s.id](t - s.from, t);
  ctx.restore();
  // Вспышка на ударе клавиши в логотипе.
  if (t >= T.keyLand) {
    const flash = 1 - prog(t, T.keyLand, T.keyLand + 0.25);
    if (flash > 0) { ctx.fillStyle = `rgba(255,255,255,${0.35 * flash})`; ctx.fillRect(0, 0, W, H); }
  }
  storyChrome(t);
}

// ─────────── Звук: музыка и саунд-дизайн ───────────
//
// Всё синтезируется в OfflineAudioContext — без сэмплов и чужих треков,
// одинаково при каждом рендере. Бит 120 BPM, сетка совпадает с ударом клавиши
// в логотипе: до него — глухой напряжённый пульс, после — полный бит и
// аккорды Am–F–C–G. Звуки интерфейса привязаны к моментам из T.

async function buildAudio() {
  const ac = new OfflineAudioContext(2, Math.ceil(DUR * RATE), RATE);
  const master = ac.createGain(); master.gain.value = 0.9;
  const comp = ac.createDynamicsCompressor();
  comp.threshold.value = -14; comp.ratio.value = 4; comp.attack.value = 0.003; comp.release.value = 0.2;
  master.connect(comp); comp.connect(ac.destination);
  const music = ac.createGain();
  const mf = ac.createBiquadFilter(); mf.type = 'lowpass'; mf.Q.value = 0.8;
  music.connect(mf); mf.connect(master);
  const sfx = ac.createGain(); sfx.gain.value = 0.9; sfx.connect(master);

  // Громкость и «глухость» музыки по сюжету.
  const F = mf.frequency, G = music.gain;
  F.setValueAtTime(700, 0); F.setValueAtTime(700, 9.2); F.exponentialRampToValueAtTime(260, 10.0);
  F.setValueAtTime(18000, T.keyLand);
  F.setValueAtTime(18000, T.vpnDrop - 0.01); F.exponentialRampToValueAtTime(350, T.vpnDrop + 0.08);
  F.setValueAtTime(350, T.lockOn); F.exponentialRampToValueAtTime(18000, T.lockOn + 0.6);
  G.setValueAtTime(0.5, 0); G.setValueAtTime(0.5, 9.3); G.linearRampToValueAtTime(0.08, 9.9);
  G.setValueAtTime(0.62, T.keyLand);
  G.setValueAtTime(0.62, DUR - 1.6); G.linearRampToValueAtTime(0.0001, DUR - 0.05);

  const R = rng(42);
  const noiseBuf = ac.createBuffer(1, RATE * 2, RATE);
  { const d = noiseBuf.getChannelData(0); for (let i = 0; i < d.length; i++) d[i] = R() * 2 - 1; }
  const noise = (t, dur, dest, filt, env) => {
    const src = ac.createBufferSource(); src.buffer = noiseBuf; src.loop = true;
    const f = ac.createBiquadFilter(); f.type = filt.type; f.frequency.value = filt.f; f.Q.value = filt.q ?? 1;
    const g = ac.createGain(); env(g.gain, t);
    src.connect(f); f.connect(g); g.connect(dest);
    src.start(t, (t * 0.37) % 1.5); src.stop(t + dur);
    return f;
  };
  const tone = (t, dur, dest, o) => {
    const osc = ac.createOscillator(); osc.type = o.type || 'sine';
    osc.frequency.setValueAtTime(o.f, t);
    if (o.f2) osc.frequency.exponentialRampToValueAtTime(o.f2, t + (o.glide ?? dur));
    if (o.detune) osc.detune.value = o.detune;
    const g = ac.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(o.v ?? 0.3, t + (o.a ?? 0.005));
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    let node = osc;
    if (o.lp) { const f = ac.createBiquadFilter(); f.type = 'lowpass'; f.frequency.value = o.lp; osc.connect(f); node = f; }
    node.connect(g); g.connect(dest);
    osc.start(t); osc.stop(t + dur + 0.02);
  };
  const perc = (v, dec) => (g, t) => { g.setValueAtTime(0.0001, t); g.exponentialRampToValueAtTime(v, t + 0.002); g.exponentialRampToValueAtTime(0.0001, t + dec); };

  // Инструменты.
  const kick = (t, v = 0.9) => tone(t, 0.32, music, { f: 150, f2: 42, glide: 0.12, v });
  const hat = (t, v = 0.12) => noise(t, 0.06, music, { type: 'highpass', f: 8000 }, perc(v, 0.05));
  const clap = (t, v = 0.35) => { noise(t, 0.2, music, { type: 'bandpass', f: 1500, q: 0.9 }, perc(v, 0.16)); noise(t + 0.012, 0.15, music, { type: 'bandpass', f: 1200, q: 0.9 }, perc(v * 0.7, 0.12)); };
  const bass = (t, f, dur) => tone(t, dur, music, { f, type: 'sawtooth', v: 0.22, lp: 380, a: 0.01 });
  const pad = (t, notes, dur, v = 0.05) => notes.forEach((f, i) => { tone(t, dur, music, { f, type: 'sawtooth', v, lp: 1600, a: 0.25, detune: (i - 1) * 7 }); tone(t, dur, music, { f: f * 1.003, type: 'sawtooth', v: v * 0.7, lp: 1600, a: 0.25 }); });
  const bell = (t, f, v = 0.12) => { tone(t, 0.9, music, { f, v, a: 0.004 }); tone(t, 0.6, music, { f: f * 2, v: v * 0.35, a: 0.004 }); };
  const hz = (m) => 440 * Math.pow(2, (m - 69) / 12);

  const BEAT = 0.5, ORIGIN = T.keyLand;
  // До дропа: глухой пульс и тревожный бас.
  for (let t = ORIGIN - 20 * BEAT; t < 9.3; t += BEAT) {
    kick(t, 0.55);
    if (t >= 3.2) hat(t + BEAT / 2, 0.08);
    if (Math.round((t - ORIGIN) / BEAT) % 4 === 0) bass(t, hz(33), 1.9);
  }
  // Дроп и основная часть: Am F C G.
  const CHORDS = [[57, [57, 60, 64]], [53, [53, 57, 60]], [48, [60, 64, 67]], [55, [55, 59, 62]]];
  const MEL = [76, 72, 74, 71];
  let bar = 0;
  for (let t = ORIGIN; t < DUR - 1.2; t += BEAT * 4, bar++) {
    const [root, chord] = CHORDS[bar % 4];
    pad(t, chord.map(hz), BEAT * 4 + 0.1);
    for (let b = 0; b < 4; b++) {
      const tb = t + b * BEAT;
      if (tb >= DUR - 1.2) break;
      const night = tb > T.vpnDrop && tb < T.lockOn;
      kick(tb, night ? 0.4 : 0.85);
      if (b % 2 === 1) clap(tb, 0.3);
      hat(tb + BEAT / 2, 0.1); if (bar % 2) hat(tb + BEAT / 4, 0.05);
      bass(tb, hz(root - 24), BEAT * 0.9);
    }
    if (bar % 2 === 1) MEL.forEach((m, i) => bell(t + i * BEAT, hz(m), 0.05));
  }
  // Финальный аккорд.
  pad(T.finalKey, [60, 64, 67, 72].map(hz), 2.8, 0.07);
  bell(T.finalKey, hz(84), 0.14); bell(T.finalKey + 0.25, hz(79), 0.1); bell(T.finalKey + 0.5, hz(88), 0.08);

  // Звуки.
  const click = (t, v = 0.4) => { noise(t, 0.03, sfx, { type: 'bandpass', f: 3200, q: 2 }, perc(v, 0.025)); tone(t, 0.04, sfx, { f: 2200, f2: 1400, v: v * 0.4 }); };
  // Фирменный звук: механическая клавиша — щелчок и низкий «тук».
  const keyThock = (t, v = 0.9) => {
    noise(t, 0.04, sfx, { type: 'bandpass', f: 2600, q: 1.4 }, perc(v * 0.7, 0.03));
    tone(t, 0.12, sfx, { f: 210, f2: 95, v, glide: 0.08 });
    tone(t + 0.005, 0.08, sfx, { f: 520, f2: 380, v: v * 0.35 });
  };
  const pop = (t, v = 0.3, f = 700) => tone(t, 0.12, sfx, { f, f2: f * 1.9, v, glide: 0.07 });
  const whoosh = (t, dur = 0.45, v = 0.22) => {
    const f = noise(t, dur, sfx, { type: 'bandpass', f: 400, q: 1.2 }, (g, t0) => { g.setValueAtTime(0.0001, t0); g.exponentialRampToValueAtTime(v, t0 + dur * 0.6); g.exponentialRampToValueAtTime(0.0001, t0 + dur); });
    f.frequency.setValueAtTime(350, t); f.frequency.exponentialRampToValueAtTime(3800, t + dur);
  };
  const riser = (t, dur, v = 0.2) => {
    const f = noise(t, dur, sfx, { type: 'bandpass', f: 300, q: 2 }, (g, t0) => { g.setValueAtTime(0.0001, t0); g.exponentialRampToValueAtTime(v, t0 + dur); });
    f.frequency.setValueAtTime(250, t); f.frequency.exponentialRampToValueAtTime(6000, t + dur);
    tone(t, dur, sfx, { f: 110, f2: 880, v: 0.06, a: dur * 0.9, glide: dur, type: 'sawtooth', lp: 2000 });
  };
  const buzz = (t, dur = 0.4, v = 0.2) => { tone(t, dur, sfx, { f: 98, type: 'square', v, lp: 900 }); tone(t, dur, sfx, { f: 103.5, type: 'square', v: v * 0.8, lp: 900 }); };
  const glitch = (t, v = 0.25) => { for (let i = 0; i < 5; i++) noise(t + i * 0.035, 0.03, sfx, { type: i % 2 ? 'highpass' : 'bandpass', f: 1500 + i * 900, q: 3 }, perc(v, 0.025)); tone(t, 0.12, sfx, { f: 60, type: 'square', v: v * 0.6, lp: 300 }); };
  const chime = (t, v = 0.2) => [[1318.5, 0], [1760, 0.07], [2637, 0.14]].forEach(([f, d]) => tone(t + d, 0.7, sfx, { f, v, a: 0.003 }));
  const clank = (t, v = 0.35) => { [520, 1370, 2410, 3900].forEach((f, i) => tone(t, 0.5 - i * 0.08, sfx, { f, v: v / (i + 1), a: 0.002 })); noise(t, 0.05, sfx, { type: 'highpass', f: 3000 }, perc(v * 0.8, 0.04)); tone(t, 0.2, sfx, { f: 140, f2: 70, v: v * 0.9 }); };
  const downSweep = (t, v = 0.2) => tone(t, 0.45, sfx, { f: 880, f2: 180, v, glide: 0.4, type: 'triangle' });

  // Крючок: лаги и гул ошибки.
  buzz(0.2, 0.5, 0.14);
  T.lag.forEach((x) => glitch(x));
  // Ловушка.
  click(T.vpnOnChip); pop(T.bubble1, 0.35, 900); pop(T.bubble2, 0.4, 700); glitch(T.bubble2 + 0.3, 0.15);
  click(T.vpnOff, 0.5); downSweep(T.vpnOff + 0.08);
  whoosh(T.split - 0.2); riser(T.orNot - 0.1, 0.9);
  // Логотип: тишина → удар клавиши → буквы.
  keyThock(T.keyLand, 1.0); tone(T.keyLand, 0.9, sfx, { f: 55, f2: 38, v: 0.7, glide: 0.6 });
  noise(T.keyLand, 1.2, sfx, { type: 'highpass', f: 6000 }, (g, t) => { g.setValueAtTime(0.0001, t); g.exponentialRampToValueAtTime(0.12, t + 0.01); g.exponentialRampToValueAtTime(0.0001, t + 1.1); });
  T.letters.forEach((x, i) => keyThock(x, 0.28 + i * 0.03));
  // Правила.
  whoosh(T.winIn - 0.1); click(T.tapCs2); click(T.tapAdd); whoosh(T.swap - 0.05, 0.35, 0.15);
  pop(T.hiCs2, 0.3, 800); pop(T.hiDiscord, 0.3, 1000);
  for (let i = 0; i < 14; i++) tone(lerp(T.pingFrom, T.pingTo, E.inOut(i / 13)), 0.05, sfx, { f: lerp(1800, 700, i / 13), v: 0.12 });
  chime(T.success);
  // Схема.
  whoosh(T.vizIn - 0.15); pop(T.vizNode1, 0.25, 600); pop(T.vizNode2, 0.25, 800);
  // Трей.
  whoosh(SCENES[5].from - 0.1); whoosh(T.trayIn, 0.5, 0.2); click(T.tapPing);
  for (let i = 0; i < 6; i++) tone(T.tapPing + 0.12 + i * 0.12, 0.05, sfx, { f: 1500 + (i % 2) * 300, v: 0.08 });
  click(T.tapServer); chime(T.tapServer + 0.1, 0.14);
  // Kill Switch.
  whoosh(T.nightIn - 0.15);
  buzz(T.vpnDrop, 0.45, 0.22); glitch(T.vpnDrop + 0.05, 0.2);
  clank(T.lockOn); chime(T.lockOn + 0.35, 0.16);
  // Ссылка: набор и серверы.
  whoosh(SCENES[7].from - 0.1);
  for (let i = 0; i < 18; i++) keyThock(lerp(T.typeFrom, T.typeTo, i / 17) + (hash(i) - 0.5) * 0.02, 0.16);
  click(T.tapPaste, 0.5);
  T.rows.forEach((x, i) => pop(x, 0.2, 600 + i * 150));
  // Финал: темы веером и удар клавиши.
  whoosh(SCENES[8].from - 0.1);
  T.themes.forEach((x, i) => { whoosh(x - 0.05, 0.25, 0.14); pop(x + 0.05, 0.12, 900 + i * 120); });
  riser(T.finalKey - 0.6, 0.6, 0.14);
  keyThock(T.finalKey, 1.0); tone(T.finalKey, 1.2, sfx, { f: 55, f2: 40, v: 0.55, glide: 0.8 });

  const buf = await ac.startRendering();
  // Мастеринг: громкость как у коротких роликов — средний уровень основной
  // части около −14 дБ, пики мягко скругляет tanh (не выше −0,5 дБ, без
  // цифрового хрипа). Без этого ролик звучал бы тише соседних в ленте.
  const TARGET_DB = -14, CEIL = 0.94;
  let sum = 0, n = 0;
  for (let c = 0; c < 2; c++) { const d = buf.getChannelData(c); for (let i = Math.floor(T.keyLand * RATE); i < Math.floor((DUR - 2) * RATE); i++) { sum += d[i] * d[i]; n++; } }
  const rmsDb = 20 * Math.log10(Math.sqrt(sum / n) + 1e-9);
  const gain = Math.pow(10, (TARGET_DB - rmsDb) / 20);
  for (let c = 0; c < 2; c++) { const d = buf.getChannelData(c); for (let i = 0; i < d.length; i++) d[i] = CEIL * Math.tanh((d[i] * gain) / CEIL); }
  return buf;
}

// ─────────── Запись в MP4 (render.mjs) и просмотр ───────────

async function encodeStory(onProgress) {
  const audio = await buildAudio();
  const aac = { codec: 'mp4a.40.2', sampleRate: RATE, numberOfChannels: 2, bitrate: 192000 };
  const opus = { codec: 'opus', sampleRate: RATE, numberOfChannels: 2, bitrate: 160000 };
  const useAac = (await AudioEncoder.isConfigSupported(aac)).supported;
  if (!useAac && !(await AudioEncoder.isConfigSupported(opus)).supported) throw new Error('браузер не кодирует ни AAC, ни Opus');
  const muxer = new Mp4Muxer.Muxer({
    target: new Mp4Muxer.ArrayBufferTarget(),
    video: { codec: 'avc', width: W, height: H, frameRate: FPS },
    audio: { codec: useAac ? 'aac' : 'opus', numberOfChannels: 2, sampleRate: RATE },
    fastStart: 'in-memory',
  });
  let failure = null;
  const venc = new VideoEncoder({ output: (c, m) => muxer.addVideoChunk(c, m), error: (e) => { failure = e; } });
  const vcfg = { codec: 'avc1.640028', width: W, height: H, bitrate: 12_000_000, framerate: FPS, avc: { format: 'avc' } };
  if (!(await VideoEncoder.isConfigSupported(vcfg)).supported) throw new Error('H.264 1080×1920 не поддерживается');
  venc.configure(vcfg);
  const aenc = new AudioEncoder({ output: (c, m) => muxer.addAudioChunk(c, m), error: (e) => { failure = e; } });
  aenc.configure(useAac ? aac : opus);

  // Звук — кусками по 1024 отсчёта.
  const L = audio.getChannelData(0), Rc = audio.getChannelData(1);
  for (let i = 0; i < L.length; i += 1024) {
    const n = Math.min(1024, L.length - i);
    const planar = new Float32Array(n * 2);
    planar.set(L.subarray(i, i + n), 0); planar.set(Rc.subarray(i, i + n), n);
    const ad = new AudioData({ format: 'f32-planar', sampleRate: RATE, numberOfFrames: n, numberOfChannels: 2, timestamp: Math.round((i / RATE) * 1e6), data: planar });
    aenc.encode(ad); ad.close();
  }
  await aenc.flush();

  const frames = Math.round(DUR * FPS);
  for (let i = 0; i < frames; i++) {
    if (failure) throw failure;
    draw(i / FPS);
    const f = new VideoFrame(cv, { timestamp: Math.round((i * 1e6) / FPS), duration: Math.round(1e6 / FPS) });
    venc.encode(f, { keyFrame: i % (FPS * 2) === 0 });
    f.close();
    while (venc.encodeQueueSize > 8) await new Promise((r) => setTimeout(r, 1));
    if (i % 30 === 0) onProgress?.(i, frames);
  }
  await venc.flush();
  if (failure) throw failure;
  muxer.finalize();
  const blob = new Blob([muxer.target.buffer], { type: 'video/mp4' });
  const b64 = await new Promise((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result).split(',')[1]); fr.readAsDataURL(blob); });
  return { b64, audioCodec: useAac ? 'AAC' : 'Opus' };
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
  // Просмотр: без звука сразу; щелчок по кадру — с начала со звуком, ещё раз — стоп.
  let actx = null, startAt = 0, t0 = performance.now();
  const now = () => (actx ? actx.currentTime - startAt : (performance.now() - t0) / 1000);
  cv.addEventListener('click', async () => {
    if (actx) { await actx.close(); actx = null; t0 = performance.now(); return; }
    const buf = await buildAudio();
    actx = new AudioContext({ sampleRate: RATE });
    const src = actx.createBufferSource(); src.buffer = buf; src.loop = true;
    src.connect(actx.destination);
    startAt = actx.currentTime + 0.05;
    src.start(startAt);
  });
  const loop = () => { draw(((now() % DUR) + DUR) % DUR); requestAnimationFrame(loop); };
  loop();
})();
