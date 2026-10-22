// Проверка серверов из этой сети: отметки у серверов в списках и совет, на какой сервер перейти.
// Без импортов кода — scripts/probe.test.mjs проверяет модуль прямо в Node.

import type { ServerProbe, ServerView } from './types';

export interface ProbeMark {
  text: string;
  title: string;
  tone: 'warn' | 'bad';
}

const MARKS: Record<Exclude<ServerProbe['verdict'], 'ok'>, ProbeMark> = {
  freeze: { text: 'заморозка', tone: 'warn', title: 'Заморозка: сервер отвечает, но загрузка через него встаёт после нескольких килобайт — так провайдер (ТСПУ) глушит VPN' },
  tcp_blocked: { text: 'блокировка', tone: 'bad', title: 'Провайдер не пускает к серверу' },
  handshake_failed: { text: 'не проходит', tone: 'bad', title: 'До сервера достучались, но его протокол не проходит — похоже, провайдер его режет' },
  no_answer: { text: 'не отвечает', tone: 'bad', title: 'Сервер не ответил при проверке' },
};
const FLAKY = 'Нестабильно: часть попыток прошла, часть — нет';

/** Отметка у сервера в списке. Рабочий сервер — без отметки, чтобы не шуметь. */
export function probeMark(p: ServerProbe | undefined): ProbeMark | null {
  if (!p) return null;
  if (p.verdict === 'ok') return p.flaky ? { text: 'нестабильно', tone: 'warn', title: FLAKY } : null;
  const m = MARKS[p.verdict];
  // Вердикт, которого окно ещё не знает (служба новее), — без отметки.
  if (!m) return null;
  return p.flaky ? { ...m, title: `${m.title}. ${FLAKY}` } : m;
}

/** Тот же протокол: `hysteria2+realm` — это Hysteria2, `hy2` — тоже. */
const family = (kind: string) => kind.toLowerCase().replace(/\+realm$/, '').replace(/^hy2$/, 'hysteria2');

/** Задержка по проверке, иначе по последнему замеру. */
const ms = (s: ServerView) => s.probe?.delay_ms ?? s.delay ?? Infinity;

/** Текущий сервер в этой сети не работает — на какой перейти: рабочий без сбоев, с меньшей задержкой,
 *  а из близких по задержке — другого протокола: провайдер обычно режет протокол целиком. */
export function betterServer(list: ServerView[], current: ServerView | undefined): ServerView | null {
  const v = current?.probe?.verdict;
  if (!current || !v || v === 'ok') return null;
  const ok = list.filter((s) => s.name !== current.name && s.probe?.verdict === 'ok' && !s.probe.flaky);
  if (!ok.length) return null;
  const best = Math.min(...ok.map(ms));
  const same = (s: ServerView) => Number(family(s.kind) === family(current.kind));
  // ponytail: «близкие» — в пределах 20 мс от лучшей; при большом разбросе задержек можно брать долю от лучшей.
  return ok.filter((s) => ms(s) <= best + 20).sort((a, b) => same(a) - same(b) || ms(a) - ms(b))[0];
}

export interface ProbeCounts {
  ok: number;
  freeze: number;
  /** Не пускает провайдер, протокол не проходит или сервер молчит. */
  down: number;
  flaky: number;
  /** У скольких серверов провайдер подменил ответ DNS. */
  dns: number;
  checked: number;
}

export function probeCounts(list: ServerView[]): ProbeCounts {
  const c: ProbeCounts = { ok: 0, freeze: 0, down: 0, flaky: 0, dns: 0, checked: 0 };
  for (const s of list) {
    const p = s.probe;
    if (!p) continue;
    c.checked++;
    if (p.verdict === 'ok') c.ok++;
    else if (p.verdict === 'freeze') c.freeze++;
    else c.down++;
    if (p.flaky) c.flaky++;
    if (p.dns === 'different' || p.dns === 'bogus') c.dns++;
  }
  return c;
}
