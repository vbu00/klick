// Разбор стран из имён серверов: `npm test` (Node 22.18+ читает .ts сам).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { countryName, groupByCountry, parseServer } from '../src/lib/countries.ts';

const p = (name, kind = 'vless') => {
  const r = parseServer(name, kind);
  return [r.code, r.title, r.tag, r.named];
};

test('имена Mistgate: флаг и код', () => {
  assert.deepEqual(p('🇧🇾 BY · HY2', 'hysteria2'), ['BY', 'Hysteria2', 'HY2', true]);
  assert.deepEqual(p('🇩🇪 DE · HY2 · WARP', 'hysteria2'), ['DE', 'Hysteria2 · WARP', 'HY2', true]);
  assert.deepEqual(p('🇳🇱 NL · VLESS · XHTTP'), ['NL', 'VLESS · XHTTP', 'VLESS', true]);
  assert.deepEqual(p('🇪🇪 EE · AWG3.1', 'wireguard'), ['EE', 'AmneziaWG 3.1', 'AWG', true]);
  assert.deepEqual(p('🇷🇺 RU · AWG2', 'wireguard'), ['RU', 'AmneziaWG 2', 'AWG', true]);
  // Windows без эмодзи: только код.
  assert.deepEqual(p('TR · VLESS · TCP'), ['TR', 'VLESS · TCP', 'VLESS', true]);
});

test('код страны в начале имени', () => {
  assert.deepEqual(p('DE-Frankfurt'), ['DE', 'Frankfurt', 'VLESS', false]);
  assert.deepEqual(p('[NL] Amsterdam 2'), ['NL', 'Amsterdam 2', 'VLESS', false]);
  assert.deepEqual(p('(US) Miami', 'trojan'), ['US', 'Miami', 'Trojan', false]);
  assert.equal(p('NL2')[0], 'NL');
  // Не страны: протокол, сокращения, просто буквы.
  assert.equal(p('HY2 Fast', 'hysteria2')[0], null);
  assert.equal(p('SS-1', 'ss')[0], null);
  assert.equal(p('AWG2', 'wireguard')[0], null);
  assert.equal(p('VPN-1')[0], null);
});

test('флаг без кода, флаг с другим кодом', () => {
  assert.deepEqual(p('🇳🇱 Amsterdam'), ['NL', 'Amsterdam', 'VLESS', false]);
  assert.deepEqual(p('🇫🇮'), ['FI', '', 'VLESS', false]);
  assert.equal(p('🇩🇪 NL relay')[0], 'DE');
});

test('названия стран по-русски и по-английски', () => {
  assert.deepEqual(p('Германия · Frankfurt'), ['DE', 'Frankfurt', 'VLESS', false]);
  assert.deepEqual(p('Нидерланды'), ['NL', '', 'VLESS', false]);
  assert.deepEqual(p('Netherlands #3'), ['NL', '#3', 'VLESS', false]);
  assert.equal(p('Сервер в Финляндии')[0], 'FI');
  assert.equal(p('США · New York', 'trojan')[0], 'US');
  assert.equal(p('United Kingdom')[0], 'GB');
  assert.equal(p('Türkiye 1')[0], 'TR');
  assert.equal(p('Turkey 1')[0], 'TR');
  assert.equal(p('Южная Корея')[0], 'KR');
  assert.equal(p('Белоруссия')[0], 'BY');
  // Город узнаём, но из подписи не убираем.
  assert.deepEqual(p('Amsterdam fast'), ['NL', 'Amsterdam fast', 'VLESS', false]);
  assert.equal(p('Moscow')[0], 'RU');
});

test('не распознано — «Другие»', () => {
  assert.deepEqual(p('AWG3.1', 'wireguard'), [null, 'AmneziaWG 3.1', 'AWG', true]);
  assert.equal(p('Server 1')[0], null);
  assert.equal(p('Индиго')[0], null);
  assert.equal(p('wireguard', 'wireguard')[2], 'WG');
});

test('названия стран по-русски, коротко', () => {
  assert.deepEqual(['BY', 'DE', 'US', 'GB', 'HK', 'AE', null].map(countryName), ['Беларусь', 'Германия', 'США', 'Великобритания', 'Гонконг', 'ОАЭ', 'Другие']);
});

test('группы: выбранная сверху, по алфавиту, «Другие» в конце', () => {
  const list = ['🇷🇺 RU · HY2', 'AWG2', '🇩🇪 DE · HY2', 'Беларусь', '🇩🇪 DE · AWG2', 'NL-1'].map((name) => ({ name, kind: 'vless' }));
  const keys = (pin) => groupByCountry(list, pin).map((g) => `${g.key}:${g.items.length}`);
  assert.deepEqual(keys(), ['BY:1', 'DE:2', 'NL:1', 'RU:1', ':1']);
  assert.deepEqual(keys('RU'), ['RU:1', 'BY:1', 'DE:2', 'NL:1', ':1']);
  assert.deepEqual(keys(''), [':1', 'BY:1', 'DE:2', 'NL:1', 'RU:1']);
  assert.deepEqual(
    groupByCountry(list).map((g) => g.name),
    ['Беларусь', 'Германия', 'Нидерланды', 'Россия', 'Другие'],
  );
  // Внутри страны — порядок подписки.
  assert.deepEqual(groupByCountry(list)[1].items.map((i) => i.server.name), ['🇩🇪 DE · HY2', '🇩🇪 DE · AWG2']);
});
