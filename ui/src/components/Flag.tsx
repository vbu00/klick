// Круглые флаги стран для списка серверов. Эмодзи-флаги Windows не рисует (видны буквы «BY»),
// поэтому флаги свои: упрощённые SVG 30×30 в круге, всё в бандле. Нет флага — кружок с кодом.

import { splitFlag } from '../lib/countries';
import { FLAGS } from '../lib/flags';
import { Icon } from './Icon';

/** Флаг страны в круге; `code` null — «Другие», значок глобуса. */
export function Flag({ code }: { code: string | null }) {
  const art = code ? FLAGS[code] : undefined;
  if (art) {
    return (
      <span className="flag" aria-hidden="true">
        <svg viewBox="0 0 30 30" dangerouslySetInnerHTML={{ __html: art }} />
      </span>
    );
  }
  return (
    <span className="flag none" aria-hidden="true">
      {code ?? <Icon name="globe" size={15} />}
    </span>
  );
}

/** Название с флагом-эмодзи в начале: флаг — своей картинкой, а не буквами, как его рисует Windows. */
export function FlagName({ name }: { name: string }) {
  const { code, rest } = splitFlag(name);
  if (!code || !FLAGS[code]) return <>{name}</>;
  return (
    <>
      <span className="flag inline" aria-hidden="true">
        <svg viewBox="0 0 30 30" dangerouslySetInnerHTML={{ __html: FLAGS[code] }} />
      </span>
      {rest}
    </>
  );
}
