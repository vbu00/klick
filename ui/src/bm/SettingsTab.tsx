// «Настройки» по макету: Подключение, Приложение, Помощь и строка версий внизу.

import { useEffect, useState } from 'react';
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

      <Cap>Подключение</Cap>
      <Group>
        {[
          <NavRow key="m" title="Способ подключения" sub={tun ? 'Весь трафик компьютера' : 'Только программы, которые слушают Windows'} value={tun ? 'VPN (TUN)' : 'Прокси'} onClick={() => win.openSheet('method')} />,
          <ToggleRow key="k" title="Kill Switch" sub={ksSub} on={ks.enabled} onChange={(v) => (v && !ks.programs.length ? win.open('ks') : void store.ksSet(v))} onOpen={() => win.open('ks')} />,
          <ToggleRow key="a" title="Запускать с Windows" sub="Свёрнутым в трей, с последней подпиской" on={!!autostart} onChange={(v) => void setAuto(v)} />,
          <ToggleRow key="u" title="Обновлять подписки сами" sub="Серверы, лимит и срок — раз в 12 часов" on={settings.auto_update} onChange={(v) => void store.setPrefs({ auto_update: v })} />,
        ]}
      </Group>

      <Cap>Приложение</Cap>
      <Group>
        {[
          <NavRow key="l" title="Оформление" sub={themeName} onClick={() => win.open('look')} />,
          <NavRow key="lang" title="Язык" sub="Интерфейс приложения" value="Русский" onClick={() => toast('Пока только русский')} />,
          <NavRow key="more" title="Дополнительно" sub="Обрывы связи, выход, порт прокси" onClick={() => win.open('more')} />,
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
