// kl!ck — пролёт по приложению (9:16, 1080×1920, 30 к/с, со звуком).
// Экраны разложены на большом поле; камера перелетает от одного к другому,
// на лету отдаляясь и слегка наклоняя поле, у цели — приближение и нажатие.
// Слова «VPN» нет ни в подписях, ни в кадре: режим «Системный proxy», из
// правил и Kill Switch — только вырезки без пояснений.
'use strict';

const W = 1080, H = 1920, FPS = 30, RATE = 48000;
const cv = document.getElementById('c');
const ctx = cv.getContext('2d');
const FONT = '"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif';
const C = { bg: '#090b10', card: '#1a1a1d', text: '#f2f2f4', dim: '#8e8e93', blue: '#007AFF', green: '#22C38A', accent: '#30d158' };

// ─────────── Помощники ───────────

const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const prog = (t, a, b) => clamp((t - a) / (b - a));
const lerp = (a, b, k) => a + (b - a) * k;
const E = {
  out: (x) => 1 - Math.pow(1 - x, 3),
  inOut: (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2),
  back: (x) => { const c1 = 1.70158, c3 = c1 + 1; return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2); },
};
const hexA = (hex, a) => { const n = parseInt(hex.slice(1), 16); return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`; };
function rng(seed) {
  return () => { seed |= 0; seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
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
const LOGO = [['kl', C.text], ['!', C.blue], ['ck', C.text]];
function logo(cx, y, size, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.font = `800 ${size}px ${FONT}`;
  const widths = LOGO.map(([s]) => ctx.measureText(s).width);
  let x = cx - widths.reduce((a, b) => a + b, 0) / 2;
  ctx.textAlign = 'left';
  LOGO.forEach(([s, color], i) => { ctx.fillStyle = color; ctx.fillText(s, x, y); x += widths[i]; });
  ctx.restore();
}

// ─────────── Экраны на поле ───────────
// Мировые координаты = пиксели 2x-скриншотов (окно 380×720 → 760×1440).

const IMG = {};
const SRC = {
  key: '../src-tauri/icons/icon.png',
  off: 'assets/fly-off.png', on: 'assets/fly-on.png', servers: 'assets/fly-servers.png', add: 'assets/fly-add.png',
  rules: 'assets/fly-rules.png', ks: 'assets/fly-ks.png', tray: 'assets/fly-tray.png', theme: 'assets/fly-theme.png',
};
const loadImages = () => Promise.all(Object.entries(SRC).map(([k, src]) => new Promise((ok, fail) => {
  const im = new Image();
  im.onload = () => { IMG[k] = im; ok(); };
  im.onerror = () => fail(new Error('не загрузилась ' + src));
  im.src = src;
})));

// Карточки: картинка, вырезка [sx, sy, sw, sh] и место на поле (x, y).
const CARDS = {
  home: { img: 'off', crop: [0, 0, 760, 1440], x: 0, y: 0 },
  servers: { img: 'servers', crop: [0, 0, 760, 1440], x: 900, y: 380 },
  add: { img: 'add', crop: [0, 0, 760, 1440], x: 0, y: 1640 },
  rules: { img: 'rules', crop: [0, 440, 760, 500], x: 900, y: 2020 },
  ks: { img: 'ks', crop: [0, 580, 760, 640], x: 0, y: 3280 },
  tray: { img: 'tray', crop: [20, 16, 640, 1094], x: 960, y: 2760 },
  theme: { img: 'theme', crop: [0, 0, 760, 1440], x: 0, y: 4100 },
};
const BOARD = { x: -120, y: -120, w: 1900, h: 5800 };

// Остановки камеры: что в кадре (прямоугольник поля), подпись, нажатие.
// tap — точка в координатах карточки (до вырезки: как на скриншоте).
const STOPS = [
  { id: 'intro', from: 0, to: 2.6 },
  { id: 'home', from: 2.6, to: 6.0, rect: ['home', 0, 0, 760, 780], title: 'Одна кнопка', sub: 'и ты подключён', tap: [380, 428], tapAt: 4.3 },
  { id: 'servers', from: 6.0, to: 9.0, rect: ['servers', 0, 560, 760, 680], title: 'Все серверы подписки', sub: 'с реальной задержкой', tap: [556, 696], tapAt: 7.3, glow: [40, 736, 680, 112, 7.9] },
  { id: 'add', from: 9.0, to: 11.8, rect: ['add', 0, 110, 760, 740], title: 'Вставил ссылку —', sub: 'остальное kl!ck сделает сам', tap: [380, 908], tapAt: 10.6 },
  { id: 'rules', from: 11.8, to: 14.4, rect: ['rules', 0, 440, 760, 500], title: 'Свои правила', sub: 'для сайтов и программ', tap: [654, 846], tapAt: 13.2 },
  { id: 'ks', from: 14.4, to: 17.0, rect: ['ks', 0, 580, 760, 640], title: 'Kill Switch', sub: 'даже для отдельных сайтов', tap: [580, 1140], tapAt: 15.8 },
  { id: 'tray', from: 17.0, to: 19.8, rect: ['tray', 20, 16, 640, 1094], title: 'Всё под рукой', sub: 'прямо в трее', tap: [540, 626], tapAt: 18.6 },
  { id: 'theme', from: 19.8, to: 22.6, rect: ['theme', 0, 170, 760, 900], title: 'Под твой вкус', sub: '4 темы · 6 акцентов', tap: [211, 1210], tapAt: 21.4 },
  { id: 'outro', from: 22.6, to: 26.6 },
];
const DUR = STOPS[STOPS.length - 1].to;
const MOVE = 0.85; // сколько длится перелёт
const LOGO_IN = 0.9, LOGO_OUT = 23.9;

/** Прямоугольник поля в мировых координатах. */
function worldRect(r) {
  const [card, x, y, w, h] = r;
  const c = CARDS[card];
  return [c.x + x - c.crop[0], c.y + y - c.crop[1], w, h];
}
/** Камера для прямоугольника: центр и масштаб, чтобы он влез в окно под подписью. */
function fit(r) {
  const [x, y, w, h] = r;
  const z = Math.min(900 / w, 1020 / h);
  return { x: x + w / 2, y: y + h / 2, z, rot: 0 };
}
const WIDE = { x: BOARD.x + BOARD.w / 2, y: 2300, z: 0.3, rot: -0.12 };

function cameraAt(t) {
  const i = STOPS.findIndex((s) => t >= s.from && t < s.to);
  const s = STOPS[Math.max(0, i)];
  const target = s.rect ? fit(worldRect(s.rect)) : { ...WIDE, y: s.id === 'intro' ? 900 : 2300, z: s.id === 'intro' ? 0.34 : 0.3 };
  // Лёгкий дрейф на месте — кадр не замирает.
  const drift = (t - s.from) * 6;
  const here = { ...target, y: target.y + drift };
  if (i <= 0) return { ...here, rot: WIDE.rot, z: here.z * (1 + 0.04 * t) };
  const prev = STOPS[i - 1];
  const from = prev.rect ? fit(worldRect(prev.rect)) : { ...WIDE, y: 900, z: 0.34 };
  from.y += (prev.to - prev.from) * 6;
  const k = prog(t, s.from, s.from + MOVE);
  if (k >= 1) return here;
  const e = E.inOut(k), arc = Math.sin(Math.PI * k);
  return {
    x: lerp(from.x, here.x, e),
    y: lerp(from.y, here.y, e),
    z: lerp(from.z, here.z, e) * (1 - 0.42 * arc),
    rot: lerp(from.rot || 0, here.rot || 0, e) - 0.07 * arc,
  };
}

// ─────────── Рисование ───────────

function drawCard(name, t, focus) {
  const c = CARDS[name];
  const [sx, sy, sw, sh] = c.crop;
  ctx.save();
  ctx.globalAlpha = focus;
  ctx.shadowColor = 'rgba(0,0,0,.65)'; ctx.shadowBlur = 80; ctx.shadowOffsetY = 30;
  rrect(c.x, c.y, sw, sh, 44); ctx.fillStyle = C.card; ctx.fill();
  ctx.shadowColor = 'transparent';
  ctx.clip();
  ctx.drawImage(IMG[c.img], sx, sy, sw, sh, c.x, c.y, sw, sh);
  // Главная: после нажатия — подключено.
  if (name === 'home') {
    const k = prog(t, STOPS[1].tapAt + 0.05, STOPS[1].tapAt + 0.4);
    if (k > 0) { ctx.globalAlpha = focus * k; ctx.drawImage(IMG.on, sx, sy, sw, sh, c.x, c.y, sw, sh); }
  }
  ctx.restore();
  ctx.save();
  ctx.globalAlpha = focus;
  rrect(c.x, c.y, sw, sh, 44); ctx.strokeStyle = 'rgba(255,255,255,.1)'; ctx.lineWidth = 3; ctx.stroke();
  ctx.restore();
}

/** Нажатие в координатах поля: точка-палец и кольцо. */
function tapAt(x, y, t, at, z) {
  const f = prog(t, at - 0.35, at - 0.1) * (1 - prog(t, at + 0.25, at + 0.45));
  const r = 1 / z;
  if (f > 0) {
    const press = t > at - 0.06 && t < at + 0.12 ? 0.82 : 1;
    ctx.save(); ctx.globalAlpha = f * 0.9;
    ctx.beginPath(); ctx.arc(x, y, 30 * r * press, 0, Math.PI * 2); ctx.fillStyle = '#fff'; ctx.fill();
    ctx.restore();
  }
  const q = prog(t, at, at + 0.55);
  if (q > 0 && q < 1) {
    ctx.beginPath(); ctx.arc(x, y, lerp(26, 120, E.out(q)) * r, 0, Math.PI * 2);
    ctx.strokeStyle = hexA(C.accent, 0.85 * (1 - q)); ctx.lineWidth = 6 * r; ctx.stroke();
  }
}

function background(t, cam) {
  ctx.fillStyle = C.bg;
  ctx.fillRect(0, 0, W, H);
  const glows = [[0.2 + 0.1 * Math.sin(t * 0.3), 0.25, C.blue, 0.22], [0.85, 0.75 + 0.06 * Math.cos(t * 0.4), C.green, 0.12]];
  for (const [bx, by, color, a] of glows) {
    const g = ctx.createRadialGradient(bx * W, by * H, 0, bx * W, by * H, 820);
    g.addColorStop(0, hexA(color, a)); g.addColorStop(1, hexA(color, 0));
    ctx.fillStyle = g; ctx.fillRect(0, 0, W, H);
  }
  // Сетка точек с параллаксом — ощущение движения.
  const ox = (-cam.x * 0.08) % 36, oy = (-cam.y * 0.08) % 36;
  ctx.fillStyle = 'rgba(255,255,255,.05)';
  for (let y = oy - 36; y < H; y += 36) for (let x = ox - 36; x < W; x += 36) ctx.fillRect(x, y, 3, 3);
}

function caption(t) {
  const s = STOPS.find((x) => t >= x.from && t < x.to);
  if (!s || !s.title) return;
  const inK = E.out(prog(t, s.from + MOVE * 0.55, s.from + MOVE + 0.2));
  const outK = 1 - prog(t, s.to - 0.25, s.to);
  const a = inK * outK;
  text(s.title, 540, 330 + 24 * (1 - inK), { size: 76, weight: 800, alpha: a });
  text(s.sub, 540, 410 + 24 * (1 - inK), { size: 50, weight: 700, color: s.id === 'ks' ? C.green : C.blue, alpha: E.out(prog(t, s.from + MOVE * 0.7, s.from + MOVE + 0.35)) * outK });
}

function draw(t) {
  t = clamp(t, 0, DUR - 1e-6);
  const cam = cameraAt(t);
  background(t, cam);
  const s = STOPS.find((x) => t >= x.from && t < x.to);
  // Поле с экранами.
  ctx.save();
  ctx.translate(540, 1000);
  ctx.rotate(cam.rot);
  ctx.scale(cam.z, cam.z);
  ctx.translate(-cam.x, -cam.y);
  const settled = s.rect ? prog(t, s.from + MOVE * 0.6, s.from + MOVE) : 0;
  for (const name of Object.keys(CARDS)) {
    const isTarget = s.rect && s.rect[0] === name;
    drawCard(name, t, isTarget ? 1 : lerp(1, 0.28, settled));
  }
  if (s.tap) {
    const c = CARDS[s.rect[0]];
    tapAt(c.x + s.tap[0] - c.crop[0], c.y + s.tap[1] - c.crop[1], t, s.tapAt, cam.z);
  }
  if (s.glow) {
    const [gx, gy, gw, gh, at] = s.glow;
    const c = CARDS[s.rect[0]];
    const k = E.out(prog(t, at, at + 0.35)) * (1 - prog(t, s.to - 0.3, s.to));
    if (k > 0) {
      ctx.save(); ctx.globalAlpha = k;
      rrect(c.x + gx - c.crop[0], c.y + gy - c.crop[1], gw, gh, 24);
      ctx.strokeStyle = C.accent; ctx.lineWidth = 8; ctx.shadowColor = C.accent; ctx.shadowBlur = 40; ctx.stroke();
      ctx.restore();
    }
  }
  ctx.restore();
  // Затемнение в начале и конце — под логотип.
  const intro = 1 - prog(t, 2.0, 2.8), outro = prog(t, 23.3, 24.1);
  const veil = Math.max(intro * 0.72, outro * 0.8);
  if (veil > 0) { ctx.fillStyle = `rgba(9,11,16,${veil})`; ctx.fillRect(0, 0, W, H); }
  // Растяжка под подписью: высокие экраны уходят под неё, текст читается.
  if (s.rect) {
    const g = ctx.createLinearGradient(0, 180, 0, 600);
    g.addColorStop(0, 'rgba(9,11,16,.96)'); g.addColorStop(0.55, 'rgba(9,11,16,.85)'); g.addColorStop(1, 'rgba(9,11,16,0)');
    ctx.fillStyle = g; ctx.fillRect(0, 0, W, 600);
  }
  caption(t);
  drawLogoBlock(t);
}

function drawLogoBlock(t) {
  const blocks = [
    { at: LOGO_IN, until: 2.35, sub: 'для Windows', cta: false },
    { at: LOGO_OUT, until: DUR + 1, sub: 'Бесплатно · Windows 10/11 · открытый код', cta: true },
  ];
  for (const b of blocks) {
    const inK = prog(t, b.at - 0.5, b.at + 0.2), outK = 1 - prog(t, b.until - 0.3, b.until);
    const a = E.out(inK) * outK;
    if (a <= 0) continue;
    const drop = E.back(prog(t, b.at - 0.45, b.at));
    const s = 300 * (0.6 + 0.4 * drop);
    const y = 760 + Math.sin(t * 2) * 8;
    const g = ctx.createRadialGradient(540, y, 0, 540, y, 380);
    g.addColorStop(0, hexA(C.blue, 0.4 * a)); g.addColorStop(1, hexA(C.blue, 0));
    ctx.fillStyle = g; ctx.fillRect(0, 300, W, 1000);
    const squash = t > b.at && t < b.at + 0.1 ? 0.92 : 1;
    ctx.save(); ctx.globalAlpha = a;
    ctx.translate(540, y); ctx.scale(1 / squash, squash);
    ctx.shadowColor = 'rgba(0,40,120,.6)'; ctx.shadowBlur = 50; ctx.shadowOffsetY = 24;
    ctx.drawImage(IMG.key, -s / 2, -s / 2, s, s);
    ctx.restore();
    logo(540, 1080, 150, E.out(prog(t, b.at, b.at + 0.35)) * outK);
    text(b.sub, 540, 1170, { size: 40, weight: 600, color: 'rgba(255,255,255,.78)', alpha: E.out(prog(t, b.at + 0.2, b.at + 0.55)) * outK });
    if (b.cta) {
      const k = E.back(prog(t, b.at + 0.5, b.at + 0.9));
      if (k > 0) {
        ctx.save(); ctx.translate(540, 1310); ctx.scale(k, k);
        rrect(-340, -58, 680, 116, 58); ctx.fillStyle = C.blue; ctx.fill();
        text('github.com/vbu00/klick', 0, 15, { size: 44, weight: 700, color: '#fff' });
        ctx.restore();
      }
    }
  }
}

// ─────────── Звук ───────────
//
// Спокойный бит 100 BPM (Fmaj7 – G6 – Em7 – Am7), свист на каждом перелёте
// камеры, щелчок на нажатии, «тук» клавиши на логотипах, звон на финале.

const BEAT = 0.6;

async function buildAudio() {
  const ac = new OfflineAudioContext(2, Math.ceil(DUR * RATE), RATE);
  const master = ac.createGain(); master.gain.value = 0.9;
  const comp = ac.createDynamicsCompressor();
  comp.threshold.value = -16; comp.ratio.value = 3; comp.attack.value = 0.004; comp.release.value = 0.25;
  master.connect(comp); comp.connect(ac.destination);
  const music = ac.createGain();
  const mf = ac.createBiquadFilter(); mf.type = 'lowpass'; mf.Q.value = 0.7;
  music.connect(mf); mf.connect(master);
  const sfx = ac.createGain(); sfx.gain.value = 0.85; sfx.connect(master);
  // Музыка раскрывается после вступления и затихает к концу.
  mf.frequency.setValueAtTime(900, 0); mf.frequency.exponentialRampToValueAtTime(16000, 2.6);
  music.gain.setValueAtTime(0.0001, 0); music.gain.exponentialRampToValueAtTime(0.55, 0.8);
  music.gain.setValueAtTime(0.55, DUR - 2.2); music.gain.linearRampToValueAtTime(0.0001, DUR - 0.1);

  const R = rng(7);
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
  const hz = (m) => 440 * Math.pow(2, (m - 69) / 12);

  // Инструменты.
  const kick = (t, v = 0.7) => tone(t, 0.3, music, { f: 120, f2: 45, glide: 0.11, v });
  const rim = (t, v = 0.18) => { noise(t, 0.05, music, { type: 'bandpass', f: 2200, q: 4 }, perc(v, 0.04)); tone(t, 0.04, music, { f: 1700, v: v * 0.3 }); };
  const hat = (t, v = 0.07) => noise(t, 0.05, music, { type: 'highpass', f: 9000 }, perc(v, 0.04));
  const pad = (t, notes, dur, v = 0.045) => notes.forEach((m, i) => { tone(t, dur, music, { f: hz(m), type: 'sawtooth', v, lp: 1400, a: 0.4, detune: (i - 1.5) * 6 }); tone(t, dur, music, { f: hz(m) * 1.004, type: 'triangle', v: v * 0.9, a: 0.4 }); });
  const pluck = (t, m, v = 0.07) => { tone(t, 0.5, music, { f: hz(m), type: 'triangle', v, a: 0.003 }); tone(t, 0.3, music, { f: hz(m + 12), v: v * 0.3, a: 0.003 }); };
  const bass = (t, m, dur) => tone(t, dur, music, { f: hz(m), type: 'sine', v: 0.3, a: 0.02 });

  const CHORDS = [[41, [53, 57, 60, 64]], [43, [55, 59, 62, 64]], [40, [52, 55, 59, 62]], [45, [57, 60, 64, 67]]]; // Fmaj7 G6 Em7 Am7
  let bar = 0;
  for (let t = 0.3; t < DUR - 1.5; t += BEAT * 4, bar++) {
    const [root, chord] = CHORDS[bar % 4];
    pad(t, chord, BEAT * 4 + 0.2);
    bass(t, root, BEAT * 1.8); bass(t + BEAT * 2, root, BEAT * 1.6);
    for (let b = 0; b < 4; b++) {
      const tb = t + b * BEAT;
      if (tb > DUR - 1.5) break;
      if (tb > 2.2) { kick(tb, b % 2 ? 0.45 : 0.7); if (b % 2) rim(tb); hat(tb + BEAT / 2); }
      // Арпеджио по нотам аккорда — восьмыми.
      if (tb > 2.2) { pluck(tb, chord[(b * 2) % 4] + 12, 0.05); pluck(tb + BEAT / 2, chord[(b * 2 + 1) % 4] + 12, 0.04); }
    }
  }
  pad(LOGO_OUT, [60, 64, 67, 71], 3, 0.06);

  // Звуки.
  const click = (t, v = 0.4) => { noise(t, 0.03, sfx, { type: 'bandpass', f: 3200, q: 2 }, perc(v, 0.025)); tone(t, 0.04, sfx, { f: 2200, f2: 1400, v: v * 0.4 }); };
  const keyThock = (t, v = 0.9) => {
    noise(t, 0.04, sfx, { type: 'bandpass', f: 2600, q: 1.4 }, perc(v * 0.7, 0.03));
    tone(t, 0.12, sfx, { f: 210, f2: 95, v, glide: 0.08 });
    tone(t + 0.005, 0.08, sfx, { f: 520, f2: 380, v: v * 0.35 });
  };
  const whoosh = (t, dur, v = 0.2) => {
    const f = noise(t, dur, sfx, { type: 'bandpass', f: 400, q: 1.1 }, (g, t0) => { g.setValueAtTime(0.0001, t0); g.exponentialRampToValueAtTime(v, t0 + dur * 0.5); g.exponentialRampToValueAtTime(0.0001, t0 + dur); });
    f.frequency.setValueAtTime(300, t); f.frequency.exponentialRampToValueAtTime(3200, t + dur * 0.5); f.frequency.exponentialRampToValueAtTime(700, t + dur);
  };
  const chime = (t, v = 0.16) => [[1318.5, 0], [1760, 0.07], [2637, 0.14]].forEach(([f, d]) => tone(t + d, 0.8, sfx, { f, v, a: 0.003 }));
  const shimmer = (t, v = 0.08) => [72, 76, 79, 84].forEach((m, i) => tone(t + i * 0.05, 0.6, sfx, { f: hz(m), v, a: 0.003 }));
  const swell = (t, dur, v = 0.12) => { const f = noise(t, dur, sfx, { type: 'bandpass', f: 500, q: 1.5 }, (g, t0) => { g.setValueAtTime(0.0001, t0); g.exponentialRampToValueAtTime(v, t0 + dur); }); f.frequency.setValueAtTime(300, t); f.frequency.exponentialRampToValueAtTime(5000, t + dur); };

  swell(0, LOGO_IN, 0.1);
  keyThock(LOGO_IN, 1.0); tone(LOGO_IN, 1.0, sfx, { f: 55, f2: 40, v: 0.5, glide: 0.7 });
  STOPS.slice(1).forEach((s) => whoosh(s.from - 0.05, MOVE + 0.15, s.id === 'outro' ? 0.24 : 0.18));
  STOPS.filter((s) => s.tapAt).forEach((s) => click(s.tapAt, 0.45));
  tone(STOPS[1].tapAt + 0.05, 0.5, sfx, { f: 220, f2: 880, v: 0.14, glide: 0.35, type: 'triangle' }); // «включилось»
  chime(STOPS[1].tapAt + 0.3, 0.12);
  chime(STOPS[2].glow[4], 0.08);
  shimmer(STOPS[7].tapAt + 0.05);
  swell(LOGO_OUT - 0.7, 0.7, 0.1);
  keyThock(LOGO_OUT, 1.0); tone(LOGO_OUT, 1.2, sfx, { f: 55, f2: 40, v: 0.5, glide: 0.8 });
  chime(LOGO_OUT + 0.6, 0.14);

  const buf = await ac.startRendering();
  // Мастеринг: средний уровень около −15 дБ, пики мягко скругляет tanh.
  const TARGET_DB = -15, CEIL = 0.94;
  let sum = 0, n = 0;
  for (let c = 0; c < 2; c++) { const d = buf.getChannelData(c); for (let i = Math.floor(3 * RATE); i < Math.floor((DUR - 3) * RATE); i++) { sum += d[i] * d[i]; n++; } }
  const gain = Math.pow(10, (TARGET_DB - 20 * Math.log10(Math.sqrt(sum / n) + 1e-9)) / 20);
  for (let c = 0; c < 2; c++) { const d = buf.getChannelData(c); for (let i = 0; i < d.length; i++) d[i] = CEIL * Math.tanh((d[i] * gain) / CEIL); }
  return buf;
}

// ─────────── Запись в MP4 и просмотр ───────────

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

function frameAt(t) { draw(t); return cv.toDataURL('image/png').split(',')[1]; }

(async () => {
  await document.fonts.load(`700 40px ${FONT}`);
  await loadImages();
  window.encodeStory = encodeStory;
  window.frameAt = frameAt;
  window.COVER_T = LOGO_IN + 0.5;
  window.storyReady = true;
  const q = new URLSearchParams(location.search);
  if (q.has('render')) { draw(0); return; }
  if (q.has('t')) { draw(parseFloat(q.get('t'))); return; }
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
