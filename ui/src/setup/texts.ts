// Тексты установщика. Пока только русский; английский добавится вместе с переводом окна.

import type { Info, Kind, TaskId } from './api';

export const T = {
  eyebrow: 'Мастер установки',
  welcome: 'Добро пожаловать в kl!ck',
  welcomeText: 'VPN-клиент для Windows на ядре mihomo. Установка займёт меньше минуты и не потребует перезагрузки.',
  folder: 'Папка установки',
  change: 'Изменить',
  needs: 'Потребуется',
  free: 'свободно',
  accept: 'Нажимая «Установить», вы принимаете',
  license: 'лицензию MIT',
  options: 'Параметры',
  install: 'Установить',
  back: 'Назад',
  next: 'Далее',
  cancel: 'Отмена',
  close: 'Закрыть',
  done: 'Готово',
  retry: 'Повторить',
  optionsTitle: 'Параметры установки',
  browse: 'Обзор…',
  desktop: 'Ярлык на рабочем столе',
  desktopSub: 'В меню «Пуск» ярлык будет всегда',
  autostart: 'Запускать вместе с Windows',
  autostartSub: 'Свёрнуто в трей, без окна',
  launch: 'Запустить kl!ck',
  doneSub: 'Найдите его в меню «Пуск» или в трее рядом с часами.',
  maintainTitle: 'kl!ck уже установлен',
  version: 'Версия',
  recommended: 'Рекомендуется',
  uninstallTitle: 'Удалить kl!ck?',
  uninstallText: 'Приложение, служба и сетевой драйвер будут удалены. Активное VPN-подключение разорвётся.',
  wipe: 'Удалить профили, подписки и настройки',
  remove: 'Удалить',
  removed: 'kl!ck удалён',
  removedWipe: 'Приложение и все данные удалены с компьютера.',
  removedKeep: 'Профили и настройки сохранены — они подхватятся при следующей установке.',
  keepGoing: 'Продолжить',
  abort: 'Прервать',
  aborting: 'Отменяем…',
  abortText: 'Уже скопированные файлы будут откачены — система останется в прежнем состоянии.',
  closeTitle: 'Закрыть установщик?',
  closeText: 'kl!ck не будет установлен. Запустить мастер можно в любой момент.',
  rolledBack: 'Всё возвращено как было.',
  notRolledBack: 'Попробуйте ещё раз — установщик доделает начатое.',
  licenseTitle: 'Лицензия',
  licenseNote: 'В состав входят ядро mihomo и база Country.mmdb — они распространяются по GPL-3.0. Тексты всех лицензий лежат в папке программы: resources\\licenses.',
  copyright: '© 2026 vbu00',
  mit: 'Лицензия MIT',
  installer: 'Установщик',
  minimize: 'Свернуть',
};

export const INSTALL_STEPS = ['Приветствие', 'Параметры', 'Установка', 'Готово'];
export const MAINT_STEPS = ['Действие', 'Подтверждение', 'Выполнение', 'Готово'];

export function taskLabel(t: TaskId, info: Info): string {
  switch (t) {
    case 'stop':
      return 'Остановка kl!ck';
    case 'files':
      return 'Распаковка файлов';
    case 'core':
      return `Ядро mihomo ${info.core_version}`.trim();
    case 'old':
      return 'Удаление прежней kl!ck';
    case 'service':
      return 'Служба kl!ck';
    case 'shortcuts':
      return 'Ярлыки и автозапуск';
    case 'stop_service':
      return 'Остановка службы kl!ck';
    case 'unhook':
      return 'Снятие Kill Switch и прокси';
    case 'driver':
      return 'Удаление драйвера Wintun';
    case 'remove':
      return 'Удаление файлов';
    case 'data':
      return 'Профили и настройки';
  }
}

export function progressTitle(kind: Kind): string {
  return { install: 'Устанавливаем kl!ck', update: 'Обновляем kl!ck', reinstall: 'Переустанавливаем kl!ck', uninstall: 'Удаляем kl!ck' }[kind];
}

export function doneTitle(kind: Kind, version: string): string {
  return kind === 'update' ? `Обновлено до ${version}` : kind === 'reinstall' ? 'kl!ck переустановлен' : 'kl!ck установлен';
}

export function failTitle(kind: Kind): string {
  return { install: 'Не удалось установить kl!ck', update: 'Не удалось обновить kl!ck', reinstall: 'Не удалось переустановить kl!ck', uninstall: 'Не удалось удалить kl!ck' }[kind];
}

export function abortTitle(kind: Kind): string {
  return kind === 'uninstall' ? 'Прервать удаление?' : 'Прервать установку?';
}

const ERRORS: Record<string, string> = {
  'path.absolute': 'Укажите полный путь, например C:\\Program Files\\klick',
  'path.long': 'Слишком длинный путь',
  'path.chars': 'В пути есть недопустимые символы',
  'path.drive': 'Такого диска нет',
  'path.system': 'В папку Windows ставить нельзя',
  'path.profile': 'В папку пользователя ставить нельзя: там файлы службы не защищены от подмены',
  'path.file': 'Это файл, а не папка',
  'path.not_empty': 'Папка не пуста — выберите пустую или новую',
  'folder.busy': 'Папку держит другая программа. Закройте её и попробуйте снова.',
  'folder.create': 'Не удалось создать папку.',
  'folder.lock': 'Не удалось защитить папку от изменений.',
  'files.write': 'Не удалось записать файлы.',
  'service.register': 'Не удалось зарегистрировать службу kl!ck.',
  'service.start': 'Служба kl!ck не запустилась.',
  busy: 'Установщик уже работает.',
};

export function errorText(code: string): string {
  return ERRORS[code] ?? code;
}

export function noSpace(need: number, free: number): string {
  return `Не хватает места: нужно ${size(need)}, свободно ${size(free)}`;
}

export const NOTES: Record<string, string> = {
  'note.reboot': 'Часть файлов удалится после перезагрузки.',
};

/** Байты по-человечески: «142 МБ», «1,4 ГБ», «212 ГБ». */
export function size(bytes: number): string {
  const mb = bytes / 1024 ** 2;
  if (mb < 1024) return `${Math.max(1, Math.round(mb))} МБ`;
  const gb = mb / 1024;
  return `${gb < 10 ? gb.toFixed(1).replace('.', ',') : Math.round(gb)} ГБ`;
}

export function oldNotice(old: NonNullable<Info['old']>): { title: string; text: string } {
  const title = old.version ? `Найдена прежняя kl!ck ${old.version}` : 'Найдена прежняя kl!ck';
  const text = old.data
    ? 'Установщик удалит её вместе с настройками — подписки нужно будет добавить заново.'
    : 'Установщик удалит её со всеми следами: службой автозапуска, правилами брандмауэра и прокси.';
  return { title, text };
}

export function actionTexts(info: Info): { key: 'update' | 'reinstall' | 'remove'; label: string; sub: string; rec: boolean }[] {
  const inst = info.installed!;
  const first =
    inst.relation === 'same'
      ? { key: 'reinstall' as const, label: 'Переустановить', sub: 'Восстановить файлы текущей версии', rec: true }
      : inst.relation === 'newer'
        ? { key: 'update' as const, label: `Установить версию ${info.version}`, sub: `Вместо более новой ${inst.version}, настройки сохранятся`, rec: false }
        : { key: 'update' as const, label: `Обновить до ${info.version}`, sub: 'Профили, подписки и настройки сохранятся', rec: true };
  return [first, { key: 'remove', label: 'Удалить kl!ck', sub: 'Убрать приложение с компьютера', rec: false }];
}

/** `C:\ProgramData\klick` → `%ProgramData%\klick`: короче и понятно, где искать. */
export function envPath(path: string): string {
  return path.replace(/^[a-z]:\\ProgramData(?=\\|$)/i, '%ProgramData%');
}
