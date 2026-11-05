// Фон главной из макета: сетка точек, по которой бегут волны подключения.

import { useEffect, useRef } from 'react';
import { toRgb } from './theme';

export type FieldMode = 'off' | 'conn' | 'on' | 'disc';

const CX = 190;
const CY = 199;
const R = 205;
const SP = 13;

interface St {
  mode: FieldMode;
  t0: number;
  burst: number;
  acc: string;
  base: string;
  dark: boolean;
}

function draw(ctx: CanvasRenderingContext2D, W: number, H: number, now: number, st: St) {
  ctx.clearRect(0, 0, W, H);
  const tc = (now - st.t0) / 1000;
  const tb = st.burst ? (now - st.burst) / 1000 : 99;
  let glow = 0;
  if (st.mode === 'conn') glow = 0.16 + 0.08 * Math.sin(tc * 5);
  else if (st.mode === 'on') glow = 0.1 + 0.03 * Math.sin(tc * 1.4);
  if (tb < 1.3) glow = Math.max(glow, 0.38 * (1 - tb / 1.3));
  if (st.mode === 'disc') glow = Math.max(0, 0.24 * (1 - tc / 0.7));
  if (glow > 0) {
    const gr = ctx.createRadialGradient(CX, CY, 20, CX, CY, 150);
    gr.addColorStop(0, 'rgba(' + st.acc + ',' + glow + ')');
    gr.addColorStop(1, 'rgba(' + st.acc + ',0)');
    ctx.fillStyle = gr;
    ctx.fillRect(0, 0, W, H);
  }
  const sy = SP * 0.866;
  let row = 0;
  for (let y = sy / 2; y < H; y += sy, row++) {
    for (let x = row % 2 ? SP : SP / 2; x < W; x += SP) {
      const dx = x - CX;
      const dy = y - CY;
      const d = Math.hypot(dx, dy);
      let f = 1 - d / R;
      if (f <= 0 || d < 34) continue;
      f = f * f * (3 - 2 * f);
      let I = 0;
      let push = 0;
      if (st.mode === 'conn') {
        for (let k = 0; k < 3; k++) {
          const r = R - ((tc * 170 + (k * R) / 3) % R);
          const g = Math.exp(-Math.pow((d - r) / 13, 2));
          I = Math.max(I, g * (0.35 + 0.65 * (1 - r / R)));
          push -= g * 3.2;
        }
      } else if (st.mode === 'disc') {
        const k = Math.min(1, tc / 0.7);
        const r = R * (1 - k) + 30 * k;
        const g = Math.exp(-Math.pow((d - r) / 18, 2)) * (1 - k * 0.85);
        I = Math.max(I, g);
        push -= g * 7 * (1 - k);
      } else if (st.mode === 'on') {
        const cyc = R + 80;
        const r = (tc * 62) % cyc;
        const g = Math.exp(-Math.pow((d - r) / 20, 2)) * 0.55 * Math.max(0, 1 - r / R);
        I = Math.max(g, 0.06 + 0.06 * Math.sin(tc * 1.6 + x * 0.09 + y * 0.05));
      }
      if (tb < 1.3) {
        const r = 30 + tb * 300;
        const g = Math.exp(-Math.pow((d - r) / 22, 2)) * (1 - tb / 1.3);
        I = Math.max(I, g);
        push += g * 6;
      }
      const ux = dx / d;
      const uy = dy / d;
      const px = x + ux * push;
      const py = y + uy * push;
      ctx.fillStyle = 'rgba(' + st.base + ',' + (st.dark ? 0.24 : 0.34) * f + ')';
      ctx.beginPath();
      ctx.arc(px, py, 1, 0, 6.2832);
      ctx.fill();
      if (I > 0.03) {
        ctx.fillStyle = 'rgba(' + st.acc + ',' + Math.min(1, I * 1.2) * f + ')';
        ctx.beginPath();
        ctx.arc(px, py, 1 + I * 1.5, 0, 6.2832);
        ctx.fill();
      }
    }
  }
}

export function Field({ mode, dark }: { mode: FieldMode; dark: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const darkRef = useRef(dark);
  darkRef.current = dark;

  useEffect(() => {
    let reduced = false;
    try {
      reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
    } catch {
      /* без matchMedia — с анимацией */
    }
    let fm: FieldMode = 'off';
    let ft0 = 0;
    let burst = 0;
    let fc = 0;
    let cols: { acc: string; base: string } | null = null;
    let sized = 0;
    let raf = 0;
    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      const cv = ref.current;
      if (!cv) return;
      const dpr = window.devicePixelRatio || 1;
      if (sized !== dpr) {
        cv.width = 380 * dpr;
        cv.height = 400 * dpr;
        sized = dpr;
      }
      const m = modeRef.current;
      if (m !== fm) {
        if (fm === 'conn' && m === 'on') burst = now;
        fm = m;
        ft0 = now;
      }
      if (!cols || fc++ % 30 === 0) {
        const cs = getComputedStyle(cv);
        cols = { acc: toRgb(cs.getPropertyValue('--accent')), base: toRgb(cs.getPropertyValue('--text3')) };
      }
      const ctx = cv.getContext('2d');
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      draw(ctx, 380, 400, reduced ? ft0 : now, { mode: reduced ? 'off' : m, t0: ft0, burst: reduced ? 0 : burst, acc: cols.acc, base: cols.base, dark: darkRef.current });
    };
    raf = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(raf);
  }, []);

  return <canvas ref={ref} aria-hidden="true" width={380} height={400} style={{ position: 'absolute', left: -16, top: -24, width: 380, height: 400, zIndex: -1, pointerEvents: 'none' }} />;
}
