/** На какой системе окно. Тексты, пути программ и кнопки окна на Windows, macOS и iOS разные.
 *  В браузере для превью систему можно выбрать: `?os=mac`, `?os=win` или `?os=ios`. */
export type Platform = 'mac' | 'win' | 'ios';

function detect(): Platform {
  if (typeof window === 'undefined') return 'win';
  const asked = new URLSearchParams(window.location.search).get('os');
  if (asked === 'mac' || asked === 'win' || asked === 'ios') return asked;
  const ua = navigator.userAgent;
  // iOS раньше mac: в UA iPhone есть «Mac OS X», а iPadOS в режиме «сайт для компьютера» зовётся Macintosh.
  if (/iPhone|iPad|iPod/i.test(ua) || (/Macintosh/i.test(ua) && navigator.maxTouchPoints > 1)) return 'ios';
  return /Mac/i.test(ua) ? 'mac' : 'win';
}

export const PLATFORM: Platform = detect();
export const isMac = PLATFORM === 'mac';
export const isIos = PLATFORM === 'ios';
/** Название системы в текстах: «Уведомление Windows», «Повторяет тему macOS». */
export const OS_NAME = isIos ? 'iOS' : isMac ? 'macOS' : 'Windows';
/** Где живёт значок: трей Windows или строка меню macOS. На iOS значка нет. */
export const TRAY_NAME = isMac ? 'строку меню' : 'трей';
