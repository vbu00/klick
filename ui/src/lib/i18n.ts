// Тексты интерфейса. Служба присылает коды, окно переводит. Английский добавится отдельным словарём.

import { isMac } from './platform';
import type { ExitAction, Mode, Route, Routing, ServerDown, Target } from './types';

export const modeName: Record<Mode, string> = {
  tun: 'VPN (TUN)',
  sys_proxy: 'Системный прокси',
};

export const routingName: Record<Routing, string> = {
  all_vpn: 'VPN для всего',
  selected: 'VPN для выбранного',
};

/** Кнопки тумблера и заголовки списков. */
export const routingTitle: Record<Routing, string> = {
  all_vpn: 'VPN для всего',
  selected: 'VPN для выбранного',
};

export const routeName: Record<Route, string> = {
  vpn: 'через VPN',
  direct: 'напрямую',
  block: 'блок',
};

export const routeTitle: Record<Route, string> = {
  vpn: 'Через VPN',
  direct: 'Напрямую',
  block: 'Блок',
};

export const targetKind: Record<Target['kind'], string> = {
  service: 'сервис',
  program: 'программа',
  domain: 'сайт',
  ip: 'IP',
};

/** «Если сервер недоступен»: название, пояснение, приписка. */
export const serverDown: Record<ServerDown, { title: string; desc: string; hint?: string }> = {
  reconnect: {
    title: 'Ждать и переподключаться',
    desc: 'Три попытки за 15 секунд, потом уведомление «Сервер не отвечает». Дальше kl!ck проверяет раз в минуту и сам вернёт связь.',
    hint: 'По умолчанию',
  },
  next_working: {
    title: 'Переключиться на следующий рабочий',
    desc: 'Если сервер не ответил, kl!ck перейдёт на следующий сервер подключения, который отвечает.',
  },
  fastest: {
    title: 'Всегда самый быстрый',
    desc: 'kl!ck держится сервера с наименьшей задержкой и переходит на другой, когда тот становится быстрее.',
    hint: 'Сервер может меняться сам',
  },
};

/** «При выходе»: что делать с VPN. */
export const exitName: Record<ExitAction, string> = {
  ask: 'Спрашивать',
  disconnect: 'Отключать VPN',
  keep: 'Оставлять VPN работать',
};

/** Куда ведёт нажатие на уведомление Windows — таблица из навигации. */
export function noticeTarget(code: string): string | undefined {
  if (code === 'vpn.down' || code === 'vpn.restored' || code === 'server.switched') return 'servers';
  if (code.startsWith('sub.')) return 'card';
  if (code === 'neighbors.conflict') return 'neighbors';
  if (code.startsWith('killswitch.')) return 'killswitch';
  if (code === 'vpn.core_crashed') return 'log';
  return undefined;
}

/** Уведомления об обрывах можно выключить в настройках; о подписке, соседях и Kill Switch — нет. */
export const DROP_NOTICES = ['vpn.down', 'vpn.restored', 'server.switched'];

/** Причина из журнала ядра — коротко и понятно. Причина обрыва известна не всегда. */
export function failureReason(error: string): string {
  const e = error.toLowerCase();
  if (e.includes('timeout') || e.includes('deadline')) return 'нет ответа';
  if (e.includes('reset')) return 'сброс соединения';
  if (e.includes('refused')) return 'отказ сервера';
  if (e.includes('no such host') || e.includes('dns')) return 'имя не найдено';
  if (e.includes('eof') || e.includes('closed')) return 'обрыв';
  if (e.includes('unreachable')) return 'адрес недоступен';
  return 'ошибка соединения';
}

type Params = Record<string, unknown> | undefined;
const str = (v: unknown) => (v == null ? '' : String(v));

/** Текст ошибки по коду службы. */
export function errorText(code: string, params?: Params): string {
  switch (code) {
    case 'vpn.no_connection':
      return 'Сначала добавьте подключение';
    case 'core.start_failed':
      return 'Ядро не запустилось. Подробности — в журнале';
    case 'core.crashed':
      return 'Ядро падает при запуске. Подробности — в журнале';
    case 'sub.fetch_failed':
      return params?.reason === 'timeout' ? 'Панель подписки не ответила вовремя' : 'Не удалось скачать подписку';
    case 'sub.http_status':
      return `Панель подписки ответила ошибкой ${str(params?.status)}`;
    case 'sub.not_subscription':
      return 'По ссылке открывается страница сайта, а не подписка';
    case 'sub.unknown_format':
      return 'Формат не распознан';
    case 'sub.no_servers':
      return 'В подписке нет серверов';
    case 'input.unknown_format':
      return 'Ожидается https://… или vless://, vmess://, trojan://, ss://, hysteria2://';
    case 'input.bad_domain':
      return 'Такого домена не бывает';
    case 'input.bad_ip':
      return 'Нужен адрес или подсеть, например 149.154.160.0/20';
    case 'input.bad_path':
      return 'Нужна папка программы';
    case 'input.folder_too_broad':
      return 'Программа лежит в общей папке вроде «Загрузок». Перенесите её в отдельную папку';
    case 'input.unknown_service':
      return 'Такого сервиса нет в каталоге';
    case 'input.empty':
      return 'Введите сайт или IP';
    case 'list.bad_index':
    case 'list.not_found':
      return 'Правило уже удалено';
    case 'killswitch.no_programs':
      return isMac ? 'Программа не найдена' : 'В папке нет программ (.exe)';
    case 'input.bad_appearance':
      return 'Такой темы нет';
    case 'update.disabled':
      return 'Проверка обновлений включится, когда kl!ck опубликуют на GitHub';
    case 'update.unreachable':
      return 'GitHub не ответил. Проверьте интернет';
    case 'update.http_status':
    case 'update.bad_answer':
      return 'Не удалось узнать о новой версии';
    case 'server.not_found':
      return 'Такого сервера больше нет в подписке';
    case 'list.duplicate':
      return 'Уже в списке';
    case 'conn.not_found':
      return 'Подключение не найдено';
    case 'service.stopping':
    case 'service.unreachable':
      return 'Служба kl!ck не отвечает';
    default:
      return 'Что-то пошло не так';
  }
}

export interface NoticeText {
  title: string;
  text?: string;
  tone: 'ok' | 'warn' | 'bad' | 'dim';
}

/** Уведомление службы — в текст всплывающего сообщения. */
export function noticeText(code: string, params?: Params): NoticeText | null {
  switch (code) {
    case 'vpn.down':
      return { title: 'Сервер не отвечает', text: 'Проверяю снова раз в минуту', tone: 'warn' };
    case 'vpn.restored':
      return { title: 'Связь восстановлена', text: str(params?.server) || undefined, tone: 'ok' };
    case 'server.switched':
      return { title: 'Сервер сменён', text: str(params?.server), tone: 'ok' };
    case 'core.restarted':
      return { title: 'Ядро перезапущено', text: 'Соединение восстановлено', tone: 'dim' };
    case 'vpn.core_crashed':
      return { title: 'VPN остановлен', text: 'Ядро не запускается. Подробности — в журнале', tone: 'bad' };
    case 'killswitch.failed':
      return { title: 'Kill Switch не применился', text: 'Нужна служба kl!ck с правами системы', tone: 'bad' };
    case 'killswitch.partial':
      return { title: 'Kill Switch защитил не все программы', tone: 'warn' };
    case 'killswitch.missing':
      return { title: 'Программа из Kill Switch не найдена', text: 'Удалили или переустановили в другую папку? Проверьте список', tone: 'warn' };
    case 'neighbors.conflict': {
      const names = Array.isArray(params?.names) ? (params?.names as string[]).join(', ') : '';
      return { title: `Мешает: ${names}`, text: 'Он перехватывает трафик режима VPN', tone: 'warn' };
    }
    case 'sub.expired':
      return { title: 'Подписка закончилась', text: str(params?.name), tone: 'bad' };
    case 'sub.expiring':
      return { title: 'Подписка скоро закончится', text: str(params?.name), tone: 'warn' };
    case 'sub.traffic_out':
      return { title: 'Трафик подписки закончился', text: str(params?.name), tone: 'bad' };
    default:
      return null;
  }
}
