// Стили из макета записаны строками CSS, как у Димы: так их проще сверять с макетом один в один.
// css('display: flex; gap: 4px') → объект для style. Разобранные строки кэшируются.

import type { CSSProperties } from 'react';

const cache = new Map<string, CSSProperties>();

const camel = (n: string) => n.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase());

export function css(str: string): CSSProperties {
  const hit = cache.get(str);
  if (hit) return hit;
  const o: Record<string, string> = {};
  let depth = 0;
  let cur = '';
  const parts: string[] = [];
  for (const ch of str) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ';' && depth === 0) {
      parts.push(cur);
      cur = '';
    } else cur += ch;
  }
  parts.push(cur);
  for (const p of parts) {
    const i = p.indexOf(':');
    if (i < 0) continue;
    let k = p.slice(0, i).trim();
    const v = p.slice(i + 1).trim();
    if (!k) continue;
    if (k.startsWith('--')) {
      o[k] = v;
      continue;
    }
    if (k.startsWith('-')) {
      k = k.slice(1);
      const c = camel(k);
      o[c.startsWith('ms') ? c : c[0].toUpperCase() + c.slice(1)] = v;
      continue;
    }
    o[camel(k)] = v;
  }
  if (cache.size > 4000) cache.clear();
  cache.set(str, o as CSSProperties);
  return o as CSSProperties;
}
