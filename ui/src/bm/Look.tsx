// «Оформление» по макету: тема карточками, основа и акцент.

import { useStore } from '../lib/store';
import type { Appearance, ThemeBase, ThemeMode } from '../lib/types';
import { systemDark } from '../lib/theme';
import { css } from './css';
import { ACCENTS, BASES, BASE_OPTS, THEME_OPTS, accentEntry, baseKey } from './theme';
import { BackBtn, Cap, H1 } from './ui';
import { useWin } from './win';

export function Look() {
  const store = useStore();
  const win = useWin();
  const a = store.settings?.appearance;
  if (!a) return null;
  const set = (patch: Partial<Appearance>) => void store.setPrefs({ appearance: { ...a, ...patch } });
  const k = baseKey(a, systemDark());
  const acc = (base: ThemeBase) => (base === 'light' ? accentEntry(a.accent)[1] : accentEntry(a.accent)[2]);
  const bIdx = Math.max(0, BASE_OPTS.findIndex(([x]) => x === a.base));

  return (
    <div style={css('display: flex; flex-direction: column; animation: pushin .32s cubic-bezier(.2,.7,.2,1)')}>
      <BackBtn label="Настройки" onClick={win.back} />
      <H1>Оформление</H1>
      <div role="radiogroup" aria-label="Тема" style={css('display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; margin-top: 24px')}>
        {THEME_OPTS.map(([key, l]: [ThemeMode, string]) => {
          const sel = a.theme === key;
          let bg: string;
          let card: string;
          let lineC: string;
          let hair: string;
          let dot: string;
          if (key === 'system') {
            bg = 'linear-gradient(135deg, #f5f5f7 0 50%, #1a1a1d 50% 100%)';
            card = 'linear-gradient(135deg, #ffffff 0 50%, #2e2e33 50% 100%)';
            lineC = '#a3a3a8';
            hair = 'rgba(0,0,0,.08)';
            dot = acc('light');
          } else {
            const bk: ThemeBase = key === 'light' ? 'light' : key === 'dark' ? 'graphite' : a.base;
            const B = BASES[bk];
            bg = B.bg;
            card = B.surface;
            lineC = B.text3;
            hair = bk === 'light' ? 'rgba(0,0,0,.1)' : 'rgba(255,255,255,.08)';
            dot = acc(bk);
          }
          return (
            <button
              key={key}
              role="radio"
              aria-checked={sel ? 'true' : 'false'}
              onClick={() => set({ theme: key })}
              className="tp"
              style={{ ...css('padding: 8.4px 8.4px 12px; border: 0; border-radius: 20px; background: var(--surface); color: var(--text); text-align: left; cursor: pointer'), boxShadow: sel ? 'inset 0 0 0 1px var(--frame), 0 2px 10px rgba(0,0,0,.04)' : 'inset 0 0 0 .4px var(--line)' }}
            >
              <span aria-hidden="true" style={{ ...css('position: relative; display: block; height: 88px; border-radius: 12px; overflow: hidden'), background: bg, boxShadow: `inset 0 0 0 .4px ${hair}` }}>
                <span style={{ ...css('position: absolute; left: 12px; top: 12px; width: 16px; height: 16px; border-radius: 50%'), background: dot }} />
                <span style={{ ...css('position: absolute; left: 36px; top: 17.5px; width: 40px; height: 5px; border-radius: 3px'), background: lineC }} />
                <span style={{ ...css('position: absolute; left: 10px; right: 10px; bottom: 10px; height: 32px; border-radius: 10px'), background: card, boxShadow: `inset 0 0 0 .4px ${hair}` }} />
              </span>
              <span style={css('display: flex; align-items: center; gap: 8px; height: 20px; margin-top: 10px; padding-left: 4px')}>
                <span aria-hidden="true" className="rdo" style={{ boxShadow: sel ? 'inset 0 0 0 2px var(--accent)' : 'inset 0 0 0 1.6px var(--text3)' }}>
                  <i style={{ transform: `scale(${sel ? 1 : 0})` }} />
                </span>
                <span style={{ ...css('font-size: 13.5px; line-height: 20px'), fontWeight: sel ? 600 : 500 }}>{l}</span>
              </span>
            </button>
          );
        })}
      </div>
      <div className="note" style={css('margin-top: 12px; padding: 0 4px')}>
        {a.theme === 'custom' ? 'Выберите основу и цвет акцента ниже.' : 'Цвет акцента ниже работает в любой теме. Основа — только для «Своей».'}
      </div>

      <Cap>Основа</Cap>
      <div className="sf sq12" role="radiogroup" aria-label="Основа" style={css('position: relative; display: flex; height: 40px; margin-top: 8px; padding: 4.4px')}>
        <span aria-hidden="true" style={{ ...css('position: absolute; top: 4.4px; bottom: 4.4px; left: 4.4px; width: calc((100% - 8.8px) / 4); border-radius: 9px; background: var(--fill); box-shadow: none; transition: transform .38s cubic-bezier(.32,.72,0,1), box-shadow .25s ease'), transform: `translateX(${bIdx * 100}%)` }} />
        {BASE_OPTS.map(([key, l]) => {
          const on = a.base === key;
          return (
            <button key={key} role="radio" aria-checked={on ? 'true' : 'false'} onClick={() => set({ base: key, theme: 'custom' })} style={{ ...css('position: relative; flex: 1; height: 31.2px; border: 0; border-radius: 9px; background: transparent; font-size: 13px; cursor: pointer; transition: color .2s ease'), color: on ? 'var(--text)' : 'var(--text2)', fontWeight: on ? 600 : 500 }}>
              {l}
            </button>
          );
        })}
      </div>

      <Cap>Акцент</Cap>
      <div className="sf sq20" role="radiogroup" aria-label="Акцент" style={css('display: flex; justify-content: space-between; margin-top: 8px; padding: 16.4px')}>
        {ACCENTS.map(([n, l, d]) => {
          const c = k === 'light' ? l : d;
          const on = accentEntry(a.accent)[0] === n;
          return <button key={n} role="radio" aria-checked={on ? 'true' : 'false'} aria-label={n} onClick={() => set({ accent: d })} className="tp" style={{ ...css('width: 36px; height: 36px; border: 0; border-radius: 50%; padding: 0; cursor: pointer'), background: c, boxShadow: on ? '0 0 0 3px var(--surface), 0 0 0 5px ' + c : 'none' }} />;
        })}
      </div>
      <div className="note" style={css('margin-top: 12px; padding: 0 4px')}>
        Акцентом подсвечиваются кнопка подключения, переключатели и статус «подключено».
      </div>
    </div>
  );
}
