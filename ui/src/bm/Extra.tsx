// Страницы для того, что kl!ck умеет, а в макете своего экрана не имеет: Kill Switch по программам,
// журнал, «О приложении» и «Дополнительно». Собраны из тех же деталей, что и экраны макета.

import { useEffect, useState, type ReactNode } from 'react';
import mark from '../assets/klick-mark.png';
import { errorText, exitName, serverDown } from '../lib/i18n';
import { programTitle, shortPath } from '../lib/rules';
import { useStore } from '../lib/store';
import type { ErrorInfo, ExitAction, KsProgramView, LogLine, ProgramView, ServerDown, UpdateView } from '../lib/types';
import { css } from './css';
import { plural } from './model';
import { coreVersion, useAbout } from './SettingsTab';
import { BackBtn, Cap, Chevron, Group, H1, Letter, Radio, Switch, ToggleRow, tintOf } from './ui';
import { useWin } from './win';

const REPO = 'https://github.com/vbu00/klick';
const MONO = "ui-monospace, 'SF Mono', 'Cascadia Mono', Consolas, monospace";

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

const Pill = ({ children, onClick, accent, disabled }: { children: ReactNode; onClick: () => void; accent?: boolean; disabled?: boolean }) => (
  <button
    onClick={() => !disabled && onClick()}
    aria-disabled={disabled ? 'true' : 'false'}
    className="tp"
    style={{ ...css('flex: none; height: 30px; padding: 0 12px; border: 0; border-radius: 999px; font-size: 12.5px; font-weight: 500; cursor: pointer; font-family: inherit'), background: accent ? 'var(--accent)' : 'var(--fill2)', color: accent ? 'var(--onAccent)' : disabled ? 'var(--text3)' : 'var(--text)' }}
  >
    {children}
  </button>
);

const Text2 = ({ title, sub, mono }: { title: ReactNode; sub?: ReactNode; mono?: boolean }) => (
  <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
    <span style={css('font-size: 14px; line-height: 18px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{title}</span>
    {sub ? <span style={{ ...css('font-size: 12.5px; line-height: 16px; color: var(--text2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis'), fontFamily: mono ? MONO : undefined }}>{sub}</span> : null}
  </span>
);

// ── Kill Switch ──────────────────────────────────────────────────────────

export function KsPage() {
  const store = useStore();
  const win = useWin();
  const { transport, settings, programNames } = store;
  const [status, setStatus] = useState<KsProgramView[] | null>(null);
  const [running, setRunning] = useState<ProgramView[] | null>(null);
  const folders = settings?.kill_switch.programs.map((p) => p.folder).join('|') ?? '';
  useEffect(() => {
    void transport
      .call<KsProgramView[]>('kill_switch_status')
      .then(setStatus)
      .catch(() => setStatus(null));
  }, [transport, folders]);
  useEffect(() => {
    void transport
      .call<ProgramView[]>('programs')
      .then(setRunning)
      .catch(() => setRunning([]));
  }, [transport]);
  if (!settings) return null;
  const ks = settings.kill_switch;
  const taken = (f: string | null) => !!f && ks.programs.some((p) => p.folder.toLowerCase() === f.toLowerCase());
  const free = (running ?? []).filter((p) => p.folder && !taken(p.folder));

  const add = async (path: string) => {
    const n = await store.ksAdd([path]);
    if (n) store.toast('Программа под защитой', undefined, 'ok');
  };
  const browse = async () => {
    const p = await transport.pickExe();
    if (p) await add(p);
  };

  return (
    <div style={css('display: flex; flex-direction: column; animation: pushin .32s cubic-bezier(.2,.7,.2,1)')}>
      <BackBtn label="Настройки" onClick={win.back} />
      <H1>Kill Switch</H1>
      <Group style="margin-top: 24px; padding: 0 16px">
        {[<ToggleRow key="k" title={ks.enabled ? 'Включён' : 'Выключен'} sub="Без VPN выбранные программы остаются без интернета" on={ks.enabled} onChange={(v) => void store.ksSet(v)} />]}
      </Group>

      <div style={css('display: flex; align-items: flex-end; justify-content: space-between; margin-top: 24px; padding: 0 4px')}>
        <span className="cap2">Защищённые программы</span>
        <span style={css('font-size: 13px; color: var(--text2); font-variant-numeric: tabular-nums')}>{ks.programs.length ? ks.programs.filter((p) => p.enabled).length + ' из ' + ks.programs.length : ''}</span>
      </div>
      <div className="sf sq20" style={{ ...css('margin-top: 8px; padding: 0 16px'), opacity: ks.enabled ? 1 : 0.6 }}>
        {ks.programs.map((p, i) => {
          const title = programNames[p.folder] ?? programTitle(p.folder);
          const missing = status?.find((s) => s.folder === p.folder)?.exes === 0;
          return (
            <div key={p.folder}>
              {i > 0 ? <div style={css('height: .4px; margin-left: 44px; background: var(--line)')} /> : null}
              <div className="rrow" style={{ minHeight: 56 }} title={p.folder}>
                <Letter name={title} tint={tintOf(title)} />
                <Text2 title={title} sub={missing ? <span style={css('color: var(--warn)')}>не найдена на диске</span> : shortPath(p.folder)} />
                <Switch label={title} on={p.enabled} onChange={(v) => void store.ksProgram(p.folder, v)} />
                <button onClick={() => void store.ksRemove(p.folder)} aria-label={'Убрать ' + title} className="tp trb" style={css('flex: none; height: 30px; padding: 0 10px; margin-right: -10px; border: 0; border-radius: 999px; font-size: 13.5px; font-weight: 500; font-family: inherit; cursor: pointer')}>
                  Убрать
                </button>
              </div>
            </div>
          );
        })}
        {!ks.programs.length ? (
          <div className="note" style={css('text-align: center; padding: 20px 0')}>
            Пока ни одной — добавьте ниже
          </div>
        ) : null}
      </div>

      <div style={css('display: flex; align-items: flex-end; justify-content: space-between; margin-top: 24px; padding: 0 4px')}>
        <span className="cap2">Запущено сейчас</span>
        <button onClick={() => void browse()} style={css('border: 0; padding: 0; background: transparent; color: var(--accent); font-size: 13px; font-weight: 500; cursor: pointer; font-family: inherit')}>
          Выбрать .exe…
        </button>
      </div>
      <div className="sf sq20" style={css('margin-top: 8px; padding: 0 16px')}>
        {free.map((p, i) => (
          <div key={p.path}>
            {i > 0 ? <div style={css('height: .4px; margin-left: 44px; background: var(--line)')} /> : null}
            <div className="rrow" style={{ minHeight: 56 }} title={p.path}>
              <Letter name={p.name} tint={tintOf(p.name)} />
              <Text2 title={p.name} sub={p.connections + ' ' + plural(p.connections, 'соединение', 'соединения', 'соединений')} />
              <Pill onClick={() => void add(p.path)}>Добавить</Pill>
            </div>
          </div>
        ))}
        {running !== null && !free.length ? (
          <div className="note" style={css('text-align: center; padding: 20px 0')}>
            Нет программ в сети — выберите .exe вручную
          </div>
        ) : null}
        {running === null ? (
          <div className="note" style={css('text-align: center; padding: 20px 0')}>
            Смотрю, кто в сети…
          </div>
        ) : null}
      </div>
      <div className="note" style={css('margin-top: 8px; padding: 0 4px')}>
        Остальные программы без VPN работают как обычно — напрямую. Пока VPN выключен, провайдер может увидеть, какие сайты пыталась открыть защищённая программа, но соединения не будет.
      </div>
      {settings.mode === 'sys_proxy' ? (
        <div className="note" style={css('margin-top: 8px; padding: 0 4px')}>
          В режиме системного прокси программа из списка работает, только если сама ходит через прокси. Надёжнее — VPN (TUN).
        </div>
      ) : null}
    </div>
  );
}

// ── Журнал ───────────────────────────────────────────────────────────────

const LEVEL: Record<string, [string, string]> = {
  error: ['ERR', 'var(--bad)'],
  warn: ['WARN', 'var(--warn)'],
  info: ['INFO', 'var(--text2)'],
  debug: ['DBG', 'var(--text3)'],
  trace: ['TRC', 'var(--text3)'],
};
const timeOf = (at: number) => new Date(at).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit', second: '2-digit' });

export function LogPage() {
  const { transport, toast } = useStore();
  const win = useWin();
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
  const report = async () => {
    const t = await transport.call<string>('report').catch(() => null);
    if (t) await copy(t, 'Отчёт скопирован');
    else toast('Служба не отдала отчёт', undefined, 'bad');
  };
  return (
    <div style={css('display: flex; flex-direction: column; animation: pushin .32s cubic-bezier(.2,.7,.2,1)')}>
      <BackBtn label="Настройки" onClick={win.back} />
      <H1>Журнал</H1>
      <div style={css('margin-top: 8px; font-size: 13.5px; line-height: 19px; color: var(--text2)')}>Короткий журнал службы, новые записи сверху. Без адресов сайтов, хранится только на этом компьютере.</div>
      <div style={css('display: flex; flex-wrap: wrap; gap: 8px; margin-top: 14px')}>
        <Pill accent onClick={() => void report()}>
          Скопировать отчёт
        </Pill>
        <Pill disabled={!lines?.length} onClick={() => void copy(text, 'Журнал скопирован')}>
          Копировать журнал
        </Pill>
        <Pill
          disabled={!lines?.length}
          onClick={() =>
            void transport
              .call('log_clear')
              .then(() => setLines([]))
              .catch(() => undefined)
          }
        >
          Очистить
        </Pill>
      </div>
      <div className="note" style={css('margin-top: 8px; padding: 0 4px')}>
        Отчёт — версии, режим и журнал, без ссылок и адресов сайтов. Его можно отправить разработчикам.
      </div>
      <div className="sf sq20" style={{ ...css('margin-top: 12px; padding: 10px 14px; font-size: 11.5px; line-height: 16px; user-select: text'), fontFamily: MONO }}>
        {!newest.length ? (
          <div className="note" style={css('text-align: center; padding: 14px 0')}>
            {lines === null ? 'Загружаю…' : 'Пусто'}
          </div>
        ) : null}
        {newest.map((l, i) => {
          const [label, color] = LEVEL[l.level] ?? LEVEL.info;
          return (
            <div key={`${l.at}-${i}`} style={css('display: flex; gap: 8px; padding: 3px 0')}>
              <span style={css('flex: none; color: var(--text3)')}>{timeOf(l.at)}</span>
              <span style={{ flex: 'none', width: 32, color }}>{label}</span>
              <span style={css('flex: 1; min-width: 0; word-break: break-word')}>{l.text}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ── О приложении ─────────────────────────────────────────────────────────

function KV({ k, v, mono }: { k: string; v: ReactNode; mono?: boolean }) {
  return (
    <div className="rrow" style={css('padding: 14px 0; min-height: 0')}>
      <span style={css('flex: none; font-size: 14px; line-height: 18px; color: var(--text2)')}>{k}</span>
      <span style={{ ...css('flex: 1; min-width: 0; text-align: right; font-size: 13.5px; line-height: 18px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; user-select: text'), fontFamily: mono ? MONO : undefined }}>{v}</span>
    </div>
  );
}

function LinkRow({ title, sub, onClick }: { title: string; sub?: string; onClick: () => void }) {
  return (
    <button onClick={onClick} className="rrow" style={css('padding: 14px 0; cursor: pointer')}>
      <Text2 title={title} sub={sub} />
      <Chevron />
    </button>
  );
}

export function About() {
  const { transport, toast } = useStore();
  const win = useWin();
  const about = useAbout();
  const [checking, setChecking] = useState(false);
  const check = async () => {
    setChecking(true);
    try {
      const u = await transport.call<UpdateView>('check_update');
      if (u.newer && u.url) {
        const url = u.url;
        toast(`Есть версия ${u.latest}`, undefined, 'ok', { label: 'Скачать', run: () => void transport.openUrl(url) });
      } else toast(`Обновлений нет · ${u.current}`, undefined, 'ok');
    } catch (e) {
      const code = (e as ErrorInfo)?.code ?? 'unknown';
      toast(errorText(code), undefined, code === 'update.disabled' ? 'dim' : 'bad');
    }
    setChecking(false);
  };
  return (
    <div style={css('display: flex; flex-direction: column; animation: pushin .32s cubic-bezier(.2,.7,.2,1)')}>
      <BackBtn label="Настройки" onClick={win.back} />
      <H1>О приложении</H1>
      <div className="sf sq24" style={css('margin-top: 24px; padding: 20px; display: flex; flex-direction: column; align-items: center; text-align: center')}>
        <img src={mark} alt="" draggable={false} style={css('width: 56px; height: 56px; border-radius: 50%; display: block')} />
        <div style={css('margin-top: 12px; font-size: 20px; line-height: 26px; font-weight: 600')}>kl!ck</div>
        <div style={css('margin-top: 2px; font-size: 13px; color: var(--text2); font-variant-numeric: tabular-nums')}>
          Версия {about?.version ?? '…'} · сборка {__BUILD_DATE__}
          {about?.dev ? ' · разработка' : ''}
        </div>
        <button onClick={() => void check()} className="tp" style={{ ...css('margin-top: 14px; height: 36px; padding: 0 18px; border: 0; border-radius: 999px; font-size: 13.5px; font-weight: 600; cursor: pointer; font-family: inherit'), background: checking ? 'var(--fill2)' : 'var(--accent)', color: checking ? 'var(--text2)' : 'var(--onAccent)' }}>
          {checking ? 'Проверяю…' : 'Проверить обновления'}
        </button>
      </div>

      <Group style="margin-top: 12px; padding: 0 16px">
        {[<KV key="c" k="Ядро" v={'mihomo ' + coreVersion(about?.core_version)} />, <KV key="o" k="Система" v={about?.os ?? '…'} />, <KV key="d" k="Папка данных" v={about?.data_dir ?? '…'} mono />]}
      </Group>

      <Cap>Авторы</Cap>
      <Group>
        {[<KV key="a" k="Разработка" v="vbu00" />, <KV key="b" k="Дизайн" v="Dmitriy Medvedev" />, <KV key="c" k="Порты на macOS и iOS" v="limeflash" />]}
      </Group>

      <Cap>Открытый код</Cap>
      <Group>
        {[
          <LinkRow key="r" title="Исходный код" sub="GitHub" onClick={() => void transport.openUrl(REPO)} />,
          <LinkRow key="m" title="Ядро mihomo" sub="GPL-3.0 · github.com/MetaCubeX/mihomo" onClick={() => void transport.openUrl('https://github.com/MetaCubeX/mihomo')} />,
          <KV key="t" k="Tauri — окно" v="MIT / Apache-2.0" />,
          <KV key="re" k="React — интерфейс" v="MIT" />,
          <KV key="i" k="Шрифт Inter" v="OFL-1.1" />,
          <KV key="rs" k="Библиотеки Rust" v="MIT / Apache-2.0" />,
        ]}
      </Group>
      <div className="note" style={css('margin-top: 12px; text-align: center')}>
        Приложение не предоставляет VPN-серверы — только подключается к тем, что вы добавили.
      </div>
    </div>
  );
}

// ── Дополнительно ────────────────────────────────────────────────────────

function Choice<T extends string>({ value, options, onPick }: { value: T; options: [T, string, string][]; onPick: (v: T) => void }) {
  return (
    <div role="radiogroup" style={css('display: flex; flex-direction: column; gap: 8px; margin-top: 8px')}>
      {options.map(([k, t, d]) => {
        const a = value === k;
        return (
          <button key={k} role="radio" aria-checked={a ? 'true' : 'false'} onClick={() => !a && onPick(k)} className="rcard sq20" style={{ background: 'var(--surface)', boxShadow: a ? 'inset 0 0 0 1px var(--frame), 0 2px 10px rgba(0,0,0,.04)' : 'inset 0 0 0 .4px var(--line)' }}>
            <Radio on={a} />
            <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px')}>
              <span style={css('font-size: 15px; line-height: 20px; font-weight: 600')}>{t}</span>
              <span style={css('font-size: 13px; line-height: 17px; color: var(--text2)')}>{d}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}

const EXIT_DESC: Record<ExitAction, string> = {
  ask: 'При выходе kl!ck спросит, отключить VPN или оставить его работать.',
  disconnect: 'VPN выключится вместе с окном.',
  keep: 'Окно закроется, а VPN продолжит работать: его держит служба.',
};

export function More() {
  const store = useStore();
  const win = useWin();
  const copy = useCopy();
  const about = useAbout();
  const settings = store.settings;
  if (!settings) return null;
  const port = `127.0.0.1:${about?.mixed_port ?? 7890}`;
  return (
    <div style={css('display: flex; flex-direction: column; animation: pushin .32s cubic-bezier(.2,.7,.2,1)')}>
      <BackBtn label="Настройки" onClick={win.back} />
      <H1>Дополнительно</H1>

      <Cap>Связь</Cap>
      <Group>
        {[
          <ToggleRow key="r" title="Восстанавливать подключение" sub="После перезагрузки VPN включится сам, если был включён" on={settings.restore_on_logon} onChange={(v) => void store.setPrefs({ restore_on_logon: v })} />,
          <ToggleRow key="n" title="Уведомлять об обрывах" sub="Уведомление Windows, когда связь пропала и вернулась" on={settings.notify} onChange={(v) => void store.setPrefs({ notify: v })} />,
        ]}
      </Group>

      <Cap>Если сервер недоступен</Cap>
      <Choice<ServerDown> value={settings.on_server_down} options={(['reconnect', 'next_working', 'fastest'] as ServerDown[]).map((k) => [k, serverDown[k].title, serverDown[k].desc])} onPick={(k) => void store.setPrefs({ on_server_down: k })} />

      <Cap>При выходе из kl!ck</Cap>
      <Choice<ExitAction> value={settings.on_exit} options={(['ask', 'disconnect', 'keep'] as ExitAction[]).map((k) => [k, exitName[k], EXIT_DESC[k]])} onPick={(k) => void store.setPrefs({ on_exit: k })} />

      <Cap>Порт прокси</Cap>
      <Group>
        {[
          <div key="p" className="rrow" style={css('padding: 14px 0')}>
            <Text2 title={<span style={{ fontFamily: MONO }}>{port}</span>} sub="Для программ, где прокси указывают вручную" />
            <Pill onClick={() => void copy(port, 'Адрес скопирован')}>Скопировать</Pill>
          </div>,
        ]}
      </Group>
      <div className="note" style={css('margin-top: 8px; padding: 0 4px')}>
        Порт доступен только с этого компьютера.
      </div>
    </div>
  );
}
