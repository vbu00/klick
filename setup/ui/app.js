// Установщик kl!ck — экраны по макету и логика переходов.
//
// Первая установка:  Приветствие → Установка → Готово
//   (папка и два переключателя — прямо на приветствии, без «Параметров»)
// Уже установлен:    Действие → [Подтверждение, если удалить] → Выполнение → Готово
//   действие зависит от версий: обновить / переустановить / откатиться, или удалить.
// Отмена — только при первой установке и до последних процентов: всё
// скопированное откатывается. Ошибка — свой экран с «Повторить».
'use strict';

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const CHECK = '<span class="check"></span>';
const mb = (b) => (b >= 1024 ** 3 ? (b / 1024 ** 3).toFixed(0) + ' ГБ' : Math.round(b / 1024 ** 2) + ' МБ');

const st = {
  info: null,
  flow: 'install', // install | maintain
  screen: 'welcome', // welcome | progress | done | error | maintain | confirm | removed
  kind: 'install', // install | update | reinstall | downgrade | remove
  path: '', free: null,
  desktop: true, autostart: true, launch: true, wipe: false,
  action: 'update', pct: 0, stage: 0,
  dialog: null, // close | cancel | license
  cancelling: false, cancelledNote: false, error: '', license: '',
};

// ─────────── Шаги слева ───────────

function stepsFor() {
  if (st.flow === 'install') return [['Приветствие', ['welcome']], ['Установка', ['progress']], ['Готово', ['done', 'error']]];
  const run = { update: 'Обновление', reinstall: 'Переустановка', downgrade: 'Установка', remove: 'Удаление' }[st.action];
  return st.action === 'remove'
    ? [['Действие', ['maintain']], ['Подтверждение', ['confirm']], [run, ['progress']], ['Готово', ['removed', 'error']]]
    : [['Действие', ['maintain']], [run, ['progress']], ['Готово', ['done', 'error']]];
}
function renderSteps() {
  const steps = stepsFor();
  const cur = steps.findIndex(([, s]) => s.includes(st.screen));
  $('steps').innerHTML = steps.map(([label], i) => {
    const cls = i < cur ? 'done' : i === cur ? 'active' : '';
    return `<div class="step ${cls}"><div class="n">${i < cur ? CHECK : i + 1}</div><div class="l">${label}</div></div>`;
  }).join('');
}

// ─────────── Экраны ───────────

function pathProblem() {
  if (st.free == null) return 'Диск недоступен — выберите другую папку';
  if (st.free < st.info.required * 1.2) return `Мало места: нужно ${mb(st.info.required)}, свободно ${mb(st.free)}`;
  return '';
}

function welcome() {
  const problem = pathProblem();
  return `<div class="scr">
    ${st.cancelledNote ? '<div class="eyebrow" style="color:var(--dim)">Установка отменена — всё откачено</div>' : '<div class="eyebrow">Мастер установки</div>'}
    <div class="h1">Добро пожаловать в kl!ck</div>
    <div class="lead">VPN-клиент для Windows на ядре mihomo. Установка займёт меньше минуты и не потребует перезагрузки.</div>
    <div class="card">
      <div class="row">
        <div class="t"><div class="k">Папка установки</div><div class="v mono" title="${esc(st.path)}">${esc(st.path)}</div>
          <div class="v ${problem ? 'err' : ''}" style="font-size:11.5px;color:${problem ? '' : 'var(--dim)'}">${problem || `Нужно ${mb(st.info.required)} · свободно ${mb(st.free)}`}</div></div>
        <button class="small" data-act="pick">Изменить</button>
      </div>
      <div class="div"></div>
      <div class="row click" data-act="toggle" data-k="desktop"><div class="t"><div class="lbl">Ярлык на рабочем столе</div><div class="hint">В меню «Пуск» ярлык будет в любом случае</div></div><div class="tg ${st.desktop ? 'on' : ''}"></div></div>
      <div class="div"></div>
      <div class="row click" data-act="toggle" data-k="autostart"><div class="t"><div class="lbl">Запускать вместе с Windows</div><div class="hint">Свёрнутым в трей, без окна</div></div><div class="tg ${st.autostart ? 'on' : ''}"></div></div>
    </div>
    <div class="grow"></div>
    <div class="foot">
      <div class="note">Нажимая «Установить», вы принимаете <a data-act="license">лицензию MIT</a></div>
      <button class="btn primary" data-act="install" ${problem ? 'disabled' : ''}>Установить</button>
    </div>
  </div>`;
}

const TASKS = {
  install: ['Подготовка', 'Файлы приложения', 'Ядро mihomo и база GeoIP', 'Ярлыки и автозапуск'],
  update: ['Остановка kl!ck и подготовка', 'Файлы приложения', 'Ядро mihomo и база GeoIP', 'Завершение'],
  remove: ['Остановка kl!ck', 'Снятие правил Kill Switch и прокси', 'Удаление файлов', 'Подключения и настройки'],
};
function tasks() {
  if (st.kind === 'remove') return st.wipe ? TASKS.remove : TASKS.remove.slice(0, 3);
  return st.kind === 'install' ? TASKS.install : TASKS.update;
}

function progress() {
  const rm = st.kind === 'remove';
  const v = st.info.version;
  const title = { install: 'Устанавливаем kl!ck', update: `Обновляем до ${v}`, reinstall: 'Переустанавливаем kl!ck', downgrade: `Устанавливаем ${v}`, remove: 'Удаляем kl!ck' }[st.kind];
  const canCancel = st.kind === 'install' && st.pct < 92 && !st.cancelling;
  const list = tasks().map((label, i) => {
    const done = i < st.stage || st.pct >= 100, act = !done && i === st.stage;
    const mark = done ? `<div class="ok ${rm ? 'rm' : ''}">${CHECK}</div>` : act ? '<div class="sp"></div>' : '<div class="dot"></div>';
    return `<div class="task ${done ? 'd' : act ? 'a' : ''}"><div class="m">${mark}</div><span>${label}</span></div>`;
  }).join('');
  return `<div class="scr">
    <div class="h2">${st.cancelling ? 'Отменяем установку…' : title}</div>
    <div class="pct"><b id="pct">${Math.floor(st.pct)}</b><span>%</span></div>
    <div class="track"><div class="fill ${rm ? 'rm' : ''}" id="fill" style="width:${st.pct}%"></div></div>
    <div class="tasks" id="tasks">${list}</div>
    <div class="grow"></div>
    <div class="foot">
      ${st.kind !== 'install' && !rm ? '<div class="note">kl!ck был остановлен на время обновления — после установки его можно сразу запустить.</div>' : ''}
      ${st.kind === 'install' ? `<button class="btn soft" data-act="askCancel" ${canCancel ? '' : 'disabled'}>Отмена</button>` : ''}
    </div>
  </div>`;
}

function done() {
  const v = st.info.version;
  const title = { install: 'kl!ck установлен', update: `Обновлено до ${v}`, reinstall: 'kl!ck переустановлен', downgrade: `Установлена версия ${v}` }[st.kind];
  return `<div class="scr">
    <div class="center">
      <div class="badge"><i></i><b>${CHECK}</b></div>
      <div class="h1">${title}</div>
      <div class="lead">Найдите его в меню «Пуск» или в трее рядом с часами.</div>
    </div>
    <div class="foot">
      <div class="cb" data-act="toggle" data-k="launch"><div class="box ${st.launch ? 'on' : ''}">${st.launch ? CHECK : ''}</div>Запустить kl!ck</div>
      <button class="btn primary" data-act="finish">Готово</button>
    </div>
  </div>`;
}

function error() {
  const rm = st.kind === 'remove';
  return `<div class="scr">
    <div class="center">
      <div class="badge red"><b>!</b></div>
      <div class="h1">${rm ? 'Не получилось удалить' : 'Не получилось установить'}</div>
      <div class="lead">${esc(st.error)}</div>
      <div class="lead" style="font-size:12px;color:var(--dim)">${rm ? 'Файлы и настройки на месте.' : 'Если это первая установка — всё скопированное удалено.'} Повторите, а если не выйдет — перезагрузите компьютер и запустите установщик снова.</div>
    </div>
    <div class="foot">
      <button class="btn" data-act="close">Закрыть</button>
      <button class="btn primary" data-act="retry">Повторить</button>
    </div>
  </div>`;
}

function actions() {
  const i = st.info, v = i.version, cur = i.installed.version;
  const list = i.compare < 0 ? [['update', `Обновить до ${v}`, 'Подключения, подписки и настройки сохранятся', 'Рекомендуется']]
    : i.compare === 0 ? [['reinstall', `Переустановить ${v}`, 'Восстановить файлы этой версии — настройки сохранятся', 'Рекомендуется']]
    : [['downgrade', `Установить ${v} вместо ${cur}`, 'У вас версия новее — откатываться стоит, только если новая сломалась', 'Старее']];
  list.push(['remove', 'Удалить kl!ck', 'Убрать приложение с компьютера', '']);
  return list;
}
function maintain() {
  const i = st.info;
  return `<div class="scr">
    <div class="h2">kl!ck уже установлен</div>
    <div class="sub">Версия ${esc(i.installed.version)} · <span class="mono">${esc(i.installed.path)}</span></div>
    <div class="opts">${actions().map(([k, label, sub, badge]) => `<div class="opt ${st.action === k ? 'on' : ''} ${k === 'remove' ? 'red' : ''}" data-act="pickAction" data-k="${k}">
      <div class="rd"><i></i></div>
      <div style="flex:1;min-width:0"><div class="ot">${label}${badge ? `<span class="rec ${k === 'downgrade' ? 'warn' : ''}">${badge}</span>` : ''}</div><div class="os">${sub}</div></div></div>`).join('')}</div>
    <div class="grow"></div>
    <div class="foot">
      <button class="btn" data-act="close">Отмена</button>
      <button class="btn ${st.action === 'remove' ? 'danger' : 'primary'}" data-act="next">Далее</button>
    </div>
  </div>`;
}

function confirmRemove() {
  return `<div class="scr">
    <div class="h2">Удалить kl!ck?</div>
    <div class="lead">Приложение будет удалено, правила Kill Switch и системный прокси — сняты. Если kl!ck сейчас подключён, соединение разорвётся.</div>
    <div class="wipe" data-act="toggle" data-k="wipe">
      <div class="box red ${st.wipe ? 'on' : ''}">${st.wipe ? CHECK : ''}</div>
      <div style="flex:1;min-width:0"><div style="font-size:13.5px;font-weight:500">Удалить подключения, подписки и настройки</div>
        <div style="font-size:11.5px;color:var(--dim);margin-top:2px" class="mono">%LOCALAPPDATA%\\com.vbu00.klick</div></div>
    </div>
    <div class="grow"></div>
    <div class="foot">
      ${st.info.uninstall ? '<button class="btn" data-act="close">Отмена</button>' : '<button class="btn" data-act="back">Назад</button>'}
      <button class="btn danger" data-act="remove">Удалить</button>
    </div>
  </div>`;
}

function removed() {
  return `<div class="scr">
    <div class="center">
      <div class="badge grey"><b>${CHECK}</b></div>
      <div class="h1">kl!ck удалён</div>
      <div class="lead">${st.wipe ? 'Приложение и все данные удалены с компьютера.' : 'Подключения и настройки сохранены — они подхватятся при следующей установке.'}</div>
    </div>
    <div class="foot"><button class="btn soft" data-act="close" style="font-weight:600;padding:0 24px">Закрыть</button></div>
  </div>`;
}

function dialog() {
  if (!st.dialog) return '';
  if (st.dialog === 'license') {
    return `<div class="ovl" data-act="closeDialog"><div class="dlg wide" data-stop>
      <div class="dt">Лицензия MIT</div>
      <pre>${esc(st.license || 'Загружаю…')}</pre>
      <div class="db"><button class="btn soft" data-act="closeDialog">Понятно</button></div></div></div>`;
  }
  const install = st.dialog === 'cancel';
  return `<div class="ovl"><div class="dlg">
    <div class="dt">${install ? 'Прервать установку?' : 'Закрыть установщик?'}</div>
    <div class="dx">${install ? 'Всё, что уже скопировано, будет удалено — компьютер останется как был.' : 'kl!ck не будет установлен. Запустить установщик можно в любой момент.'}</div>
    <div class="db"><button class="btn soft" data-act="closeDialog">Продолжить</button>
      <button class="btn danger" data-act="${install ? 'abort' : 'quit'}" style="padding:0 14px">${install ? 'Прервать' : 'Закрыть'}</button></div>
  </div></div>`;
}

const SCREENS = { welcome, progress, done, error, maintain, confirm: confirmRemove, removed };
function render() {
  renderSteps();
  $('screen').innerHTML = SCREENS[st.screen]();
  $('dialog').innerHTML = dialog();
}
function go(screen) {
  st.screen = screen;
  render();
}

// Прогресс — точечно, без перерисовки экрана (иначе мигает анимация).
function updateProgress() {
  if (st.screen !== 'progress') return;
  if ($('pct')) $('pct').textContent = Math.floor(st.pct);
  if ($('fill')) $('fill').style.width = st.pct + '%';
  const html = $('screen').querySelector('.tasks');
  if (html) { const fresh = document.createElement('div'); fresh.innerHTML = progress(); html.replaceWith(fresh.querySelector('.tasks')); }
  const cancel = $('screen').querySelector('[data-act=askCancel]');
  if (cancel) cancel.disabled = !(st.pct < 92 && !st.cancelling);
}

// ─────────── Действия ───────────

async function runInstall(kind) {
  st.kind = kind; st.pct = 0; st.stage = 0; st.cancelling = false; st.cancelledNote = false;
  go('progress');
  try {
    await ks.install({ path: st.path, desktop: st.desktop, autostart: st.autostart, kind });
    st.pct = 100;
    // Пока думали над «Прервать?», установка успела закончиться — вопрос снят.
    if (st.dialog === 'cancel') st.dialog = null;
    setTimeout(() => go('done'), 450);
  } catch (e) {
    st.dialog = null;
    if (String(e) === 'cancelled') { st.cancelledNote = true; st.cancelling = false; go('welcome'); return; }
    st.error = String(e);
    go('error');
  }
}

async function runRemove() {
  st.kind = 'remove'; st.pct = 0; st.stage = 0;
  go('progress');
  try {
    await ks.uninstall(st.wipe);
    st.pct = 100;
    setTimeout(() => go('removed'), 450);
  } catch (e) {
    st.error = String(e);
    go('error');
  }
}

const ACTS = {
  pick: async () => {
    const p = await ks.pickFolder(st.path);
    if (!p) return;
    st.path = p;
    st.free = await ks.freeSpace(p);
    render();
  },
  toggle: (el) => { const k = el.dataset.k; st[k] = !st[k]; render(); },
  license: async () => { st.dialog = 'license'; render(); if (!st.license) { st.license = await ks.license(); render(); } },
  install: () => runInstall('install'),
  askCancel: () => { st.dialog = 'cancel'; render(); },
  abort: () => { st.dialog = null; st.cancelling = true; ks.cancel(); render(); },
  closeDialog: () => { st.dialog = null; render(); },
  quit: () => ks.finish(null),
  close: () => ks.finish(null),
  finish: () => ks.finish(st.launch ? st.path : null),
  pickAction: (el) => { st.action = el.dataset.k; render(); },
  next: () => (st.action === 'remove' ? go('confirm') : runInstall(st.action)),
  back: () => go('maintain'),
  remove: runRemove,
  retry: () => (st.kind === 'remove' ? runRemove() : runInstall(st.kind)),
};

document.addEventListener('click', (e) => {
  const el = e.target.closest('[data-act]');
  if (!el) return;
  // Щелчок по самому окну диалога не закрывает его — только по фону.
  if (el.classList.contains('ovl') && e.target.closest('[data-stop]')) return;
  ACTS[el.dataset.act]?.(el, e);
});
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && st.dialog) { st.dialog = null; render(); }
});

// Крестик в углу: что значит «закрыть» — зависит от экрана.
$('closeBtn').onclick = () => {
  if (st.dialog) return;
  if (st.screen === 'welcome') { st.dialog = 'close'; render(); return; }
  if (st.screen === 'progress') {
    if (st.kind === 'install' && st.pct < 92 && !st.cancelling) { st.dialog = 'cancel'; render(); }
    return; // обновление и удаление не прерываются — дождаться окончания
  }
  ks.finish(null);
};
$('minBtn').onclick = () => ks.minimize();
document.addEventListener('contextmenu', (e) => e.preventDefault());

ks.onProgress((p) => {
  if (st.cancelling) return;
  st.pct = Math.max(st.pct, p.pct);
  st.stage = Math.max(st.stage, p.stage);
  updateProgress();
});

(async () => {
  st.info = await ks.info();
  $('ver').textContent = `Установщик · ${st.info.version}`;
  st.path = st.info.defaultPath;
  st.free = st.info.free;
  if (st.info.installed) {
    st.flow = 'maintain';
    st.action = actions()[0][0];
    if (st.info.uninstall) { st.action = 'remove'; st.screen = 'confirm'; } else st.screen = 'maintain';
  }
  render();
})();
