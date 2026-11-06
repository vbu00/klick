// «Настройки» по макету: Подключение, Приложение, Помощь и строка версий внизу.

import { useEffect, useState } from 'react';
import { isIos, isMac, TRAY_NAME } from '../lib/platform';
import { useStore } from '../lib/store';
import type { AboutView } from '../lib/types';
import { css } from './css';
import { plural } from './model';
import { BASE_OPTS, THEME_OPTS } from './theme';
import { Cap, Group, H1, NavRow, ToggleRow } from './ui';
import { useWin } from './win';

/** Версии службы и ядра — один раз за жизнь окна. */
let aboutCache: AboutView | null = null;
export function useAbout() {
  const { transport } = useStore();
  const [about, setAbout] = useState<AboutView | null>(aboutCache);
  useEffect(() => {
    if (aboutCache) return;
    void transport
      .call<AboutView>('about')
      .then((a) => {
        aboutCache = a;
        setAbout(a);
      })
      .catch(() => undefined);
  }, [transport]);
  return about;
}

/** Ход обновления в один клик — от службы. */
export function updateStage(p: { stage: 'download' | 'verify' | 'install'; got: number; total: number } | null): string {
  if (!p) return 'Готовлюсь к загрузке…';
  const mb = (b: number) => (b / 1048576).toFixed(1);
  if (p.stage === 'download') return `Скачиваю ${mb(p.got)} из ${mb(p.total)} МБ`;
  if (p.stage === 'verify') return 'Проверяю подпись автора';
  return 'Ставлю — окно закроется и через минуту откроется снова';
}

export const autostartTitle = isMac ? 'Открывать при входе в систему' : 'Запускать с Windows';

export const coreVersion = (v: string | null | undefined) => (v ? (v.startsWith('v') ? v : 'v' + v) : '…');

export function SettingsTab() {
  const store = useStore();
  const win = useWin();
  const { settings, transport, toast } = store;
  const about = useAbout();
  const [autostart, setAutostart] = useState<boolean | null>(null);
  useEffect(() => {
    transport.autostart
      .get()
      .then(setAutostart)
      .catch(() => setAutostart(null));
  }, [transport]);
  if (!settings) return null;

  const tun = settings.mode === 'tun';
  const ks = settings.kill_switch;
  const n = ks.programs.filter((p) => p.enabled).length;
  const a = settings.appearance;
  const baseName = BASE_OPTS.find(([k]) => k === a.base)?.[1] ?? '';
  const themeName = a.theme === 'custom' ? 'Своя · ' + baseName : (THEME_OPTS.find(([k]) => k === a.theme)?.[1] ?? '');
  const ksSub = !n ? 'Выберите программы — без VPN они останутся без сети' : tun ? 'Без VPN — без интернета: ' + n + ' ' + plural(n, 'программа', 'программы', 'программ') : 'В режиме прокси — только программы из списка';

  const setAuto = async (on: boolean) => {
    try {
      await transport.autostart.set(on);
      setAutostart(on);
    } catch {
      toast('Не удалось изменить автозапуск', undefined, 'bad');
    }
  };

  return (
    <div style={css('display: flex; flex-direction: column; animation: tabin .28s cubic-bezier(.2,.7,.2,1)')}>
      <H1>Настройки</H1>
      {store.state?.update ? (
        <div className="sf sq20" role="status" style={css('display: flex; align-items: center; gap: 12px; margin-top: 16px; padding: 14px 16px; animation: fadeUp .4s cubic-bezier(.2,.8,.2,1) both')}>
          <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
            <span style={css('font-size: 14px; line-height: 18px; font-weight: 600')}>Доступна kl!ck {store.state.update}</span>
            <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2)')}>{store.updating ? updateStage(store.updateProgress) : 'Обновление в один клик, настройки и подключения останутся'}</span>
          </span>
          <button onClick={() => !store.updating && void store.installUpdate()} aria-disabled={store.updating ? 'true' : 'false'} className="tp" style={{ ...css('flex: none; height: 32px; padding: 0 14px; border: 0; border-radius: 999px; font-size: 13px; font-weight: 600; cursor: pointer; font-family: inherit'), background: store.updating ? 'var(--fill2)' : 'var(--accent)', color: store.updating ? 'var(--text2)' : 'var(--onAccent)' }}>
            {store.updating ? 'Ставлю…' : 'Обновить'}
          </button>
        </div>
      ) : null}

      <Cap>Подключение</Cap>
      <Group>
        {[
          // iOS: только VPN, Kill Switch по программам и автозапуска там нет.
          isIos ? null : <NavRow key="m" title="Способ подключения" sub={tun ? 'Весь трафик компьютера' : isMac ? 'Только программы, которые слушают macOS' : 'Только программы, которые слушают Windows'} value={tun ? 'VPN (TUN)' : 'Прокси'} onClick={() => win.openSheet('method')} />,
          isIos ? null : <ToggleRow key="k" title="Kill Switch" sub={ksSub} on={ks.enabled} onChange={(v) => (v && !ks.programs.length ? win.open('ks') : void store.ksSet(v))} onOpen={() => win.open('ks')} />,
          isIos ? null : <ToggleRow key="a" title={autostartTitle} sub={`Свёрнутым в ${TRAY_NAME}, с последней подпиской`} on={!!autostart} onChange={(v) => void setAuto(v)} />,
          <ToggleRow key="u" title="Обновлять подписки сами" sub="Серверы, лимит и срок — раз в 12 часов" on={settings.auto_update} onChange={(v) => void store.setPrefs({ auto_update: v })} />,
        ]}
      </Group>

      <Cap>Приложение</Cap>
      <Group>
        {[
          <NavRow key="l" title="Оформление" sub={themeName} onClick={() => win.open('look')} />,
          <NavRow key="lang" title="Язык" sub="Интерфейс приложения" value="Русский" onClick={() => toast('Пока только русский')} />,
          <NavRow key="more" title="Дополнительно" sub={isIos ? 'Обрывы связи, серверы по странам' : 'Обрывы связи, серверы по странам, выход, порт'} onClick={() => win.open('more')} />,
        ]}
      </Group>

      <Cap>Помощь</Cap>
      <Group>
        {[
          <NavRow key="d" title="Диагностика" sub="Серверы, блокировки, утечки" onClick={() => win.open('diag')} />,
          <NavRow key="j" title="Журнал" sub="Для отправки разработчикам" onClick={() => win.open('log')} />,
          <NavRow key="ab" title="О приложении" sub="Версия, лицензии, GitHub" value={about?.version ?? ''} onClick={() => win.open('about')} />,
        ]}
      </Group>
      <div className="note" style={css('margin-top: 16px; text-align: center')}>
        kl!ck {about?.version ?? '…'} · mihomo {coreVersion(about?.core_version)}
      </div>
    </div>
  );
}
