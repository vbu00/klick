// «Настройки»: группы из макета; «Режим подключения» переехал на «Соединение».
// Подэкраны: Kill Switch, «Если сервер недоступен», тема, журнал, «О приложении».

import { useEffect, useState, type ReactNode } from 'react';
import { Sheet } from '../components/Chrome';
import { Back, Row, SectionHead, Seg, Tile, Toggle } from '../components/Controls';
import { Icon } from '../components/Icon';
import { KsInfo } from '../components/KsInfo';
import { ProgramList } from '../components/ProgramList';
import { errorText, exitName, serverDown } from '../lib/i18n';
import { isMac, OS_NAME, TRAY_NAME } from '../lib/platform';
import { plural, programTitle, shortPath } from '../lib/rules';
import { useStore } from '../lib/store';
import { ACCENTS, BASES, PALETTES, THEME_DESC, THEMES, systemDark } from '../lib/theme';
import type { AboutView, Appearance, ErrorInfo, ExitAction, KsProgramView, LogLine, ServerDown, UpdateView } from '../lib/types';

/** «Запускать с Windows»; на Mac — как называет это сама система. */
const autostartTitle = isMac ? 'Открывать при входе в систему' : 'Запускать с Windows';

type Sub = null | 'ks' | 'down' | 'theme' | 'log' | 'about';

const REPO = 'https://github.com/vbu00/klick';

/** Авторы kl!ck; порт на macOS — только в версии для Mac. */
const AUTHORS: [string, string, string][] = [
  ['vbu00', 'Разработка', 'https://github.com/vbu00'],
  ['Dmitriy Medvedev', 'Дизайн интерфейса и логотип', 'https://github.com/aleuuu'],
  ...(isMac ? [['limeflash', 'Порт на macOS', 'https://github.com/limeflash'] as [string, string, string]] : []),
];

/** Скопировать в буфер обмена и сказать об этом. */
function useCopy() {
  const { toast } = useStore();
  return async (text: string, title: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast(title, undefined, 'ok');
    } catch {
      toast('Не удалось скопировать', undefined, 'bad');
    }
  };
}

export function Settings() {
  const { settings, state, nav, consumeNav } = useStore();
  const [sub, setSub] = useState<Sub>(null);

  // Нажали на уведомление: сразу нужный подэкран.
  useEffect(() => {
    if (nav?.target === 'killswitch') setSub('ks');
    else if (nav?.target === 'log') setSub('log');
    else return;
    consumeNav();
  }, [nav, consumeNav]);

  useEffect(() => {
    document.querySelector('main.content')?.scrollTo({ top: 0 });
  }, [sub]);

  if (!settings || !state) return null;
  const back = () => setSub(null);
  return (
    <div className="screen">
      {sub === null ? <Main onSub={setSub} /> : null}
      {sub === 'ks' ? <KillSwitchScreen onBack={back} /> : null}
      {sub === 'down' ? <ServerDownScreen onBack={back} /> : null}
      {sub === 'theme' ? <ThemeScreen onBack={back} /> : null}
      {sub === 'log' ? <LogScreen onBack={back} /> : null}
      {sub === 'about' ? <AboutScreen onBack={back} /> : null}
    </div>
  );
}

function Main({ onSub }: { onSub: (s: Sub) => void }) {
  const store = useStore();
  const { transport, toast } = store;
  const settings = store.settings!;
  const copy = useCopy();
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [about, setAbout] = useState<AboutView | null>(null);
  const [logCount, setLogCount] = useState<number | null>(null);
  const [lang, setLang] = useState(false);
  const [exitSheet, setExitSheet] = useState(false);

  useEffect(() => {
    transport.autostart
      .get()
      .then(setAutostart)
      .catch(() => setAutostart(null));
    transport
      .call<AboutView>('about')
      .then(setAbout)
      .catch(() => undefined);
    transport
      .call<LogLine[]>('log')
      .then((l) => setLogCount(l.length))
      .catch(() => undefined);
  }, [transport]);

  const ks = settings.kill_switch;
  const ksOn = ks.programs.filter((p) => p.enabled).length;
  const a = settings.appearance;
  const themeName = THEMES.find(([k]) => k === a.theme)?.[1] ?? '';
  const themeSummary =
    a.theme === 'custom' ? `${themeName} · ${BASES.find(([k]) => k === a.base)?.[1] ?? ''}` : a.theme === 'system' ? `${themeName} · сейчас ${systemDark() ? 'тёмная' : 'светлая'}` : themeName;
  const port = `127.0.0.1:${about?.mixed_port ?? 7890}`;

  const setAuto = async (on: boolean) => {
    try {
      await transport.autostart.set(on);
      setAutostart(on);
    } catch {
      toast('Не удалось изменить автозапуск', undefined, 'bad');
    }
  };

  const report = async () => {
    const text = await transport.call<string>('report').catch(() => null);
    if (text) await copy(text, 'Отчёт скопирован');
    else toast('Служба не отдала отчёт', undefined, 'bad');
  };

  return (
    <>
      <div className="screen-title">Настройки</div>

      <SectionHead title="Подключение" />
      <div className="card-list">
        <Row title="Kill Switch" sub={ks.enabled ? `Вкл · ${plural(ksOn, ['приложение', 'приложения', 'приложений'])}` : 'Выкл'} onClick={() => onSub('ks')} chevron />
        <Row title="Если сервер недоступен" sub={serverDown[settings.on_server_down].title} onClick={() => onSub('down')} chevron />
        <Row
          wrap
          title="Восстанавливать подключение"
          sub="Если VPN был включён, после перезагрузки он включится сам"
          right={<Toggle label="Восстанавливать подключение" on={settings.restore_on_logon} onChange={(v) => void store.setPrefs({ restore_on_logon: v })} />}
        />
      </div>

      <SectionHead title="Оформление" />
      <div className="card-list">
        <Row title="Тема" sub={themeSummary} onClick={() => onSub('theme')} chevron />
      </div>

      <SectionHead title="Общие" />
      <div className="card-list">
        <Row title={autostartTitle} sub={`Свёрнутым в ${TRAY_NAME}`} right={<Toggle label={autostartTitle} on={!!autostart} disabled={autostart === null} onChange={(v) => void setAuto(v)} />} />
        <Row
          wrap
          title="Обновлять подписки"
          sub="Как просит панель, иначе раз в 12 часов"
          right={<Toggle label="Обновлять подписки" on={settings.auto_update} onChange={(v) => void store.setPrefs({ auto_update: v })} />}
        />
        <Row
          wrap
          title="Уведомлять об обрывах"
          sub={`Уведомление ${OS_NAME}, когда связь пропала и вернулась`}
          right={<Toggle label="Уведомлять об обрывах" on={settings.notify} onChange={(v) => void store.setPrefs({ notify: v })} />}
        />
        <Row title="При выходе из kl!ck" sub={exitName[settings.on_exit]} onClick={() => setExitSheet(true)} chevron />
        <Row title="Язык" sub="Русский" onClick={() => setLang(true)} chevron />
      </div>

      <SectionHead title="Продвинутые" />
      <div className="card-list">
        <Row
          wrap
          title="Порт прокси"
          sub="Для программ, где прокси указывают вручную. Доступен только с этого компьютера."
          right={
            <button className="copy-chip" title="Скопировать адрес" onClick={() => void copy(port, 'Адрес скопирован')}>
              {port}
              <Icon name="copy" size={13} />
            </button>
          }
        />
      </div>

      <SectionHead title="Диагностика" />
      <div className="card-list">
        <Row title="Журнал" sub={logCount == null ? 'Записи службы' : plural(logCount, ['запись', 'записи', 'записей'])} onClick={() => onSub('log')} chevron />
        <Row wrap title="Скопировать отчёт" sub="Версии, режим и журнал — без ссылок и адресов сайтов" onClick={() => void report()} right={<Icon name="copy" size={16} className="row-chev" />} />
        <Row title="Ядро Mihomo" sub={about?.core_version ?? 'узнаю версию…'} right={<span className="status-dot" style={{ background: about?.core_version ? 'var(--accent)' : 'var(--dim)' }} />} />
      </div>

      <div className="card-list mt22">
        <Row title="О приложении" sub={`Версия ${about?.version ?? '…'}`} onClick={() => onSub('about')} chevron />
      </div>

      {lang ? <LanguageSheet onClose={() => setLang(false)} /> : null}
      {exitSheet ? <ExitChoiceSheet onClose={() => setExitSheet(false)} /> : null}
    </>
  );
}

/** Что делать с VPN при выходе из трея: спрашивать или сразу запомненное. */
function ExitChoiceSheet({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const current = store.settings!.on_exit;
  const desc: Record<ExitAction, string> = {
    ask: 'При выходе kl!ck спросит, отключить VPN или оставить его работать.',
    disconnect: 'VPN выключится вместе с окном.',
    keep: 'Окно закроется, а VPN продолжит работать: его держит служба.',
  };
  return (
    <Sheet onClose={onClose}>
      <h3>При выходе из kl!ck</h3>
      <div className="choice-list">
        {(['ask', 'disconnect', 'keep'] as ExitAction[]).map((k) => (
          <button key={k} className={current === k ? 'choice on' : 'choice'} onClick={() => current !== k && void store.setPrefs({ on_exit: k })}>
            <span className="radio-ring">
              <i />
            </span>
            <span className="row-text">
              <span className="choice-title">{exitName[k]}</span>
              <span className="choice-desc">{desc[k]}</span>
            </span>
          </button>
        ))}
      </div>
      <div className="sheet-actions" style={{ marginTop: 16 }}>
        <button onClick={onClose}>Готово</button>
      </div>
    </Sheet>
  );
}

function LanguageSheet({ onClose }: { onClose: () => void }) {
  return (
    <Sheet onClose={onClose}>
      <h3>Язык</h3>
      <div className="choice-list">
        <button className="choice on">
          <span className="radio-ring">
            <i />
          </span>
          <span className="row-text">
            <span className="choice-title">Русский</span>
          </span>
        </button>
        <button className="choice" disabled>
          <span className="radio-ring">
            <i />
          </span>
          <span className="row-text">
            <span className="choice-title">English</span>
            <span className="choice-hint">появится вместе с переводом интерфейса</span>
          </span>
        </button>
      </div>
      <div className="sheet-actions" style={{ marginTop: 16 }}>
        <button onClick={onClose}>Готово</button>
      </div>
    </Sheet>
  );
}

// ── Kill Switch ─────────────────────────────────────────────────────────

function KillSwitchScreen({ onBack }: { onBack: () => void }) {
  const store = useStore();
  const { transport, isRunning } = store;
  const settings = store.settings!;
  const ks = settings.kill_switch;
  const [status, setStatus] = useState<KsProgramView[] | null>(null);
  const [info, setInfo] = useState(false);
  const [picker, setPicker] = useState(false);
  const folders = ks.programs.map((p) => p.folder).join('|');

  useEffect(() => {
    transport
      .call<KsProgramView[]>('kill_switch_status')
      .then(setStatus)
      .catch(() => setStatus(null));
  }, [transport, folders]);

  const enabled = ks.programs.filter((p) => p.enabled).length;
  const missing = (folder: string) => status?.find((s) => s.folder === folder)?.exes === 0;

  return (
    <>
      <Back label="Настройки" onClick={onBack} />
      <div className="title-row">
        <div className="sub-title">Kill Switch</div>
        <button className="info-btn lg" aria-label="Как это работает" title="Как это работает" onClick={() => setInfo(true)}>
          <Icon name="info" size={22} />
        </button>
      </div>

      <div className="ks-main">
        <div className="row-text">
          <div className="ks-title">{ks.enabled ? 'Включён' : 'Выключен'}</div>
          <div className="ks-desc">Если VPN выключен или соединение оборвалось, выбранные программы остаются без интернета — их данные не уйдут напрямую через провайдера.</div>
        </div>
        <Toggle big label="Kill Switch" on={ks.enabled} onChange={(v) => void store.ksSet(v)} />
      </div>

      <div className="sec-head mt22">
        <span className="caps">Защищённые приложения</span>
        <span className="sec-count">{ks.programs.length ? `${enabled} из ${ks.programs.length}` : ''}</span>
      </div>
      <div className={ks.enabled ? 'card-list' : 'card-list dimmed'}>
        {ks.programs.map((p) => {
          const title = store.programNames[p.folder] ?? programTitle(p.folder);
          return (
            <Row
              key={p.folder}
              left={<Tile label={title} />}
              title={title}
              sub={
                missing(p.folder) ? (
                  <span className="warn-text" title={p.folder}>
                    не найдена на диске
                  </span>
                ) : (
                  <span className="mono-sub" title={p.folder}>
                    {shortPath(p.folder)}
                  </span>
                )
              }
              right={
                <>
                  <Toggle label={title} on={p.enabled} onChange={(v) => void store.ksProgram(p.folder, v)} />
                  <button className="icon-x" aria-label={`Убрать ${title}`} title="Убрать" onClick={() => void store.ksRemove(p.folder)}>
                    <Icon name="winClose" size={14} />
                  </button>
                </>
              }
            />
          );
        })}
        <button className="row add-row" onClick={() => setPicker(true)}>
          <Icon name="plus" size={18} />
          Добавить приложение
        </button>
      </div>
      <div className="foot-note">Приложения, не отмеченные здесь, при выключенном VPN работают как обычно — напрямую. Гарантию «ни пакета мимо VPN» даёт только Kill Switch.</div>
      <div className="note-card">
        Пока VPN выключен, провайдер может увидеть, какие сайты пыталась открыть защищённая программа: имена сайтов {OS_NAME} спрашивает сама. Соединения при этом не будет.
      </div>
      {settings.mode === 'sys_proxy' ? (
        <div className="note-card">В режиме системного прокси программа из списка работает, только если сама ходит через прокси, иначе остаётся без сети. Надёжнее — режим VPN (TUN).</div>
      ) : null}

      {info ? <KsInfo vpnOn={isRunning} routing={settings.routing} protectedCount={enabled} onClose={() => setInfo(false)} /> : null}
      {picker ? <KsPicker onClose={() => setPicker(false)} /> : null}
    </>
  );
}

/** Выбор программ для Kill Switch — лист «Выбрать приложение» из макета. */
function KsPicker({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const [selected, setSelected] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const taken = (folder: string) => store.settings?.kill_switch.programs.some((p) => p.folder.toLowerCase() === folder.toLowerCase()) ?? false;

  const add = async (paths: string[]) => {
    if (!paths.length || busy) return;
    setBusy(true);
    const n = await store.ksAdd(paths);
    setBusy(false);
    if (n > 0) {
      store.toast(n === 1 ? 'Программа под защитой' : `Под защитой программ: ${n}`, n === 1 ? 'Без VPN она не выйдет в интернет' : 'Без VPN они не выйдут в интернет', 'ok');
      onClose();
    }
  };

  const browse = async () => {
    const path = await store.transport.pickExe();
    if (path) void add([path]);
  };

  return (
    <Sheet onClose={onClose} tall>
      <div className="sheet-head">
        <div className="row-text">
          <h3>Выбрать приложение</h3>
          <p>Отмеченные программы останутся без интернета, пока VPN выключен.</p>
        </div>
        <button className="sheet-close" aria-label="Закрыть" onClick={onClose}>
          <Icon name="winClose" size={14} />
        </button>
      </div>
      <div className="sheet-body">
        <ProgramList selected={selected} onToggle={(path) => setSelected((prev) => (prev.includes(path) ? prev.filter((x) => x !== path) : [...prev, path]))} taken={taken} />
      </div>
      <div className="sheet-foot">
        <button className="ghost" onClick={() => void browse()}>
          Обзор…
        </button>
        <button className="go" disabled={!selected.length || busy} onClick={() => void add(selected)}>
          {busy ? 'Добавляю…' : selected.length ? `Добавить · ${selected.length}` : 'Добавить'}
        </button>
      </div>
    </Sheet>
  );
}

// ── Если сервер недоступен ──────────────────────────────────────────────

function ServerDownScreen({ onBack }: { onBack: () => void }) {
  const store = useStore();
  const current = store.settings!.on_server_down;
  return (
    <>
      <Back label="Настройки" onClick={onBack} />
      <div className="sub-title">Если сервер недоступен</div>
      <div className="sub-lead">Что делать, когда сервер перестал отвечать. После смены сети и выхода из сна связь проверяется сразу.</div>
      <div className="choice-list">
        {(['reconnect', 'next_working', 'fastest'] as ServerDown[]).map((k) => {
          const d = serverDown[k];
          const on = current === k;
          return (
            <button key={k} className={on ? 'choice on' : 'choice'} onClick={() => !on && void store.setPrefs({ on_server_down: k })}>
              <span className="radio-ring">
                <i />
              </span>
              <span className="row-text">
                <span className="choice-title">{d.title}</span>
                <span className="choice-desc">{d.desc}</span>
                {d.hint ? <span className="choice-hint">{d.hint}</span> : null}
              </span>
            </button>
          );
        })}
      </div>
    </>
  );
}

// ── Тема ────────────────────────────────────────────────────────────────

function ThemeScreen({ onBack }: { onBack: () => void }) {
  const store = useStore();
  const a = store.settings!.appearance;
  const set = (patch: Partial<Appearance>) => void store.setPrefs({ appearance: { ...a, ...patch } });

  return (
    <>
      <Back label="Настройки" onClick={onBack} />
      <div className="sub-title">Оформление</div>
      <div className="theme-grid">
        {THEMES.map(([k, label]) => {
          const p = k === 'light' ? PALETTES.light : k === 'custom' ? PALETTES[a.base] : PALETTES.graphite;
          const sys = k === 'system';
          const on = a.theme === k;
          return (
            <button key={k} className={on ? 'theme-card on' : 'theme-card'} onClick={() => set({ theme: k })}>
              <span className="theme-prev" style={{ background: sys ? 'linear-gradient(135deg,#f5f5f7 50%,#1a1a1d 50%)' : p.win, borderColor: sys ? 'rgba(128,128,128,.3)' : p.line }}>
                <span className="theme-bar">
                  <i style={{ background: k === 'custom' ? a.accent : k === 'light' ? '#1fa34a' : '#30d158' }} />
                  <b style={{ background: sys ? 'rgba(128,128,128,.6)' : p.dim }} />
                </span>
                <span className="theme-fill" style={{ background: sys ? 'rgba(128,128,128,.28)' : p.card }} />
              </span>
              <span className="theme-label">
                <span className="radio-ring">
                  <i />
                </span>
                {label}
              </span>
            </button>
          );
        })}
      </div>
      <div className="hint">{THEME_DESC[a.theme]}</div>
      {a.theme === 'custom' ? (
        <>
          <SectionHead title="Основа" />
          <Seg className="mt10" value={a.base} options={BASES} onChange={(base) => set({ base })} />
          <SectionHead title="Акцент" />
          <div className="swatches">
            {ACCENTS.map((c) => (
              <button
                key={c}
                className="swatch"
                title={c}
                aria-label={`Акцент ${c}`}
                style={{ background: c, boxShadow: `0 0 0 2px var(--card), 0 0 0 4px ${a.accent === c ? c : 'transparent'}` }}
                onClick={() => set({ accent: c })}
              />
            ))}
          </div>
          <div className="foot-note">Акцентом подсвечиваются кнопка подключения, переключатели и статус «подключено».</div>
        </>
      ) : null}
    </>
  );
}

// ── Журнал ──────────────────────────────────────────────────────────────

const LEVEL: Record<string, [string, string]> = {
  error: ['ERR', 'var(--red)'],
  warn: ['WARN', 'var(--orange)'],
  info: ['INFO', 'var(--dim)'],
  debug: ['DBG', 'var(--dim2)'],
  trace: ['TRC', 'var(--dim2)'],
};

const timeOf = (at: number) => new Date(at).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit', second: '2-digit' });

function LogScreen({ onBack }: { onBack: () => void }) {
  const { transport } = useStore();
  const copy = useCopy();
  const [lines, setLines] = useState<LogLine[] | null>(null);

  useEffect(() => {
    let alive = true;
    const load = () =>
      transport
        .call<LogLine[]>('log')
        .then((l) => alive && setLines(l))
        .catch(() => undefined);
    void load();
    const t = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [transport]);

  const newest = (lines ?? []).slice().reverse();
  const text = (lines ?? []).map((l) => `${timeOf(l.at)} ${(LEVEL[l.level] ?? LEVEL.info)[0]} ${l.text}`).join('\n');

  return (
    <>
      <Back label="Настройки" onClick={onBack} />
      <div className="title-row">
        <div className="sub-title">Журнал</div>
        <div className="log-actions">
          <button className="pill-btn" disabled={!lines?.length} onClick={() => void copy(text, 'Журнал скопирован')}>
            <Icon name="copy" size={13} />
            Копировать
          </button>
          <button
            className="pill-btn"
            disabled={!lines?.length}
            onClick={() =>
              void transport
                .call('log_clear')
                .then(() => setLines([]))
                .catch(() => undefined)
            }
          >
            Очистить
          </button>
        </div>
      </div>
      <div className="sub-lead">Короткий журнал службы, новые записи сверху. Без адресов сайтов, хранится только на этом компьютере.</div>
      <div className="log-box">
        {newest.length === 0 ? <div className="log-empty">{lines === null ? 'Загружаю…' : 'Пусто'}</div> : null}
        {newest.map((l, i) => {
          const [label, color] = LEVEL[l.level] ?? LEVEL.info;
          return (
            <div key={`${l.at}-${i}`} className="log-line">
              <span className="log-time">{timeOf(l.at)}</span>
              <span className="log-level" style={{ color }}>
                {label}
              </span>
              <span className="log-text">{l.text}</span>
            </div>
          );
        })}
      </div>
    </>
  );
}

// ── О приложении ────────────────────────────────────────────────────────

function AboutScreen({ onBack }: { onBack: () => void }) {
  const { transport, toast } = useStore();
  const [about, setAbout] = useState<AboutView | null>(null);
  const [checking, setChecking] = useState(false);
  const [sheet, setSheet] = useState<'news' | 'licenses' | null>(null);

  useEffect(() => {
    transport
      .call<AboutView>('about')
      .then(setAbout)
      .catch(() => undefined);
  }, [transport]);

  const check = async () => {
    setChecking(true);
    try {
      const u = await transport.call<UpdateView>('check_update');
      if (u.newer && u.url) {
        const url = u.url;
        toast(`Есть версия ${u.latest}`, 'Скачать можно на GitHub', 'ok', { label: 'Скачать', run: () => void transport.openUrl(url) });
      } else {
        toast('Обновлений нет', `У вас последняя версия ${u.current}`, 'ok');
      }
    } catch (e) {
      const code = (e as ErrorInfo)?.code ?? 'unknown';
      toast(errorText(code), undefined, code === 'update.disabled' ? 'dim' : 'bad');
    }
    setChecking(false);
  };

  return (
    <>
      <Back label="Настройки" onClick={onBack} />
      <div className="about-head">
        <div className="about-logo">
          <Icon name="power" size={32} />
        </div>
        <div className="about-name">kl!ck</div>
        <div className="about-ver">
          Версия {about?.version ?? '…'} · сборка {__BUILD_DATE__}
          {about?.dev ? ' · разработка' : ''}
        </div>
        <button className="pill-btn mt14" disabled={checking} onClick={() => void check()}>
          {checking ? 'Проверяю…' : 'Проверить обновления'}
        </button>
      </div>

      <div className="card-list mt24">
        <KV k="Ядро">Mihomo {about?.core_version ?? '…'}</KV>
        <KV k="Система">{about?.os ?? '…'}</KV>
        <KV k="Папка данных" mono>
          {about?.data_dir ?? '…'}
        </KV>
      </div>

      <SectionHead title="Авторы" />
      <div className="card-list">
        {AUTHORS.map(([name, role, url]) => (
          <Row key={name} title={name} sub={role} onClick={() => void transport.openUrl(url)} chevron />
        ))}
      </div>

      <div className="card-list mt12">
        <Row title="Что нового" onClick={() => setSheet('news')} chevron />
        <Row title="Исходный код" sub="GitHub" onClick={() => void transport.openUrl(REPO)} chevron />
        <Row title="Лицензии открытого ПО" sub="Mihomo · GPL-3.0 и другие" onClick={() => setSheet('licenses')} chevron />
      </div>
      <div className="about-foot">Приложение не предоставляет VPN-серверы — только подключается к тем, что вы добавили.</div>

      {sheet === 'news' ? (
        <Sheet onClose={() => setSheet(null)}>
          <h3>Что нового в {about?.version ?? 'этой версии'}</h3>
          <ul className="sheet-list">
            <li>kl!ck собрана заново: служба с правами системы держит ядро, ключи и Kill Switch, окно только показывает и командует.</li>
            <li>Два режима — VPN (TUN) и системный прокси — и тумблер «VPN для всего / VPN для выбранного» со своим списком в каждом положении.</li>
            <li>Kill Switch по программам, «Как вас видят сайты», «Сейчас в сети» и «Не открывается?».</li>
          </ul>
          <div className="sheet-actions" style={{ marginTop: 16 }}>
            <button onClick={() => setSheet(null)}>Понятно</button>
          </div>
        </Sheet>
      ) : null}
      {sheet === 'licenses' ? (
        <Sheet onClose={() => setSheet(null)}>
          <h3>Лицензии открытого ПО</h3>
          <div className="card-list mt14">
            <KV k="Mihomo — ядро">GPL-3.0</KV>
            <KV k="Tauri — окно">MIT / Apache-2.0</KV>
            <KV k="React — интерфейс">MIT</KV>
            <KV k="Библиотеки Rust">MIT / Apache-2.0</KV>
          </div>
          <p>Ядро mihomo распространяется по GPL-3.0, его исходный код — github.com/MetaCubeX/mihomo.</p>
          <div className="sheet-actions" style={{ marginTop: 16 }}>
            <button onClick={() => void transport.openUrl('https://github.com/MetaCubeX/mihomo')}>Исходный код ядра</button>
            <button className="main" onClick={() => setSheet(null)}>
              Понятно
            </button>
          </div>
        </Sheet>
      ) : null}
    </>
  );
}

function KV({ k, children, mono }: { k: string; children: ReactNode; mono?: boolean }) {
  return (
    <div className="kv-row">
      <span className="kv-k">{k}</span>
      <span className={mono ? 'kv-v mono' : 'kv-v'}>{children}</span>
    </div>
  );
}
