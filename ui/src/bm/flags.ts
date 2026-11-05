// Флаги стран квадратиками на градиентах, как в макете (FI, NL, DE, US — оттуда же).
// Страну сервера узнаём из флага-эмодзи, кода страны или названия в имени сервера.

import { FLAG as MOCK } from './paths';

const tri = (a: string, b: string, c: string, dir = '180deg') => `linear-gradient(${dir}, ${a} 0 33.3%, ${b} 33.3% 66.6%, ${c} 66.6%)`;
const two = (a: string, b: string, dir = '180deg') => `linear-gradient(${dir}, ${a} 0 50%, ${b} 50%)`;
const cross = (bg: string, fg: string) =>
  `linear-gradient(90deg, transparent 28%, ${fg} 28% 46%, transparent 46%), linear-gradient(180deg, transparent 39%, ${fg} 39% 61%, transparent 61%), ${bg}`;
const plus = (bg: string, fg: string) =>
  `linear-gradient(90deg, transparent 40%, ${fg} 40% 60%, transparent 60%) center / 60% 60% no-repeat, linear-gradient(180deg, transparent 40%, ${fg} 40% 60%, transparent 60%) center / 60% 60% no-repeat, ${bg}`;
const disc = (bg: string, fg: string) => `radial-gradient(circle at 50% 50%, ${fg} 0 22%, transparent 23%), ${bg}`;

export const FLAG: Record<string, string> = {
  ...MOCK,
  RU: tri('#ffffff', '#0039a6', '#d52b1e'),
  FR: tri('#0055a4', '#ffffff', '#ef4135', '90deg'),
  IT: tri('#009246', '#ffffff', '#ce2b37', '90deg'),
  IE: tri('#169b62', '#ffffff', '#ff883e', '90deg'),
  BE: tri('#000000', '#fdda24', '#ef3340', '90deg'),
  RO: tri('#002b7f', '#fcd116', '#ce1126', '90deg'),
  MD: tri('#0046ae', '#ffd200', '#cc092f', '90deg'),
  AT: tri('#ed2939', '#ffffff', '#ed2939'),
  EE: tri('#0072ce', '#000000', '#ffffff'),
  LT: tri('#fdb913', '#006a44', '#c1272d'),
  LV: tri('#9e3039', '#ffffff', '#9e3039'),
  BG: tri('#ffffff', '#00966e', '#d62612'),
  HU: tri('#ce2939', '#ffffff', '#477050'),
  AM: tri('#d90012', '#0033a0', '#f2a800'),
  RS: tri('#c6363c', '#0c4076', '#ffffff'),
  LU: tri('#ed2939', '#ffffff', '#00a1de'),
  ES: 'linear-gradient(180deg, #aa151b 0 25%, #f1bf00 25% 75%, #aa151b 75%)',
  PL: two('#ffffff', '#dc143c'),
  UA: two('#0057b7', '#ffd700'),
  ID: two('#ce1126', '#ffffff'),
  MC: two('#ce1126', '#ffffff'),
  SE: cross('#006aa7', '#fecc00'),
  NO: 'linear-gradient(90deg, transparent 25%, #00205b 25% 37%, transparent 37%), linear-gradient(180deg, transparent 44%, #00205b 44% 56%, transparent 56%), linear-gradient(90deg, transparent 21%, #ffffff 21% 41%, transparent 41%), linear-gradient(180deg, transparent 36%, #ffffff 36% 64%, transparent 64%), #ba0c2f',
  DK: cross('#c8102e', '#ffffff'),
  IS: cross('#02529c', '#dc1e35'),
  CH: plus('#da291c', '#ffffff'),
  GE: plus('#ffffff', '#ff0000'),
  GB: 'linear-gradient(90deg, transparent 42%, #c8102e 42% 58%, transparent 58%), linear-gradient(180deg, transparent 40%, #c8102e 40% 60%, transparent 60%), linear-gradient(90deg, transparent 36%, #ffffff 36% 64%, transparent 64%), linear-gradient(180deg, transparent 33%, #ffffff 33% 67%, transparent 67%), #012169',
  JP: disc('#ffffff', '#bc002d'),
  KR: disc('#ffffff', '#cd2e3a'),
  BD: disc('#006a4e', '#f42a41'),
  TR: disc('#e30a17', '#ffffff'),
  CA: 'linear-gradient(90deg, #d52b1e 0 25%, #ffffff 25% 75%, #d52b1e 75%)',
  SG: two('#ef3340', '#ffffff'),
  HK: disc('#de2910', '#ffffff'),
  CN: 'radial-gradient(circle at 25% 30%, #ffde00 0 12%, transparent 13%), #de2910',
  VN: disc('#da251d', '#ffff00'),
  KZ: disc('#00afca', '#fec50c'),
  BY: 'linear-gradient(180deg, #c8313e 0 66%, #4aa657 66%)',
  IL: 'linear-gradient(180deg, #ffffff 0 12%, #0038b8 12% 22%, #ffffff 22% 78%, #0038b8 78% 88%, #ffffff 88%)',
  AE: 'linear-gradient(90deg, #ff0000 0 25%, transparent 25%), ' + tri('#00732f', '#ffffff', '#000000'),
  IN: tri('#ff9933', '#ffffff', '#138808'),
  CZ: 'linear-gradient(to bottom right, #11457e 0 35%, transparent 35%), ' + two('#ffffff', '#d7141a'),
  BR: 'radial-gradient(circle, #002776 0 22%, transparent 23%), radial-gradient(ellipse 48% 38% at 50% 50%, #fedf00 0 98%, transparent 100%), #009c3b',
  AU: 'linear-gradient(90deg, #012169 0 45%, transparent 45%) top left / 100% 50% no-repeat, #012169',
  AR: tri('#74acdf', '#ffffff', '#74acdf'),
  MX: tri('#006847', '#ffffff', '#ce1126', '90deg'),
  PT: 'linear-gradient(90deg, #006600 0 40%, #ff0000 40%)',
  GR: 'repeating-linear-gradient(180deg, #0d5eaf 0 11.1%, #ffffff 11.1% 22.2%)',
  CY: disc('#ffffff', '#d57800'),
  MY: 'linear-gradient(90deg, #010066 0 45%, transparent 45%) top left / 100% 54% no-repeat, repeating-linear-gradient(180deg, #cc0001 0 14.3%, #ffffff 14.3% 28.6%)',
  TH: 'linear-gradient(180deg, #a51931 0 16.6%, #f4f5f8 16.6% 33.3%, #2d2a4a 33.3% 66.6%, #f4f5f8 66.6% 83.3%, #a51931 83.3%)',
  ZA: tri('#e03c31', '#007749', '#001489'),
  AZ: tri('#00b5e2', '#ef3340', '#509e2f'),
  UZ: tri('#0099b5', '#ffffff', '#1eb53a'),
  KG: disc('#e8112d', '#ffef00'),
  TW: 'linear-gradient(90deg, #000095 0 50%, transparent 50%) top left / 100% 50% no-repeat, #fe0000',
  SK: tri('#ffffff', '#0b4ea2', '#ee1c25'),
  SI: tri('#ffffff', '#005da4', '#ed1c24'),
  HR: tri('#ff0000', '#ffffff', '#171796'),
  FI: MOCK.FI,
};

const NAMES: [RegExp, string][] = [
  [/финлянд|finland|helsinki|хельсинк/i, 'FI'],
  [/нидерланд|голланд|netherland|holland|amsterdam|амстердам|rotterdam/i, 'NL'],
  [/герман|germany|deutschland|frankfurt|франкфурт|berlin|берлин|nuremberg|falkenstein/i, 'DE'],
  [/сша|америк|usa|united states|new york|нью-йорк|los angeles|chicago|dallas|miami|seattle/i, 'US'],
  [/росси|russia|москв|moscow|санкт|petersburg/i, 'RU'],
  [/великобрит|британ|англи|united kingdom|britain|england|london|лондон|\buk\b/i, 'GB'],
  [/франц|france|paris|париж/i, 'FR'],
  [/польш|poland|warsaw|варшав/i, 'PL'],
  [/швеци|sweden|stockholm|стокгольм/i, 'SE'],
  [/норвег|norway|oslo/i, 'NO'],
  [/дани|denmark|copenhagen/i, 'DK'],
  [/эстони|estonia|tallinn|таллин/i, 'EE'],
  [/латви|latvia|riga|рига/i, 'LV'],
  [/литв|lithuania|vilnius|вильнюс/i, 'LT'],
  [/украин|ukrain|kyiv|kiev|киев/i, 'UA'],
  [/казахст|kazakh|almaty|алматы|astana|астана/i, 'KZ'],
  [/турци|turkey|türkiye|istanbul|стамбул/i, 'TR'],
  [/итали|italy|milan|милан|rome|рим\b/i, 'IT'],
  [/испани|spain|madrid|мадрид/i, 'ES'],
  [/австри|austria|vienna|вена/i, 'AT'],
  [/швейцар|switzerland|zurich|цюрих/i, 'CH'],
  [/япони|japan|tokyo|токио/i, 'JP'],
  [/сингапур|singapore/i, 'SG'],
  [/канад|canada|toronto|торонто/i, 'CA'],
  [/болгар|bulgaria|sofia/i, 'BG'],
  [/румын|romania|bucharest/i, 'RO'],
  [/чехи|czech|prague|праг/i, 'CZ'],
  [/венгри|hungary|budapest/i, 'HU'],
  [/гонконг|hong ?kong/i, 'HK'],
  [/армени|armenia|yerevan|ереван/i, 'AM'],
  [/грузи|georgia|tbilisi|тбилиси/i, 'GE'],
  [/серби|serbia|belgrade|белград/i, 'RS'],
  [/молдав|moldova|chisinau/i, 'MD'],
  [/израил|israel|tel aviv/i, 'IL'],
  [/эмират|uae|emirates|dubai|дубай/i, 'AE'],
  [/инди|india|mumbai/i, 'IN'],
  [/коре|korea|seoul|сеул/i, 'KR'],
  [/ирланд|ireland|dublin|дублин/i, 'IE'],
  [/бельги|belgium|brussels/i, 'BE'],
  [/португал|portugal|lisbon/i, 'PT'],
  [/греци|greece|athens/i, 'GR'],
  [/кипр|cyprus/i, 'CY'],
  [/люксемб|luxembourg/i, 'LU'],
  [/исланд|iceland/i, 'IS'],
  [/австрали|australia|sydney/i, 'AU'],
  [/бразил|brazil|sao paulo/i, 'BR'],
  [/аргентин|argentin/i, 'AR'],
  [/мексик|mexic/i, 'MX'],
  [/тайван|taiwan/i, 'TW'],
  [/вьетнам|vietnam/i, 'VN'],
  [/таиланд|thailand|bangkok/i, 'TH'],
  [/малайзи|malaysia/i, 'MY'],
  [/индонези|indonesia/i, 'ID'],
  [/азербайдж|azerbaijan|baku/i, 'AZ'],
  [/узбекист|uzbekistan|tashkent/i, 'UZ'],
  [/киргиз|кыргыз|kyrgyz/i, 'KG'],
  [/беларус|belarus|minsk|минск/i, 'BY'],
  [/словаки|slovakia/i, 'SK'],
  [/словени|slovenia/i, 'SI'],
  [/хорвати|croatia/i, 'HR'],
  [/китай|china/i, 'CN'],
];

/** Код страны по имени сервера; null — не узнали. */
export function countryOf(name: string | null | undefined): string | null {
  if (!name) return null;
  const ri = [...name].map((ch) => ch.codePointAt(0) ?? 0).filter((c) => c >= 0x1f1e6 && c <= 0x1f1ff);
  if (ri.length >= 2) return String.fromCharCode(ri[0] - 0x1f1e6 + 65, ri[1] - 0x1f1e6 + 65);
  for (const [re, cc] of NAMES) if (re.test(name)) return cc;
  const code = name.match(/(?:^|[\s[(|_-])([A-Z]{2})(?=$|[\s\])|_\d-])/);
  if (code && FLAG[code[1]]) return code[1];
  return null;
}

export function flagOf(name: string | null | undefined): string {
  const cc = countryOf(name);
  return (cc && FLAG[cc]) || 'var(--fill)';
}

/** Имя сервера без флага-эмодзи в начале. */
export function cleanName(name: string): string {
  return name.replace(/[\u{1F1E6}-\u{1F1FF}]{2}/gu, '').replace(/^[\s|·,:-]+/, '').trim() || name;
}

/** Короткое имя для подписи «сейчас …»: страна без города — «Финляндия · Helsinki» → «Финляндия». */
export function shortName(name: string): string {
  return cleanName(name).split(/\s+[·|,—-]\s+/)[0];
}
