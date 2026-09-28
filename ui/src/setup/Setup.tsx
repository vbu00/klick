// Установщик kl!ck по макету design/installer: слева шаги, справа экраны. Установка
// (Приветствие → Параметры → Установка → Готово) и обслуживание уже стоящей kl!ck
// (Действие → Подтверждение → Выполнение → Готово).

import { useEffect, useRef, useState } from 'react';
import logo from '../assets/logo.png';
import { Icon } from '../components/Icon';
import { expectedTasks, type Api, type Hello, type Kind, type Outcome, type Progress, type Request } from './api';
import {
  abortTitle,
  actionTexts,
  doneTitle,
  envPath,
  errorText,
  failTitle,
  INSTALL_STEPS,
  MAINT_STEPS,
  noSpace,
  NOTES,
  oldNotice,
  progressTitle,
  size,
  T,
  taskLabel,
} from './texts';

type Screen = 'welcome' | 'options' | 'progress' | 'done' | 'maintain' | 'uninstall' | 'removed' | 'failed';
type Action = 'update' | 'reinstall' | 'remove';

const subscribed = new WeakSet<Api>();

const STEP_OF: Record<'install' | 'maintain', Partial<Record<Screen, number>>> = {
  // «Параметров» нет: папка и переключатели — на первом экране; «options» — только если с папкой беда.
  install: { welcome: 0, options: 0, progress: 1, failed: 1, done: 2 },
  maintain: { maintain: 0, uninstall: 1, progress: 2, failed: 2, done: 3, removed: 3 },
};

export function Setup({ api, hello }: { api: Api; hello: Hello }) {
  const info = hello.info;
  const flow = hello.start === 'install' ? 'install' : 'maintain';
  const home: Screen = hello.start === 'install' ? 'welcome' : hello.start === 'uninstall' ? 'uninstall' : 'maintain';
  const actions = info.installed ? actionTexts(info) : [];

  const [screen, setScreen] = useState<Screen>(home);
  const [path, setPath] = useState(info.default_path);
  const [pathError, setPathError] = useState<string | null>(null);
  const [free, setFree] = useState<number | null>(null);
  const [desktop, setDesktop] = useState(true);
  const [autostart, setAutostart] = useState(true);
  const [action, setAction] = useState<Action>(actions[0]?.key ?? 'update');
  const [wipe, setWipe] = useState(false);
  const [keepOld, setKeepOld] = useState(true);
  const [fresh, setFresh] = useState(false);
  const [launch, setLaunch] = useState(true);
  const [kind, setKind] = useState<Kind>('install');
  const [progress, setProgress] = useState<Progress | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [ask, setAsk] = useState<null | 'abort' | 'close'>(null);
  const [aborting, setAborting] = useState(false);
  const [license, setLicense] = useState<{ klick: string; notices: string } | null>(null);
  const request = useRef<Request | null>(null);
  const shown = useSmooth(progress);

  // События Rust-части: подписка одна, а решения — по свежему состоянию через ссылки.
  const onDone = useRef<(o: Outcome) => void>(() => {});
  const onClose = useRef<() => void>(() => {});
  const onProgress = useRef<(p: Progress) => void>(() => {});
  onProgress.current = setProgress;
  useEffect(() => {
    // StrictMode в разработке вызывает эффект дважды — подписываемся один раз.
    if (subscribed.has(api)) return;
    subscribed.add(api);
    api.onProgress((p) => onProgress.current(p));
    api.onDone((o) => onDone.current(o));
    api.onClose(() => onClose.current());
  }, [api]);

  onDone.current = (o) => {
    setAborting(false);
    setAsk(null);
    if (o.cancelled) {
      setProgress(null);
      setScreen(home);
      return;
    }
    setOutcome(o);
    if (!o.ok) {
      setScreen('failed');
      return;
    }
    // Дать полосе дойти до конца, как в макете.
    setProgress((p) => (p ? { ...p, active: p.tasks.length, pct: 100, ceil: 100, cancellable: false } : p));
    setTimeout(() => setScreen(request.current?.kind === 'uninstall' ? 'removed' : 'done'), 450);
  };

  const canAbort = screen === 'progress' && !!progress?.cancellable && shown <= 92 && !aborting;
  onClose.current = () => {
    if (screen === 'progress') {
      if (canAbort) setAsk('abort');
    } else if (screen === 'welcome' || screen === 'options') {
      setAsk('close');
    } else {
      void api.quit();
    }
  };

  // Точка невозврата прошла, пока был открыт вопрос «Прервать?» — вопрос уже не к месту.
  useEffect(() => {
    if (ask === 'abort' && progress && !progress.cancellable) setAsk(null);
  }, [ask, progress]);

  // Свободное место на диске выбранной папки.
  useEffect(() => {
    let live = true;
    const t = setTimeout(() => api.freeSpace(path).then((f) => live && setFree(f)), 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [api, path]);

  function run(req: Request) {
    request.current = req;
    setKind(req.kind);
    setProgress({ tasks: expectedTasks(req, info), active: 0, pct: 0, ceil: 0, cancellable: true });
    setOutcome(null);
    setAsk(null);
    setAborting(false);
    setScreen('progress');
    api.start(req).catch((e) => {
      setOutcome({ ok: false, cancelled: false, error: String(e), detail: null, rolled_back: true, notes: [], path: req.path });
      setScreen('failed');
    });
  }

  async function startInstall() {
    const c = await api.checkPath(path);
    if (!c.ok) {
      setPathError(c.code);
      setScreen('options');
      return;
    }
    const f = await api.freeSpace(c.path);
    if (f !== null && f < info.size) {
      setPathError(noSpace(info.size, f));
      setScreen('options');
      return;
    }
    setPath(c.path);
    setPathError(null);
    run({ kind: 'install', path: c.path, desktop, autostart, wipe: false, keep_old: keepOld });
  }

  async function browse() {
    const picked = await api.pickFolder();
    if (!picked) return;
    // Выбрали общую папку (D:\Games) — kl!ck ляжет в свою папку внутри.
    const c = await api.checkPath(picked);
    const next = !c.ok && c.code === 'path.not_empty' ? `${picked.replace(/\\+$/, '')}\\klick` : picked;
    setPath(next);
    const again = await api.checkPath(next);
    setPathError(again.ok ? null : again.code);
  }

  function doAction() {
    const inst = info.installed!;
    if (action === 'remove') setScreen('uninstall');
    else run({ kind: action, path: inst.path, desktop: false, autostart: false, wipe: fresh });
  }

  async function finish() {
    if (launch && outcome) await api.launch(outcome.path).catch(() => {});
    await api.quit();
  }

  const stepDefs = flow === 'maintain' ? MAINT_STEPS : INSTALL_STEPS;
  const cur = STEP_OF[flow][screen] ?? 0;
  const isRemove = kind === 'uninstall';
  // Поверх более новой версии — это не обновление, а установка своей.
  const titleKind: Kind = kind === 'update' && info.installed?.relation === 'newer' ? 'install' : kind;
  const notes = (outcome?.notes ?? []).map((n) => NOTES[n] ?? n);

  return (
    <div className="installer">
      <aside className="rail" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <img src={logo} alt="" draggable={false} />
          <div>
            <div className="brand-name">kl!ck</div>
            <div className="brand-sub">
              {T.installer} · {info.version}
            </div>
          </div>
        </div>
        <ol className="steps">
          {stepDefs.map((label, i) => {
            const done = i < cur;
            const active = i === cur;
            return (
              <li key={label} className={`step${done ? ' done' : ''}${active ? ' active' : ''}`}>
                <span className="step-dot">{done ? <span className="tick" /> : i + 1}</span>
                <span className="step-label">{label}</span>
              </li>
            );
          })}
        </ol>
        <div className="grow" data-tauri-drag-region />
        <div className="rail-foot">
          {T.copyright}
          <br />
          <br />
          {T.mit}
        </div>
      </aside>

      <main className="pane">
        <header className="bar" data-tauri-drag-region>
          <div className="grow" data-tauri-drag-region />
          <button className="bar-btn" aria-label={T.minimize} onClick={() => api.minimize()}>
            <Icon name="winMin" size={14} />
          </button>
          <button className="bar-btn close" aria-label={T.close} onClick={() => onClose.current()}>
            <Icon name="winClose" size={14} />
          </button>
        </header>

        {screen === 'welcome' && (
          <section className="screen" key="welcome">
            <div className="eyebrow">{T.eyebrow}</div>
            <h1 className="hero">{T.welcome}</h1>
            {/* Нашлась прежняя kl!ck — вместо вступления её плашка: иначе экран не вмещает. */}
            {!info.old && <p className="lead">{T.welcomeText}</p>}
            <div className="card">
              <div className="card-row">
                <div className="grow">
                  <div className="card-cap">{T.folder}</div>
                  <div className="mono ellipsis">{path}</div>
                  <div className="card-cap">
                    {T.needs} {size(info.size)}
                    {free !== null ? ` · ${T.free} ${size(free)}` : ''}
                  </div>
                </div>
                <button className="btn small" onClick={() => void browse()}>
                  {T.change}
                </button>
              </div>
              <div className="hr" />
              <ToggleRow label={T.desktop} sub={T.desktopSub} on={desktop} onToggle={() => setDesktop(!desktop)} />
              <div className="hr" />
              <ToggleRow label={T.autostart} sub={T.autostartSub} on={autostart} onToggle={() => setAutostart(!autostart)} />
            </div>
            {pathError && <div className="path-error">{pathError.startsWith('path.') ? errorText(pathError) : pathError}</div>}
            {info.old && <OldNotice old={info.old} keep={keepOld} onToggle={() => setKeepOld(!keepOld)} />}
            <div className="grow" />
            <div className="foot">
              <div className="grow legal">
                {T.accept}{' '}
                <a
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    void api.licenses().then(setLicense);
                  }}
                >
                  {T.license}
                </a>
              </div>
              <button className="btn primary" onClick={() => void startInstall()}>
                {T.install}
              </button>
            </div>
          </section>
        )}

        {screen === 'options' && (
          <section className="screen" key="options">
            <h2 className="title">{T.optionsTitle}</h2>
            <div className="path-row">
              <input
                className={pathError ? 'path-input bad' : 'path-input'}
                value={path}
                spellCheck={false}
                onChange={(e) => {
                  setPath(e.target.value);
                  setPathError(null);
                }}
                onBlur={() => void api.checkPath(path).then((c) => setPathError(c.ok ? null : c.code))}
              />
              <button className="btn" onClick={() => void browse()}>
                {T.browse}
              </button>
            </div>
            {pathError && <div className="path-error">{pathError.startsWith('path.') ? errorText(pathError) : pathError}</div>}
            <div className="card toggles">
              <ToggleRow label={T.desktop} sub={T.desktopSub} on={desktop} onToggle={() => setDesktop(!desktop)} />
              <div className="hr" />
              <ToggleRow label={T.autostart} sub={T.autostartSub} on={autostart} onToggle={() => setAutostart(!autostart)} />
            </div>
            <div className="grow" />
            <div className="foot end">
              <button className="btn ghost" onClick={() => setScreen('welcome')}>
                {T.back}
              </button>
              <button className="btn primary" onClick={() => void startInstall()}>
                {T.install}
              </button>
            </div>
          </section>
        )}

        {screen === 'progress' && progress && (
          <section className="screen" key="progress">
            <h2 className="title">{progressTitle(titleKind)}</h2>
            <div className="pct">
              <span className="pct-num">{Math.floor(shown)}</span>
              <span className="pct-sign">%</span>
            </div>
            <div className="track">
              <div className={isRemove ? 'fill gray' : 'fill'} style={{ width: `${shown}%` }}>
                <div className="shine" />
              </div>
            </div>
            <ul className="tasks">
              {progress.tasks.map((t, i) => {
                const st = i < progress.active ? 'done' : i === progress.active ? 'active' : 'pending';
                return (
                  <li key={t} className={`task ${st}`}>
                    <span className="task-mark">
                      {st === 'done' ? (
                        <span className={isRemove ? 'task-done gray' : 'task-done'}>
                          <span className="tick small" />
                        </span>
                      ) : st === 'active' ? (
                        <span className="spinner" />
                      ) : (
                        <span className="pending-dot" />
                      )}
                    </span>
                    <span>{taskLabel(t, info)}</span>
                  </li>
                );
              })}
            </ul>
            <div className="grow" />
            <div className="foot end">
              <button className="btn" disabled={!canAbort} onClick={() => setAsk('abort')}>
                {aborting ? T.aborting : T.cancel}
              </button>
            </div>
          </section>
        )}

        {screen === 'done' && (
          <section className="screen" key="done">
            <div className="center">
              <div className="badge-ok">
                <div className="ring" />
                <div className="disc">
                  <span className="tick big" />
                </div>
              </div>
              <h2 className="big-title">{doneTitle(titleKind, info.version)}</h2>
              <p className="center-text">{T.doneSub}</p>
              {!!outcome?.migrated && <p className="note">{T.migrated(outcome.migrated)}</p>}
              {!!outcome?.not_migrated?.length && <p className="note">{T.notMigrated(outcome.not_migrated)}</p>}
              {notes.map((n) => (
                <p key={n} className="note">
                  {n}
                </p>
              ))}
            </div>
            <div className="foot">
              <label className="check-line grow">
                <input type="checkbox" checked={launch} onChange={() => setLaunch(!launch)} />
                <span className="check">{launch && <span className="tick" />}</span>
                {T.launch}
              </label>
              <button className="btn primary wide" onClick={() => void finish()}>
                {T.done}
              </button>
            </div>
          </section>
        )}

        {screen === 'maintain' && info.installed && (
          <section className="screen" key="maintain">
            <h2 className="title">{T.maintainTitle}</h2>
            <div className="subline">
              {T.version} {info.installed.version} · <span className="mono">{info.installed.path}</span>
            </div>
            <div className="actions">
              {actions.map((a) => {
                const on = action === a.key;
                return (
                  <button key={a.key} className={`action${on ? ' on' : ''}${a.key === 'remove' ? ' red' : ''}`} onClick={() => setAction(a.key)}>
                    <span className="radio">
                      <span className="radio-dot" />
                    </span>
                    <span className="grow">
                      <span className="action-head">
                        <span className="action-label">{a.label}</span>
                        {a.rec && <span className="rec">{T.recommended}</span>}
                      </span>
                      <span className="action-sub">{a.sub}</span>
                    </span>
                  </button>
                );
              })}
            </div>
            {action !== 'remove' && (
              <label className="card wipe">
                <input type="checkbox" checked={fresh} onChange={() => setFresh(!fresh)} />
                <span className={fresh ? 'check red on' : 'check red'}>{fresh && <span className="tick" />}</span>
                <span className="grow">
                  <span className="wipe-label">{T.fresh}</span>
                  <span className="wipe-path">{T.freshSub}</span>
                </span>
              </label>
            )}
            <div className="grow" />
            <div className="foot end">
              <button className="btn ghost" onClick={() => void api.quit()}>
                {T.cancel}
              </button>
              <button className="btn primary" onClick={doAction}>
                {T.next}
              </button>
            </div>
          </section>
        )}

        {screen === 'uninstall' && info.installed && (
          <section className="screen" key="uninstall">
            <h2 className="title">{T.uninstallTitle}</h2>
            <p className="lead small">{T.uninstallText}</p>
            <label className="card wipe">
              <input type="checkbox" checked={wipe} onChange={() => setWipe(!wipe)} />
              <span className={wipe ? 'check red on' : 'check red'}>{wipe && <span className="tick" />}</span>
              <span className="grow">
                <span className="wipe-label">{T.wipe}</span>
                <span className="wipe-path mono">{envPath(info.data_path)}</span>
              </span>
            </label>
            <div className="grow" />
            <div className="foot end">
              <button className="btn ghost" onClick={() => setScreen('maintain')}>
                {T.back}
              </button>
              <button className="btn danger" onClick={() => run({ kind: 'uninstall', path: info.installed!.path, desktop: false, autostart: false, wipe })}>
                {T.remove}
              </button>
            </div>
          </section>
        )}

        {screen === 'removed' && (
          <section className="screen" key="removed">
            <div className="center">
              <div className="badge-gray">
                <span className="tick big" />
              </div>
              <h2 className="big-title">{T.removed}</h2>
              <p className="center-text">{request.current?.wipe ? T.removedWipe : T.removedKeep}</p>
              {notes.map((n) => (
                <p key={n} className="note">
                  {n}
                </p>
              ))}
            </div>
            <div className="foot end">
              <button className="btn wide strong" onClick={() => void api.quit()}>
                {T.close}
              </button>
            </div>
          </section>
        )}

        {screen === 'failed' && outcome && (
          <section className="screen" key="failed">
            <div className="center">
              <div className="badge-bad">!</div>
              <h2 className="big-title">{failTitle(titleKind)}</h2>
              <p className="center-text">
                {outcome.error ? errorText(outcome.error) : ''} {outcome.rolled_back ? T.rolledBack : T.notRolledBack}
              </p>
              {outcome.detail && <p className="detail mono">{outcome.detail}</p>}
            </div>
            <div className="foot end">
              <button className="btn ghost" onClick={() => void api.quit()}>
                {T.close}
              </button>
              <button className="btn primary" onClick={() => request.current && run(request.current)}>
                {T.retry}
              </button>
            </div>
          </section>
        )}

        {ask && (
          <div className="scrim">
            <div className="dialog" role="dialog" aria-modal="true">
              <div className="dialog-title">{ask === 'abort' ? abortTitle(kind) : T.closeTitle}</div>
              <div className="dialog-text">{ask === 'abort' ? T.abortText : T.closeText}</div>
              <div className="dialog-foot">
                <button className="btn small2" onClick={() => setAsk(null)}>
                  {T.keepGoing}
                </button>
                <button
                  className="btn small2 danger"
                  onClick={() => {
                    if (ask === 'abort') {
                      setAborting(true);
                      void api.cancel();
                    } else void api.quit();
                    setAsk(null);
                  }}
                >
                  {T.abort}
                </button>
              </div>
            </div>
          </div>
        )}

        {license && (
          <div className="scrim" onClick={() => setLicense(null)}>
            <div className="dialog license" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
              <div className="dialog-title">{T.licenseTitle}</div>
              <pre className="license-text">{license.klick}</pre>
              <div className="dialog-text">{T.licenseNote}</div>
              <div className="dialog-foot">
                <button className="btn small2" onClick={() => setLicense(null)}>
                  {T.close}
                </button>
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

/** Прежняя kl!ck: что с ней будет, и — если у неё есть данные — перенести ли подписки. */
function OldNotice({ old, keep, onToggle }: { old: NonNullable<Hello['info']['old']>; keep: boolean; onToggle: () => void }) {
  const n = oldNotice(old, keep);
  return (
    <div className="notice">
      <Icon name="info" size={16} />
      <div className="grow">
        <b>{n.title}</b>
        <span>{n.text}</span>
      </div>
      {old.data && (
        <button className="notice-switch" role="switch" aria-checked={keep} aria-label={T.keepOld} title={T.keepOldSub} onClick={onToggle}>
          <span className="notice-switch-label">{T.keepOld}</span>
          <span className={keep ? 'switch on' : 'switch'}>
            <span className="knob" />
          </span>
        </button>
      )}
    </div>
  );
}

function ToggleRow({ label, sub, on, onToggle }: { label: string; sub: string; on: boolean; onToggle: () => void }) {
  return (
    <button className="toggle-row" role="switch" aria-checked={on} onClick={onToggle}>
      <span className="grow">
        <span className="toggle-label">{label}</span>
        <span className="toggle-sub">{sub}</span>
      </span>
      <span className={on ? 'switch on' : 'switch'}>
        <span className="knob" />
      </span>
    </button>
  );
}

/** Полоса без рывков: между событиями задача без своего прогресса медленно подползает к концу. */
function useSmooth(p: Progress | null): number {
  const [shown, setShown] = useState(0);
  const st = useRef({ p, at: 0, shown: 0 });
  useEffect(() => {
    st.current.p = p;
    st.current.at = performance.now();
    if (!p || p.pct === 0) {
      st.current.shown = 0;
      setShown(0);
    }
  }, [p]);
  useEffect(() => {
    let raf = 0;
    const tick = () => {
      const { p, at } = st.current;
      if (p) {
        const t = (performance.now() - at) / 1000;
        const creep = p.pct + (p.ceil - p.pct) * 0.85 * (1 - Math.exp(-t / 2.5));
        const target = Math.max(p.pct, creep);
        let s = st.current.shown + (target - st.current.shown) * 0.12;
        if (Math.abs(target - s) < 0.2) s = target;
        if (s > st.current.shown) {
          st.current.shown = s;
          setShown(s);
        }
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);
  return shown;
}
