// Страна сервера по его имени из подписки и подпись для списка «по странам».
// Имена придумывает панель: «🇧🇾 BY · HY2», «[NL] Amsterdam», «Германия · Frankfurt», «AWG2».
// Модуль без зависимостей во время выполнения: его проверяет scripts/countries.test.mjs прямо в Node.

/** Сервер, разобранный для списка по странам. */
export interface ParsedServer {
  /** Код страны ISO (`DE`); null — страну не узнали, сервер пойдёт в «Другие». */
  code: string | null;
  /** Остаток имени без страны, по-человечески: «Hysteria2 · WARP», «AmneziaWG 3.1», «Amsterdam». Пусто — в имени была только страна. */
  title: string;
  /** Короткая метка протокола для строки страны: HY2, VLESS, AWG… */
  tag: string;
  /** Протокол уже назван в `title` — второй раз его не показываем. */
  named: boolean;
}

export interface CountryGroup<T> {
  /** Код страны; `''` — «Другие». */
  key: string;
  code: string | null;
  name: string;
  items: { server: T; p: ParsedServer }[];
}

const names = (lang: string, style: 'long' | 'short') => {
  try {
    return new Intl.DisplayNames([lang], { type: 'region', style, fallback: 'none' });
  } catch {
    return null;
  }
};
const RU = names('ru', 'long');
const RU_SHORT = names('ru', 'short');
const EN = names('en', 'long');
const EN_SHORT = names('en', 'short');

/** Где имя из Intl длинное или неудачное; короткие имена Intl в WebView2 и Node разные. */
const RU_NAME: Record<string, string> = { US: 'США', GB: 'Великобритания', AE: 'ОАЭ', HK: 'Гонконг', KR: 'Южная Корея', EU: 'Евросоюз' };

/** Двухбуквенные коды, которые в начале имени чаще значат протокол, чем страну. */
const NOT_COUNTRY = new Set(['SS']);

/** Название страны по-русски: «Беларусь», «США»; без Intl — сам код. */
export function countryName(code: string | null): string {
  if (!code) return 'Другие';
  return RU_NAME[code] ?? RU_SHORT?.of(code)?.replace(/\s*\(.*\)$/, '') ?? code;
}

const isRegion = (code: string) => !NOT_COUNTRY.has(code) && (RU ? RU.of(code) !== undefined : true);

// ── Страна по названию в имени ────────────────────────────────────────────

/** Страны, которые узнаём по названию (по флагу и коду — любые). Названия даёт Intl, по-русски и по-английски. */
const NAMED =
  'RU BY UA KZ UZ KG AM GE AZ MD DE NL FI FR GB US PL SE NO DK IS IE BE LU PT ES IT CH AT CZ SK SI HU RO BG RS HR GR CY MT TR IL AE SA QA IN JP KR CN HK TW SG TH VN MY ID PH AU NZ CA MX BR AR CL ZA EG LV LT EE'.split(' ');

/** Другие имена стран: их, как и названия, убираем из начала подписи. */
const ALIASES: [string, string][] = [
  ['США', 'US'], ['USA', 'US'], ['America', 'US'], ['Америка', 'US'],
  ['UK', 'GB'], ['England', 'GB'], ['Britain', 'GB'], ['Англия', 'GB'], ['Британия', 'GB'],
  ['Holland', 'NL'], ['Голландия', 'NL'], ['Turkey', 'TR'], ['Emirates', 'AE'], ['Эмираты', 'AE'],
  ['Korea', 'KR'], ['Корея', 'KR'], ['Czech', 'CZ'], ['Белоруссия', 'BY'],
];

/** Города: по ним узнаём страну, но из подписи не убираем — «Amsterdam» и есть имя сервера. */
const CITIES: [string, string][] = (
  [
    ['NL', 'Amsterdam Амстердам Rotterdam'],
    ['DE', 'Frankfurt Франкфурт Berlin Берлин Munich Мюнхен Nuremberg Falkenstein'],
    ['FI', 'Helsinki Хельсинки'],
    ['FR', 'Paris Париж Gravelines Roubaix'],
    ['GB', 'London Лондон Manchester'],
    ['US', 'New_York Нью-Йорк Los_Angeles Лос-Анджелес Miami Майами Chicago Dallas Seattle Ashburn'],
    ['TR', 'Istanbul Стамбул Ankara'],
    ['JP', 'Tokyo Токио Osaka'],
    ['SE', 'Stockholm Стокгольм'],
    ['PL', 'Warsaw Варшава'],
    ['KZ', 'Almaty Алматы Astana Астана'],
    ['LV', 'Riga Рига'],
    ['LT', 'Vilnius Вильнюс'],
    ['EE', 'Tallinn Таллин'],
    ['CH', 'Zurich Цюрих Geneva'],
    ['AT', 'Vienna Вена'],
    ['CZ', 'Prague Прага'],
    ['IT', 'Milan Милан Rome'],
    ['ES', 'Madrid Мадрид Barcelona'],
    ['CA', 'Toronto Торонто Montreal Vancouver'],
    ['AE', 'Dubai Дубай'],
    ['RU', 'Moscow Москва Мск Петербург Petersburg'],
    ['BY', 'Minsk Минск'],
    ['UA', 'Kyiv Kiev Киев'],
    ['GE', 'Tbilisi Тбилиси'],
    ['AM', 'Yerevan Ереван'],
    ['RS', 'Belgrade Белград'],
    ['HU', 'Budapest Будапешт'],
    ['NO', 'Oslo Осло'],
    ['IL', 'Tel_Aviv Тель-Авив'],
    ['KR', 'Seoul Сеул'],
    ['AU', 'Sydney Сидней'],
  ] as [string, string][]
).flatMap(([code, list]) => list.split(' ').map((n): [string, string] => [n.replace('_', ' '), code]));

const norm = (s: string) => s.toLowerCase().replace(/ё/g, 'е');
const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

interface Pattern {
  re: RegExp;
  code: string;
  /** Название страны (убираем из начала подписи), а не город. */
  country: boolean;
}

/** Русское название ловим по основе с падежным окончанием: «Германия», «в Германии», «в Нидерландах» — но не «Индиго». */
function pattern(name: string, code: string, country: boolean): Pattern {
  let n = norm(name.replace(/\s*\(.*\)$/, ''));
  const stem = /[а-я]/.test(n) && n.length > 4;
  if (stem && /[аяыиьй]$/.test(n)) n = n.slice(0, -1);
  const tail = `${stem ? '(?:[аеиоуыьюяй]|ами|ам|ах|ов|ом|ой|ей|ью|ею|ии)?' : ''}(?!\\p{L})`;
  return { re: new RegExp(`(?:^|[^\\p{L}])(${esc(n)}${tail})`, 'u'), code, country };
}

const PATTERNS: Pattern[] = (() => {
  const seen = new Set<string>();
  const out: [string, string, boolean][] = [];
  const add = (name: string | undefined, code: string, country: boolean) => {
    if (!name || name === code || seen.has(norm(name))) return;
    seen.add(norm(name));
    out.push([name, code, country]);
  };
  for (const code of NAMED) for (const d of [RU, RU_SHORT, EN, EN_SHORT]) add(d?.of(code), code, true);
  for (const [n, code] of ALIASES) add(n, code, true);
  for (const [n, code] of CITIES) add(n, code, false);
  // Длинные раньше коротких: «Южная Корея» прежде «Корея»; страны прежде городов.
  out.sort((a, b) => Number(b[2]) - Number(a[2]) || b[0].length - a[0].length);
  return out.map(([n, code, country]) => pattern(n, code, country));
})();

function byName(text: string): { code: string; at: number; len: number; country: boolean } | null {
  const t = norm(text);
  for (const p of PATTERNS) {
    const m = p.re.exec(t);
    if (m) return { code: p.code, at: m.index + m[0].length - m[1].length, len: m[1].length, country: p.country };
  }
  return null;
}

// ── Протокол ──────────────────────────────────────────────────────────────

/** Протокол ядра → короткая метка и как он назван по-человечески. */
const PROTO: Record<string, [string, string]> = {
  hysteria2: ['HY2', 'Hysteria2'],
  hy2: ['HY2', 'Hysteria2'],
  hysteria: ['HY', 'Hysteria'],
  vless: ['VLESS', 'VLESS'],
  vmess: ['VMess', 'VMess'],
  trojan: ['Trojan', 'Trojan'],
  ss: ['SS', 'Shadowsocks'],
  shadowsocks: ['SS', 'Shadowsocks'],
  ssr: ['SSR', 'ShadowsocksR'],
  tuic: ['TUIC', 'TUIC'],
  wireguard: ['WG', 'WireGuard'],
  anytls: ['AnyTLS', 'AnyTLS'],
  socks5: ['SOCKS5', 'SOCKS5'],
  http: ['HTTP', 'HTTP'],
};

const AWG = /\b(?:AWG|amnezia)/i;

/** Сокращения протоколов в имени — полными словами: «HY2» → «Hysteria2», «AWG3.1» → «AmneziaWG 3.1». */
function humanize(s: string): string {
  return s
    .replace(/\bHY(?:STERIA)?2\b/gi, 'Hysteria2')
    .replace(/\bA(?:MNEZIA)?WG[ -]?(\d+(?:\.\d+)?)\b/gi, 'AmneziaWG $1')
    .replace(/\bAWG\b/gi, 'AmneziaWG')
    .replace(/\bWG\b/g, 'WireGuard')
    .replace(/\bVLESS\b/gi, 'VLESS')
    .replace(/\bVMESS\b/gi, 'VMess');
}

const SEP = /^[\s·•|\-–—_:,/\\]+|[\s·•|\-–—_:,/\\]+$/g;

/** Код в начале имени: «BY · …», «DE-2», «[NL] …», «(US) …». */
const LEAD = /^[[(]?([A-Z]{2})[\])]?(?=$|[\s·•|\-–—_:,./\\\d])/;

/** Окно перерисовывается каждую секунду (таймер, скорость) — разбор одного имени делаем один раз. */
const parsed = new Map<string, ParsedServer>();

/** Разобрать имя сервера: страна, подпись без страны, протокол. */
export function parseServer(name: string, kind: string): ParsedServer {
  const key = `${kind}
${name}`;
  let p = parsed.get(key);
  if (!p) parsed.set(key, (p = parse(name, kind)));
  return p;
}

function parse(name: string, kind: string): ParsedServer {
  // Флаг — два символа regional indicator: 🇧🇾 = B + Y.
  const flag = [...name].filter((ch) => (ch.codePointAt(0) ?? 0) >= 0x1f1e6 && (ch.codePointAt(0) ?? 0) <= 0x1f1ff);
  let code: string | null = flag.length >= 2 ? flag.slice(0, 2).map((ch) => String.fromCharCode((ch.codePointAt(0) ?? 0) - 0x1f1e6 + 65)).join('') : null;
  let rest = name.replace(/[\u{1F1E6}-\u{1F1FF}]/gu, '').replace(SEP, '');

  const lead = LEAD.exec(rest);
  if (lead && isRegion(lead[1]) && (!code || code === lead[1])) {
    code = lead[1];
    rest = rest.slice(lead[0].length).replace(SEP, '');
  }

  const found = byName(rest);
  if (found && (!code || code === found.code)) {
    code = found.code;
    // «Германия · Frankfurt» → «Frankfurt»; город и название в середине имени оставляем.
    if (found.country && found.at === 0) rest = rest.slice(found.len).replace(SEP, '');
  }

  const k = kind.toLowerCase().replace(/\+realm$/, '');
  let [tag, human] = PROTO[k] ?? [kind.toUpperCase(), kind];
  if (k === 'wireguard' && AWG.test(name)) [tag, human] = ['AWG', 'AmneziaWG'];
  const title = humanize(rest);
  return { code, title, tag, named: title.toLowerCase().includes(human.toLowerCase()) };
}

/** Серверы по странам: `pin` — сверху, дальше по алфавиту, «Другие» — в конце. Внутри страны — порядок подписки. */
export function groupByCountry<T extends { name: string; kind: string }>(list: T[], pin?: string | null): CountryGroup<T>[] {
  const map = new Map<string, CountryGroup<T>>();
  for (const server of list) {
    const p = parseServer(server.name, server.kind);
    const key = p.code ?? '';
    let g = map.get(key);
    if (!g) map.set(key, (g = { key, code: p.code, name: countryName(p.code), items: [] }));
    g.items.push({ server, p });
  }
  return [...map.values()].sort(
    (a, b) => Number(b.key === pin) - Number(a.key === pin) || Number(a.key === '') - Number(b.key === '') || a.name.localeCompare(b.name, 'ru'),
  );
}

/** Метки протоколов страны без повторов, в порядке подписки. */
export function groupTags<T>(g: CountryGroup<T>): string[] {
  return [...new Set(g.items.map((i) => i.p.tag))];
}
