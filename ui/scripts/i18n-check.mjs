// Проверка перевода: `npm run i18n`.
// 1. Русский текст в интерфейсе, не обёрнутый в t() / tk() / tmap() / plural(), — забыт.
// 2. Ключ t() без английского в src/lib/en.ts — не переведён.
// 3. Подстановки {name} в русском и английском должны совпадать.
// Строку, где русский текст — данные, а не интерфейс (домены .рф, разбор ответов), помечают
// комментарием `i18n-skip` на этой же строке.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { en } from '../src/lib/en.ts';

const SRC = fileURLToPath(new URL('../src/', import.meta.url));
// Превью со службой-заглушкой и сами словари не проверяем.
const SKIP = new Set(['lib/mock.ts', 'Preview.tsx', 'lib/en.ts', 'lib/lang.ts']);
const CYR = /[А-Яа-яЁё]/;

function files(dir) {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? files(p) : /\.(ts|tsx)$/.test(n) ? [p] : [];
  });
}

/** Строки и комментарии; `code` — текст, где они заменены пробелами (скобки считать можно). */
function lex(src) {
  const strings = [];
  const code = src.split('');
  const blank = (a, b) => {
    for (let k = a; k < b; k++) if (code[k] !== '\n') code[k] = ' ';
  };
  const stack = []; // строка шаблона — внутри него, '{' — внутри ${…}
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    const top = stack[stack.length - 1];
    if (typeof top === 'object') {
      // Текст шаблона до `${` или закрывающей кавычки.
      const start = i;
      let text = '';
      while (i < src.length && src[i] !== '`' && !(src[i] === '$' && src[i + 1] === '{')) {
        if (src[i] === '\\') text += src[i++];
        text += src[i++];
      }
      top.parts.push(text);
      blank(start, i);
      if (src[i] === '`') {
        stack.pop();
        top.end = i + 1;
        i++;
      } else {
        top.expr = true;
        stack.push('{');
        i += 2;
      }
      continue;
    }
    if (c === '/' && src[i + 1] === '/') {
      const e = src.indexOf('\n', i);
      const end = e < 0 ? src.length : e;
      blank(i, end);
      i = end;
      continue;
    }
    if (c === '/' && src[i + 1] === '*') {
      const e = src.indexOf('*/', i + 2);
      const end = e < 0 ? src.length : e + 2;
      blank(i, end);
      i = end;
      continue;
    }
    if (c === "'" || c === '"') {
      const start = i++;
      let text = '';
      while (i < src.length && src[i] !== c && src[i] !== '\n') {
        if (src[i] === '\\') text += src[i++];
        text += src[i++];
      }
      i++;
      strings.push({ start, end: i, parts: [text], expr: false });
      blank(start + 1, i - 1);
      continue;
    }
    if (c === '`') {
      const tpl = { start: i, end: -1, parts: [], expr: false };
      strings.push(tpl);
      stack.push(tpl);
      i++;
      continue;
    }
    if (c === '{' && stack.length) stack.push('{');
    if (c === '}' && top === '{') {
      stack.pop();
      i++;
      continue;
    }
    i++;
  }
  return { strings, code: code.join('') };
}

/** Области вызовов `name(…)` в тексте без строк и комментариев. */
function regions(code, name) {
  const out = [];
  const re = new RegExp(`(?<![\\w.])${name}\\(`, 'g');
  let m;
  while ((m = re.exec(code))) {
    let depth = 0;
    for (let k = m.index + name.length; k < code.length; k++) {
      if (code[k] === '(') depth++;
      else if (code[k] === ')' && --depth === 0) {
        out.push([m.index, k]);
        break;
      }
    }
  }
  return out;
}

const inside = (pos, rs) => rs.some(([a, b]) => pos > a && pos < b);
const lineOf = (src, pos) => src.slice(0, pos).split('\n').length;
const vars = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');

const forgotten = [];
const missing = new Map();
const used = new Set();

for (const file of files(SRC)) {
  const rel = relative(SRC, file).split(sep).join('/');
  if (SKIP.has(rel)) continue;
  const src = readFileSync(file, 'utf8');
  const lines = src.split('\n');
  const { strings, code } = lex(src);
  const tmaps = regions(code, 'tmap');
  const plurals = regions(code, 'plural');
  const skip = (pos) => /i18n-skip/.test(lines[lineOf(src, pos) - 1]);

  for (const s of strings) {
    const text = s.parts.join('${…}');
    if (!CYR.test(text) || skip(s.start)) continue;
    const before = code.slice(0, s.start).trimEnd();
    const where = `${rel}:${lineOf(src, s.start)}`;
    if (/(?<![\w.])(t|tk)\($/.test(before) || inside(s.start, tmaps)) {
      if (s.expr) forgotten.push(`${where}  ключ с \${…}, нужны {подстановки}: ${text}`);
      else {
        used.add(text);
        if (!(text in en)) missing.set(text, where);
        else if (vars(text) !== vars(en[text])) forgotten.push(`${where}  подстановки не совпадают: «${text}» / «${en[text]}»`);
      }
      continue;
    }
    if (inside(s.start, plurals)) {
      // Первая форма — ключ: «один|много».
      const r = plurals.find(([a, b]) => s.start > a && s.start < b);
      const first = strings.find((x) => x.start > r[0] && x.start < r[1] && CYR.test(x.parts.join('')));
      if (first === s) {
        used.add(text);
        if (!(text in en)) missing.set(text, where);
        else if (!en[text].includes('|')) forgotten.push(`${where}  склонение без «|»: «${en[text]}»`);
      }
      continue;
    }
    forgotten.push(`${where}  ${text}`);
  }
  // Кириллица вне строк и комментариев — текст в разметке JSX.
  code.split('\n').forEach((l, k) => {
    if (CYR.test(l) && !/i18n-skip/.test(lines[k])) forgotten.push(`${rel}:${k + 1}  разметка: ${l.trim()}`);
  });
}

const stale = Object.keys(en).filter((k) => !used.has(k));
for (const f of forgotten) console.log('без перевода  ' + f);
for (const [k, w] of missing) console.log(`нет в en.ts   ${w}  ${k}`);
for (const k of stale) console.log(`лишний ключ   ${k}`);
console.log(`\nбез t(): ${forgotten.length}, нет перевода: ${missing.size}, лишних ключей: ${stale.length}, переведено: ${used.size - missing.size}`);
process.exit(forgotten.length || missing.size ? 1 : 0);
