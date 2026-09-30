/** На какой системе окно. Тексты, пути программ и кнопки окна на Windows и macOS разные.
 *  В браузере для превью систему можно выбрать: `?os=mac` или `?os=win`. */
export type Platform = 'mac' | 'win';

function detect(): Platform {
  if (typeof window === 'undefined') return 'win';
  const asked = new URLSearchParams(window.location.search).get('os');
  if (asked === 'mac' || asked === 'win') return asked;
  return /Mac/i.test(navigator.userAgent) ? 'mac' : 'win';
}

export const PLATFORM: Platform = detect();
export const isMac = PLATFORM === 'mac';
/** Название системы в текстах: «Уведомление Windows», «Повторяет тему macOS». */
export const OS_NAME = isMac ? 'macOS' : 'Windows';
/** Где живёт значок: трей Windows или строка меню macOS. */
export const TRAY_NAME = isMac ? 'строку меню' : 'трей';
