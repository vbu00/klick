// Язык интерфейса. Ключ перевода — сам русский текст: код читается как раньше, а английский
// лежит в en.ts. Проверка, что ничего не забыто: `npm run i18n`.

import { en } from './en';

export type Lang = 'ru' | 'en';

const CACHE = 'klick.lang';

/** Язык Windows — пока служба не прислала настройки, и в установщике. */
export function systemLang(): Lang {
  const l = typeof navigator !== 'undefined' ? navigator.language : 'ru';
  return /^(ru|be|uk|kk)\b/i.test(l) ? 'ru' : 'en';
}

function cached(): Lang {
  try {
    const v = localStorage.getItem(CACHE);
    if (v === 'ru' || v === 'en') return v;
  } catch {
    /* хранилище недоступно */
  }
  return systemLang();
}

let current: Lang = cached();
if (typeof document !== 'undefined') document.documentElement.lang = current;

export function lang(): Lang {
  return current;
}

export function setLang(l: Lang): void {
  current = l;
  if (typeof document !== 'undefined') document.documentElement.lang = l;
  try {
    localStorage.setItem(CACHE, l);
  } catch {
    /* хранилище недоступно */
  }
}

/** Текст на языке интерфейса. `{name}` в тексте — подстановки из `vars`. */
export function t(ru: string, vars?: Record<string, string | number>): string {
  let s = current === 'en' ? (en[ru] ?? ru) : ru;
  if (vars) s = s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
  return s;
}

/** Пометка «это ключ перевода» для текста в константе модуля: переводится при показе через t(). */
export const tk = (ru: string): string => ru;

/** Таблица названий, которая переводится при чтении: `tmap({ tun: 'Системный прокси' })[k]`. */
export function tmap<K extends string>(m: Record<K, string>): Record<K, string> {
  return new Proxy(m, { get: (o, k) => (typeof k === 'string' && typeof o[k as K] === 'string' ? t(o[k as K]) : Reflect.get(o, k)) });
}

/**
 * 1 программа, 2 программы, 5 программ. По-английски — `en[первая форма]` вида «program|programs».
 */
export function plural(n: number, forms: [string, string, string]): string {
  if (current === 'en') {
    const [one, many] = (en[forms[0]] ?? forms[0]).split('|');
    return `${n} ${n === 1 ? one : many ?? one}`;
  }
  const m10 = n % 10;
  const m100 = n % 100;
  const form = m10 === 1 && m100 !== 11 ? forms[0] : m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14) ? forms[1] : forms[2];
  return `${n} ${form}`;
}
