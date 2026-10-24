// Проверка серверов: отметки и совет «переключиться на лучший» (`npm test`).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { betterServer, probeCounts, probeMark } from '../src/lib/probe.ts';

const srv = (name, kind, verdict, delay_ms, flaky = false) => ({ name, kind, delay: delay_ms ?? null, selected: false, probe: verdict ? { verdict, flaky, delay_ms } : undefined });

test('отметки: рабочий без отметки, нестабильный и сбои — с отметкой', () => {
  assert.equal(probeMark(undefined), null);
  assert.equal(probeMark({ verdict: 'ok', flaky: false }), null);
  assert.equal(probeMark({ verdict: 'ok', flaky: true }).text, 'нестабильно');
  assert.equal(probeMark({ verdict: 'freeze', flaky: false }).tone, 'warn');
  assert.equal(probeMark({ verdict: 'tcp_blocked', flaky: true }).tone, 'bad');
  assert.match(probeMark({ verdict: 'tcp_blocked', flaky: true }).title, /Нестабильно/);
  assert.equal(probeMark({ verdict: 'новый', flaky: false }), null);
});

test('совет: лучший рабочий, из близких — другой протокол', () => {
  const cur = srv('NL · VLESS', 'vless', 'freeze', 40);
  const list = [
    cur,
    srv('DE · VLESS', 'vless', 'ok', 30),
    srv('FI · HY2', 'hysteria2+realm', 'ok', 45),
    srv('EE · HY2', 'hysteria2', 'ok', 20, true),
    srv('TR · AWG', 'wireguard', 'freeze', 10),
  ];
  // DE быстрее на 15 мс, но тоже VLESS — берём Hysteria2; нестабильный и замороженный не годятся.
  assert.equal(betterServer(list, cur).name, 'FI · HY2');
  // Свой протокол сильно быстрее — берём его.
  list[1] = srv('DE · VLESS', 'vless', 'ok', 10);
  assert.equal(betterServer(list, cur).name, 'DE · VLESS');
  // Текущий работает — совета нет; рабочих нет — тоже.
  assert.equal(betterServer(list, srv('X', 'vless', 'ok', 50)), null);
  assert.equal(betterServer([cur, srv('Y', 'vless', 'tcp_blocked')], cur), null);
  assert.equal(betterServer(list, undefined), null);
});

test('итоги по серверам', () => {
  const list = [srv('a', 'vless', 'ok', 1), srv('b', 'vless', 'ok', 1, true), srv('c', 'vless', 'freeze', 1), srv('d', 'vless', 'no_answer'), srv('e', 'vless')];
  list[2].probe.dns = 'different';
  assert.deepEqual(probeCounts(list), { ok: 2, freeze: 1, down: 1, flaky: 1, dns: 1, checked: 4 });
});
