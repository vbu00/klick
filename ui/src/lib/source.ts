// Что вставили в поле «Добавить» или в буфер для трея: подписка, одиночная ссылка, список или конфигурация текстом.
// Служба принимает в add_connection только одну ссылку; список и конфигурацию — как файл (import_file).

import { protocolName } from './format';

const LINK_SCHEMES = ['vless', 'vmess', 'trojan', 'ss', 'ssr', 'socks', 'socks5', 'hysteria', 'hysteria2', 'hy2', 'tuic', 'wireguard', 'wg', 'anytls'];

/** `text` — несколько ссылок или конфигурация целиком: добавляется как файл. */
export type Detect = { kind: 'empty' | 'sub' | 'link' | 'text' | 'bad'; title: string; text: string; color: string; name?: string };

const schemeOf = (s: string) => (s.includes('://') ? s.split('://')[0].toLowerCase() : '');

export function detect(input: string): Detect {
  const s = input.trim();
  if (!s) return { kind: 'empty', title: 'Ожидаем ссылку', text: 'Скопируйте её у провайдера VPN и вставьте выше — тип определится сам.', color: 'var(--dim)' };
  const scheme = schemeOf(s);
  if ((scheme === 'https' || scheme === 'http') && !/\s/.test(s)) {
    return { kind: 'sub', title: 'Ссылка на подписку', text: 'Загрузим список серверов, лимит трафика и срок действия. Будет обновляться автоматически.', color: 'var(--accent)' };
  }
  if (scheme === 'ssconf' && !/\s/.test(s)) {
    return { kind: 'sub', title: 'Ключ доступа Outline', text: 'Загрузим сервер Shadowsocks по ключу и будем обновлять его автоматически.', color: 'var(--accent)' };
  }
  if (LINK_SCHEMES.includes(scheme) && !/\s/.test(s)) {
    return { kind: 'link', title: `Одиночная конфигурация · ${protocolName(scheme === 'hy2' ? 'hysteria2' : scheme)}`, text: 'Прямое соединение с одним сервером. Без лимитов и срока — только адрес и ключ.', color: 'var(--accent)' };
  }
  const lines = s.split(/\s+/);
  if (lines.length > 1 && lines.every((l) => LINK_SCHEMES.includes(schemeOf(l)))) {
    return { kind: 'text', title: `Список серверов · ${lines.length}`, text: 'Добавим одно подключение со всеми этими серверами.', color: 'var(--accent)', name: 'Мои серверы' };
  }
  if (/^[{[]/.test(s) || /^\[Interface\]/m.test(s) || /^proxies:/m.test(s)) {
    return { kind: 'text', title: 'Конфигурация текстом', text: 'Clash / mihomo, sing-box, Xray или WireGuard — возьмём из неё серверы.', color: 'var(--accent)', name: 'Конфигурация' };
  }
  return { kind: 'bad', title: 'Формат не распознан', text: 'Ожидается https://… или vless://, vmess://, trojan://, ss://, hysteria2:// — или конфигурация целиком', color: 'var(--red)' };
}
