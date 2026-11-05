// Главная по макету: скорость, «+», время подключения, кнопка питания, положение маршрутов,
// подписки вкладками и карточка подписки с серверами.

import { useEffect, useRef, useState } from 'react';
import { errorText, routingName } from '../lib/i18n';
import { protocolName } from '../lib/format';
import { useStore } from '../lib/store';
import type { Connection, ServerView } from '../lib/types';
import { Field, type FieldMode } from './Field';
import { css } from './css';
import { cleanName, flagOf, shortName } from './flags';
import { autoSet, best, dayMonth, fmtClock, gbText, gbTotal, liveIdOf, mbit, plural, pingColor, pingText, setAuto, subStatus } from './model';
import { MI } from './paths';
import { useWin } from './win';

const W = 'M12 8v5M12 16.5h.01M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z';
const POWER = 'M18.75 20V7.5a1.25 1.25 0 1 1 2.5 0V20a1.25 1.25 0 1 1-2.5 0zm9.43-12.3a1.25 1.25 0 0 0-1.36 2.1C30.43 12.15 32.5 15.87 32.5 20a12.5 12.5 0 0 1-25 0c0-4.13 2.07-7.85 5.68-10.2a1.25 1.25 0 0 0-1.36-2.1C7.48 10.53 5 15.01 5 20a15 15 0 0 0 30 0c0-4.99-2.48-9.47-6.82-12.3z';

function useNow(on: boolean) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!on) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [on]);
  return now;
}

interface Banner {
  t: string;
  x: string;
  ic: string;
  a: string;
  a2: string;
  go: () => void;
  go2: () => void;
  tone?: 'bad' | 'warn';
}

export function Home({ dark }: { dark: boolean }) {
  const store = useStore();
  const win = useWin();
  const { state, settings, servers, pinging, traffic, tested, neighbors, browserProxy, transport, toast } = store;
  const [disc, setDisc] = useState(false);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [menuOpen, setMenuOpen] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [auto, setAutoState] = useState(autoSet());
  const [scan, setScan] = useState(0);
  const [fresh, setFresh] = useState(false);
  const wasPinging = useRef(false);
  const vpn = state?.vpn ?? 'off';
  const now = useNow(vpn !== 'off');

  useEffect(() => {
    if (vpn === 'off' || vpn === 'error') setDisc(false);
  }, [vpn]);
  useEffect(() => {
    if (!disc) return;
    const t = setTimeout(() => setDisc(false), 4000);
    return () => clearTimeout(t);
  }, [disc]);

  // Пока идёт замер — бегущая подсветка по строкам; после — значения «выпрыгивают».
  useEffect(() => {
    if (pinging) {
      wasPinging.current = true;
      setScan(0);
      const t = setInterval(() => setScan((x) => x + 1), 170);
      return () => clearInterval(t);
    }
    if (wasPinging.current) {
      wasPinging.current = false;
      setFresh(true);
      const t = setTimeout(() => setFresh(false), 1200);
      return () => clearTimeout(t);
    }
  }, [pinging]);

  // Нажали на уведомление о серверах — раскрыть список.
  const { nav } = store;
  useEffect(() => {
    if (nav?.target === 'servers' && settings?.active_connection) setExpanded((e) => ({ ...e, [settings.active_connection!]: true }));
  }, [nav, settings?.active_connection]);

  if (!state || !settings) return null;
  const conns = settings.connections;
  const activeId = settings.active_connection;
  const act: Connection | undefined = conns.find((c) => c.id === (win.viewId ?? activeId)) ?? conns[0];

  if (!act) return <EmptyHome />;

  const liveId = liveIdOf(state, activeId);
  const isSub = act.kind === 'subscription';
  const multi = act.kind !== 'link';
  const here = liveId === act.id;
  const on = vpn === 'connected';
  const conning = vpn === 'connecting';
  const rec = here && vpn === 'reconnecting';
  const err = here && (vpn === 'error' || vpn === 'server_down');
  const dsc = here && disc;
  const otherLive = !!liveId && liveId !== act.id && vpn !== 'error';
  const status = subStatus(act);
  const inactive = !!status;
  const isActive = act.id === activeId;
  const list: ServerView[] = servers[act.id] ?? [];
  const wasTested = !!tested[act.id];
  const isAuto = !!auto[act.id];
  const selected = list.find((s) => s.selected) ?? list.find((s) => s.name === act.selected_server) ?? list[0];
  const fastest = best(list);
  const sv = isAuto ? fastest ?? selected : selected;
  const exp = multi && (expanded[act.id] ?? true);
  const elapsed = state.since ? (now - state.since * 1000) / 1000 : 0;

  const label = inactive
    ? 'подписка неактивна'
    : here && on
      ? 'время подключения'
      : here && conning
        ? 'подключаемся…'
        : rec
          ? 'связь пропала — переподключаемся…'
          : err
            ? 'не удалось подключиться'
            : dsc
              ? 'отключаемся…'
              : liveId && liveId !== act.id
                ? 'подключена другая подписка'
                : 'не подключено';

  const runPings = () => {
    if (otherLive) {
      toast('Задержку покажем, когда подключитесь к этой подписке');
      return;
    }
    if (!isActive) {
      void store.selectConnection(act.id).then((ok) => ok && void store.testLatency());
      return;
    }
    void store.testLatency();
  };

  const pickFastest = async () => {
    await store.testLatency();
    const s = store.servers[act.id];
    const b = best(s ?? []);
    if (b && !b.selected) await store.selectServer(b.name);
  };

  const togglePower = async () => {
    if (dsc) return;
    if (status) {
      toast(status === 'expired' ? 'Подписка закончилась — продлите у провайдера' : 'Трафик закончился — продлите у провайдера', undefined, 'bad');
      return;
    }
    if (here && conning) {
      setDisc(true);
      await store.disconnect();
      return;
    }
    if (here && (on || rec || vpn === 'server_down')) {
      setDisc(true);
      await store.disconnect();
      return;
    }
    if (otherLive) {
      await store.selectConnection(act.id);
      return;
    }
    if (!isActive && !(await store.selectConnection(act.id))) return;
    await store.connect();
  };

  const askPick = (name: string | 'auto') => {
    const live = here && (on || rec || conning);
    const pick = () => {
      if (name === 'auto') {
        setAuto(act.id, true);
        setAutoState(autoSet());
        void pickFastest();
      } else {
        setAuto(act.id, false);
        setAutoState(autoSet());
        if (!list.find((s) => s.name === name)?.selected) void store.selectServer(name);
      }
    };
    if (name !== 'auto' && !isAuto && sv?.name === name && live) return;
    if (name === 'auto' && isAuto && live) return;
    if (!live && !otherLive) {
      if (!isActive) void store.selectConnection(act.id).then((ok) => ok && pick());
      else pick();
      return;
    }
    const x = list.find((v) => v.name === name);
    const where = name === 'auto' ? 'самому быстрому серверу' : cleanName(x?.name ?? name);
    const cur = conns.find((p) => p.id === liveId);
    if (otherLive) {
      win.confirm({
        title: 'Переключиться на «' + act.name + '»?',
        text: 'Подключение через «' + (cur ? cur.name : 'другую подписку') + '» оборвётся, и kl!ck подключится к ' + where + '. Пара секунд без сети.',
        okLabel: 'Переключить',
        onOk: () => void store.selectConnection(act.id).then((ok) => ok && pick()),
      });
    } else {
      win.confirm({ title: 'Сменить сервер?', text: 'Соединение оборвётся на пару секунд и переподключится к ' + where + '.', okLabel: 'Сменить', onOk: pick });
    }
  };

  const refresh = async () => {
    if (refreshing) return;
    setRefreshing(true);
    await store.refresh(act.id);
    setRefreshing(false);
  };

  // Срок и трафик.
  const info = act.info;
  let expLabel = '';
  let expLeft = '';
  if (info?.expire) {
    const d = info.expire * 1000;
    if (status === 'expired') {
      const ago = Math.max(1, Math.round((Date.now() - d) / 864e5));
      expLabel = 'Истекла ' + dayMonth(info.expire);
      expLeft = ago + ' ' + plural(ago, 'день', 'дня', 'дней') + ' назад';
    } else {
      const left = Math.max(0, Math.round((d - Date.now()) / 864e5));
      expLabel = 'Действует до ' + dayMonth(info.expire);
      expLeft = left + ' ' + plural(left, 'день', 'дня', 'дней');
    }
  }
  const hasTraffic = !!info && info.total > 0;
  const used = info ? info.upload + info.download : 0;
  const pct = hasTraffic ? Math.min(100, (used / info!.total) * 100) : 0;
  const trafColor = pct > 85 ? 'var(--bad)' : pct > 70 ? 'var(--warn)' : 'var(--accent)';
  const showInfo = isSub && (!!info?.expire || hasTraffic);

  // Плашка над вкладками.
  let bn: Banner | null = null;
  const openPage = (what: string) => {
    toast('Страницу подписки панель не сообщает', what === 'support' ? 'Ссылка на поддержку появится, если её даёт панель' : undefined);
  };
  if (status === 'expired') {
    bn = { t: 'Подписка закончилась', x: 'Срок истёк ' + (info?.expire ? dayMonth(info.expire) : '') + '. Продлите у провайдера — серверы вернутся сами.', ic: 'M12 7v5l3 2M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z', a: 'Продлить', a2: 'Обновить', go: () => openPage('page'), go2: () => void refresh() };
  } else if (status === 'traffic') {
    bn = { t: 'Трафик закончился', x: 'Израсходованы все ' + gbTotal(info!.total) + ' ГБ. Лимит обновится с новым периодом — или докупите у провайдера.', ic: W, a: 'Докупить', a2: 'Обновить', go: () => openPage('page'), go2: () => void refresh() };
  } else if (here && vpn === 'server_down' && multi) {
    const bb = fastest && fastest.name !== sv?.name ? fastest : undefined;
    bn = {
      t: 'Сервер не отвечает',
      x: (sv ? cleanName(sv.name) : 'Сервер') + ' не отвечает. ' + (bb ? 'Быстрее всех сейчас ' + shortName(bb.name) + ' · ' + bb.delay + ' мс.' : 'Проверьте задержку и выберите другой сервер.'),
      ic: W,
      a: 'Включить «Авто»',
      a2: 'Повторить',
      go: () => {
        setAuto(act.id, true);
        setAutoState(autoSet());
        void pickFastest();
      },
      go2: () => void store.disconnect().then(() => store.connect()),
    };
  } else if (here && vpn === 'server_down') {
    bn = { t: 'Сервер не отвечает', x: cleanName(sv?.name ?? act.name) + ' не отвечает. Проверьте ключ у провайдера.', ic: W, a: 'Повторить', a2: 'Диагностика', go: () => void store.disconnect().then(() => store.connect()), go2: () => win.open('diag') };
  } else if (here && vpn === 'error') {
    bn = { t: 'VPN не включился', x: errorText(state.error ?? ''), ic: W, a: 'Повторить', a2: 'Диагностика', go: () => void store.connect(), go2: () => win.open('diag') };
  } else if (neighbors.length && state.mode === 'tun' && on) {
    bn = {
      t: neighbors.join(', ') + ' мешает режиму VPN',
      x: 'Он перехватывает трафик всех адаптеров, и часть сайтов через VPN может не открываться. Остановите его или переключитесь на системный прокси.',
      ic: W,
      a: 'Системный прокси',
      a2: 'Понятно',
      go: () => void store.setMode('sys_proxy'),
      go2: () => store.toast('Остановите обход в Klutz или zapret, пока включён VPN'),
      tone: 'warn',
    };
  } else if (browserProxy.length && state.mode === 'sys_proxy' && on) {
    bn = {
      t: 'Браузер идёт мимо kl!ck',
      x: 'Прокси перехватило расширение ' + browserProxy.join(', ') + '. Выключите его или переключитесь на VPN (TUN).',
      ic: W,
      a: 'VPN (TUN)',
      a2: 'Понятно',
      go: () => void store.setMode('tun'),
      go2: () => store.toast('Выключите расширение на странице расширений браузера'),
      tone: 'warn',
    };
  }

  const fieldMode: FieldMode = here && on ? 'on' : here && (conning || vpn === 'reconnecting') ? 'conn' : dsc ? 'disc' : 'off';
  const spinning = (here && conning) || rec;
  const pwBg = here && on ? 'var(--accent)' : 'var(--surface)';
  const pwFg = here && on ? 'var(--onAccent)' : (here && conning) || rec ? 'var(--accent)' : err ? 'var(--bad)' : inactive ? 'var(--text3)' : 'var(--icon)';
  const pwShadow = here && on ? '0 10px 26px color-mix(in srgb, var(--accent) 38%, transparent)' : err ? 'inset 0 0 0 1.5px var(--bad), 0 6px 18px color-mix(in srgb, var(--bad) 18%, transparent)' : 'inset 0 0 0 .4px var(--line)';
  const pwAnim = here && on ? 'halo 2.4s ease-out infinite' : dsc ? 'powerOff .5s cubic-bezier(.2,.7,.2,1)' : 'none';
  const powerLabel = dsc ? 'Отключаемся' : here && (on || conning) ? 'Отключить' : liveId && liveId !== act.id ? 'Переключиться на эту подписку' : 'Подключить';
  const labelColor = (here && conning) || rec ? 'var(--warn)' : err || inactive ? 'var(--bad)' : 'var(--text2)';
  const timer = fmtClock(here && (on || dsc || rec) ? elapsed : 0);
  const down = on ? mbit(traffic.down).toFixed(1) + ' Мб/с' : '0 Мб/с';
  const up = on ? mbit(traffic.up).toFixed(1) + ' Мб/с' : '0 Мб/с';

  // Строки серверов.
  type Row = { key: string; name: string; sub: string; op: number; checked: boolean; radio: boolean; flag?: string; ping: string; pingColor: string; loading: boolean; fresh: boolean; scanning: boolean; pick: () => void };
  const rowState = (i: number, delay: number | null | undefined) => {
    if (otherLive) return { ping: '—', pingColor: 'var(--text3)', loading: false, fresh: false, scanning: false };
    return {
      ping: pingText(delay, wasTested),
      pingColor: pingColor(delay, wasTested),
      loading: pinging && isActive,
      fresh: fresh && isActive,
      scanning: pinging && isActive && i === scan % Math.max(1, list.length + 1),
    };
  };
  let rows: Row[] = [];
  if (multi) {
    rows.push({ key: 'auto', name: 'Авто', sub: 'Самый быстрый · сейчас ' + (fastest ? shortName(fastest.name) : '—'), op: 1, checked: isAuto, radio: true, pick: () => askPick('auto'), ...rowState(0, fastest?.delay) });
    const alive = list.filter((x) => x.delay != null).sort((a, c) => (a.delay ?? 0) - (c.delay ?? 0));
    const rest = list.filter((x) => x.delay == null);
    [...alive, ...rest].forEach((x, i) => {
      rows.push({ key: x.name, name: cleanName(x.name), sub: protocolName(x.kind), op: x.delay == null && wasTested ? 0.55 : 1, checked: !isAuto && x.name === selected?.name, radio: true, pick: () => askPick(x.name), ...rowState(i + 1, x.delay) });
    });
  } else {
    const x = list[0];
    rows = [{ key: 'one', name: cleanName(x?.name ?? act.name), sub: x ? protocolName(x.kind) : 'Одиночная конфигурация', op: 1, checked: false, radio: false, flag: flagOf(x?.name ?? act.name), pick: runPings, ...rowState(0, x?.delay) }];
  }
  if (multi && !list.length && !isActive) rows = rows.slice(0, 1);

  const cardSub = multi ? (sv ? (isAuto ? 'Авто · ' + shortName(sv.name) : cleanName(sv.name).replace(' · ', ', ')) : list.length ? 'Нет доступных серверов' : isActive ? 'Загружаем серверы…' : 'Серверы появятся после переключения') : shortName(list[0]?.name ?? act.name);
  const flag = flagOf(sv?.name ?? (multi ? null : act.name));

  const menu: { label: string; icon: { fr: string; d: string; d2?: string }; color: string; sep: boolean; run: () => void }[] = [];
  if (isSub) {
    menu.push({
      label: 'Скопировать ссылку',
      icon: MI.link,
      color: 'var(--text)',
      sep: false,
      run: () =>
        void transport
          .call<string>('connection_link', { id: act.id })
          .then((l) => navigator.clipboard.writeText(l))
          .then(() => toast('Ссылка скопирована', undefined, 'ok'))
          .catch(() => toast('Не удалось скопировать ссылку', undefined, 'bad')),
    });
    if (act.web_page) menu.push({ label: 'Страница подписки', icon: MI.doc, color: 'var(--text)', sep: false, run: () => void transport.openUrl(act.web_page!) });
    if (act.support_url) menu.push({ label: 'Поддержка', icon: MI.chat, color: 'var(--text)', sep: false, run: () => void transport.openUrl(act.support_url!) });
  }
  menu.push({
    label: 'Переименовать',
    icon: MI.rename,
    color: 'var(--text)',
    sep: false,
    run: () =>
      win.dialog({
        title: isSub ? 'Переименовать подписку' : 'Переименовать',
        text: 'Название видно только вам — на сервере ничего не меняется.',
        okLabel: 'Сохранить',
        okBg: 'var(--accent)',
        input: { value: act.name, label: 'Название подписки' },
        onOk: (v) => void store.rename(act.id, v),
      }),
  });
  menu.push({
    label: isSub ? 'Удалить подписку' : 'Удалить',
    icon: MI.trash,
    color: 'var(--bad)',
    sep: true,
    run: () =>
      win.dialog({
        title: 'Удалить «' + act.name + '»?',
        text: isSub ? 'Серверы и настройки этой подписки пропадут. Ссылку можно будет добавить снова.' : 'Сервер и его настройки пропадут. Ссылку или файл можно будет добавить снова.',
        okLabel: 'Удалить',
        okBg: 'var(--bad)',
        onOk: () => {
          win.setViewId(null);
          void store.remove(act.id);
        },
      }),
  });

  return (
    <div style={css('position: relative; z-index: 0; display: flex; flex-direction: column; gap: 32px; animation: tabin .28s cubic-bezier(.2,.7,.2,1)')}>
      <Field mode={fieldMode} dark={dark} />

      <TopBar on={on} down={down} up={up} />

      <div style={css('display: flex; flex-direction: column; align-items: center; gap: 16px')}>
        <div style={css('display: flex; flex-direction: column; align-items: center; gap: 4px')}>
          <span style={{ ...css('font-size: 18px; line-height: 21px; transition: color .3s ease'), color: labelColor }}>{label}</span>
          <span style={{ ...css('font-size: 20px; line-height: 24px; font-variant-numeric: tabular-nums; letter-spacing: .2px'), color: here && on ? 'var(--text)' : 'var(--text3)' }}>{timer}</span>
        </div>
        <button
          onClick={() => void togglePower()}
          aria-label={powerLabel}
          className="tp"
          style={{
            ...css('position: relative; width: 76px; height: 76px; border: 0; border-radius: 50%; padding: 0; transition: background-color .6s ease, color .5s ease, box-shadow .6s ease, transform .14s cubic-bezier(.2,.7,.2,1); display: flex; align-items: center; justify-content: center; cursor: pointer'),
            background: pwBg,
            boxShadow: pwShadow,
            animation: pwAnim,
            color: pwFg,
          }}
        >
          <svg width="40" height="40" viewBox="0 0 40 40" fill="currentColor" aria-hidden="true">
            <path d={POWER} />
          </svg>
          {spinning ? <span aria-hidden="true" style={css('position: absolute; inset: -5px; border-radius: 50%; border: 2px solid transparent; border-top-color: var(--accent); border-right-color: var(--accent); animation: spin .9s linear infinite')} /> : null}
        </button>
        <button onClick={() => win.go('routes')} className="sf tp" style={css('display: flex; align-items: center; gap: 8px; height: 36px; padding: 0 18px; border: 0; border-radius: 999px; color: var(--text); font-size: 15px; font-weight: 600; cursor: pointer')}>
          <span aria-hidden="true" style={{ ...css('width: 8px; height: 8px; border-radius: 50%'), background: on ? 'var(--ok)' : 'var(--text3)' }} />
          {routingName[state.routing]}
        </button>
      </div>

      {bn ? (
        <div className="sf sq20" role="status" style={css('display: flex; align-items: flex-start; gap: 12px; padding: 14px 16px; animation: fadeUp .4s cubic-bezier(.2,.8,.2,1) both')}>
          <span aria-hidden="true" style={{ ...css('flex: none; width: 36px; height: 36px; border-radius: 50%; display: flex; align-items: center; justify-content: center'), background: `color-mix(in srgb, var(--${bn.tone ?? 'bad'}) 14%, transparent)`, color: `var(--${bn.tone ?? 'bad'})` }}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
              <path d={bn.ic} />
            </svg>
          </span>
          <div style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px')}>
            <span style={css('font-size: 14px; line-height: 18px; font-weight: 600')}>{bn.t}</span>
            <span style={css('font-size: 12.5px; line-height: 17px; color: var(--text2)')}>{bn.x}</span>
            <div style={css('display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px')}>
              <button onClick={bn.go} className="tp" style={css('height: 32px; padding: 0 14px; border: 0; border-radius: 999px; background: var(--accent); color: var(--onAccent); font-size: 13px; font-weight: 600; cursor: pointer; font-family: inherit')}>
                {bn.a}
              </button>
              <button onClick={bn.go2} className="tp" style={css('height: 32px; padding: 0 14px; border: 0; border-radius: 999px; background: var(--fill2); color: var(--text); font-size: 13px; font-weight: 500; cursor: pointer; font-family: inherit')}>
                {bn.a2}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      <div style={css('display: flex; flex-direction: column; gap: 16px')}>
        <div role="tablist" aria-label="Подписки" className="ksc" style={css('display: flex; justify-content: safe center; gap: 4px; overflow-x: auto; margin: 0 -16px; padding: 0 16px')}>
          {conns.map((p) => {
            const a = p.id === act.id;
            return (
              <button
                key={p.id}
                role="tab"
                aria-selected={a ? 'true' : 'false'}
                onClick={() => {
                  setMenuOpen(false);
                  win.setViewId(p.id);
                  if (!liveId && p.id !== activeId) void store.selectConnection(p.id);
                }}
                className="tp"
                style={{ ...css('flex: none; display: flex; align-items: center; justify-content: center; gap: 6px; height: 34px; padding: 0 16px; border: 0; border-radius: 999px; background: var(--surface); box-shadow: inset 0 0 0 .4px var(--line); font-size: 13.5px; cursor: pointer; white-space: nowrap'), color: a ? 'var(--text)' : 'var(--text2)' }}
              >
                {on && liveId === p.id ? <span aria-hidden="true" style={css('width: 6px; height: 6px; border-radius: 50%; background: var(--ok)')} /> : null}
                <span style={css('display: inline-grid')}>
                  <span style={css('grid-area: 1 / 1; font-weight: 400')}>{p.name}</span>
                  <span aria-hidden="true" style={css('grid-area: 1 / 1; font-weight: 400; visibility: hidden')}>
                    {p.name}
                  </span>
                </span>
              </button>
            );
          })}
        </div>

        <div style={css('position: relative')}>
          <section className="sf sq28" style={css('overflow: hidden')}>
            <div style={{ ...css('display: flex; flex-direction: column; gap: 16px; padding: 20px'), boxShadow: exp || !multi ? 'inset 0 -.4px 0 var(--line)' : 'none' }}>
              <div style={css('display: flex; align-items: center; gap: 8px')}>
                <button
                  onClick={() => {
                    if (!multi) return;
                    setExpanded({ ...expanded, [act.id]: !exp });
                    setMenuOpen(false);
                  }}
                  aria-expanded={exp ? 'true' : 'false'}
                  aria-label={exp ? 'Свернуть серверы' : 'Показать серверы'}
                  style={css('flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; border: 0; padding: 0; background: transparent; color: var(--text); text-align: left; cursor: pointer')}
                >
                  <span aria-hidden="true" style={{ ...css('flex: none; width: 44px; height: 44px; border-radius: 12px; box-shadow: inset 0 0 0 1px rgba(0,0,0,.06)'), background: flag }} />
                  <span style={css('min-width: 0; display: flex; flex-direction: column; gap: 4px')}>
                    <span style={css('font-size: 15px; line-height: 18px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{act.name}</span>
                    <span style={css('font-size: 13px; line-height: 16px; color: var(--text2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{cardSub}</span>
                  </span>
                </button>
                <div style={css('display: flex; align-items: center; gap: 6px')}>
                  <button onClick={runPings} className="act pl" aria-label="Проверить задержку" title="Проверить задержку">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <polyline className="ecg" pathLength={100} strokeDasharray="100" points="2.5,12.75 6,12.75 8.6,4.5 12.75,19.5 16.1,8.25 18,12.75 21.5,12.75" style={{ opacity: otherLive ? 0.35 : pinging ? 0.25 : 1, transition: 'opacity .2s ease' }} />
                      <polyline pathLength={100} strokeDasharray="24 100" points="2.5,12.75 6,12.75 8.6,4.5 12.75,19.5 16.1,8.25 18,12.75 21.5,12.75" style={{ stroke: 'var(--accent)', opacity: pinging ? 1 : 0, animation: pinging ? 'run .9s linear infinite' : 'none' }} />
                    </svg>
                  </button>
                  {isSub ? (
                    <button onClick={() => void refresh()} className="act rf" aria-label="Обновить подписку" title="Обновить подписку">
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ animation: refreshing ? 'spin1 .9s cubic-bezier(.6,-0.28,.3,1.3) infinite' : 'none' }}>
                        <path d="M17.79 15.05a7.5 7.5 0 1 1-1.15-8.09c.13.13.59.65.98 1.43" />
                        <path d="m14.06 8.45 4.03.73.71-4.11" />
                      </svg>
                    </button>
                  ) : null}
                  <button onClick={() => setMenuOpen(!menuOpen)} className="dotsb" aria-label="Ещё" aria-haspopup="menu" aria-expanded={menuOpen ? 'true' : 'false'}>
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                      <path d="M12 7c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm0 7c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm0 7c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z" />
                    </svg>
                  </button>
                </div>
              </div>
              {showInfo ? (
                <>
                  {info?.expire ? (
                    <div style={css('display: flex; justify-content: space-between')}>
                      <span className="lab">{expLabel}</span>
                      <span className="val" style={{ color: status === 'expired' ? 'var(--bad)' : 'var(--text)' }}>
                        {expLeft}
                      </span>
                    </div>
                  ) : null}
                  {hasTraffic ? (
                    <div style={css('display: flex; flex-direction: column; gap: 10px')}>
                      <div style={css('display: flex; align-items: baseline; justify-content: space-between')}>
                        <span className="lab">
                          Осталось <span style={{ color: status === 'traffic' ? 'var(--bad)' : 'var(--text)', fontVariantNumeric: 'tabular-nums' }}>{gbText(Math.max(0, info!.total - used))} ГБ</span>
                        </span>
                        <span className="lab" style={css('font-variant-numeric: tabular-nums')}>
                          из {gbTotal(info!.total)} ГБ
                        </span>
                      </div>
                      <div role="progressbar" aria-label="Остаток трафика" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(100 - pct)} style={css('position: relative; height: 10px; border-radius: 999px; background: var(--fill); box-shadow: inset 0 0 0 .4px var(--line); overflow: hidden')}>
                        <div style={{ ...css('position: absolute; left: 0; top: 0; bottom: 0; border-radius: 999px; transition: width .5s cubic-bezier(.2,.7,.2,1), background-color .3s ease'), width: (100 - pct).toFixed(0) + '%', background: trafColor }} />
                        {[25, 50, 75].map((l) => (
                          <span key={l} aria-hidden="true" style={{ ...css('position: absolute; top: 3px; bottom: 3px; width: 1px; background: color-mix(in srgb, var(--text) 10%, transparent)'), left: l + '%' }} />
                        ))}
                      </div>
                    </div>
                  ) : null}
                </>
              ) : null}
            </div>

            {exp || !multi ? (
              <div style={css('display: flex; flex-direction: column; gap: 4px; padding: 8px; animation: expandin .25s ease')}>
                {rows.map((s) => (
                  <button key={s.key} onClick={s.pick} className={'srow' + (s.checked ? ' on' : '')} role={s.radio ? 'radio' : 'button'} aria-checked={s.radio ? (s.checked ? 'true' : 'false') : undefined} style={{ position: 'relative', overflow: 'hidden', opacity: s.op }}>
                    {s.scanning ? <span aria-hidden="true" style={css('position: absolute; inset: 0; background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent) 14%, transparent), transparent); animation: rowScan .9s cubic-bezier(.4,0,.2,1) infinite')} /> : null}
                    {s.radio ? (
                      <span aria-hidden="true" className="rdo" style={{ boxShadow: s.checked ? 'inset 0 0 0 2px var(--accent)' : 'inset 0 0 0 1.6px var(--text3)' }}>
                        <i style={{ transform: `scale(${s.checked ? 1 : 0})` }} />
                      </span>
                    ) : (
                      <span aria-hidden="true" style={{ ...css('flex: none; width: 20px; height: 20px; border-radius: 6px; box-shadow: inset 0 0 0 .4px rgba(0,0,0,.15)'), background: s.flag }} />
                    )}
                    <span style={css('flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px')}>
                      <span style={css('font-size: 14px; line-height: 16px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis')}>{s.name}</span>
                      <span style={css('font-size: 12.5px; line-height: 16px; color: var(--text2)')}>{s.sub}</span>
                    </span>
                    {s.loading ? <span className="skel" role="img" aria-label="Замеряем задержку" /> : null}
                    {s.fresh && !s.loading ? (
                      <span style={{ ...css('font-size: 13px; font-weight: 600; font-variant-numeric: tabular-nums; animation: valPop .45s cubic-bezier(.2,.8,.2,1) both'), color: s.pingColor }}>{s.ping}</span>
                    ) : null}
                    {!s.loading && !s.fresh ? <span style={{ ...css('font-size: 13px; font-weight: 500; font-variant-numeric: tabular-nums'), color: s.pingColor }}>{s.ping}</span> : null}
                  </button>
                ))}
              </div>
            ) : null}
          </section>

          {menuOpen ? (
            <>
              <button aria-label="Закрыть меню" onClick={() => setMenuOpen(false)} style={css('position: fixed; inset: 0; z-index: 30; border: 0; padding: 0; background: transparent; cursor: default')} />
              <div role="menu" style={css('position: absolute; top: 62px; right: 16px; z-index: 31; width: 228px; padding: 6px; border-radius: 18px; background: var(--surface); box-shadow: var(--pop); transform-origin: top right; animation: popin .16s cubic-bezier(.2,.8,.2,1)')}>
                {menu.map((m) => (
                  <div key={m.label}>
                    {m.sep ? <div style={css('height: 1px; margin: 4px 8px; background: var(--line)')} /> : null}
                    <button
                      role="menuitem"
                      onClick={() => {
                        setMenuOpen(false);
                        m.run();
                      }}
                      className="mi"
                      style={{ color: m.color }}
                    >
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" style={{ flex: 'none', opacity: m.color === 'var(--bad)' ? 1 : 0.85 }}>
                        <path fillRule={m.icon.fr as 'evenodd' | 'nonzero'} d={m.icon.d} />
                        <path fillRule="evenodd" d={m.icon.d2 ?? 'M0 0'} />
                      </svg>
                      {m.label}
                    </button>
                  </div>
                ))}
              </div>
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}

const DL = 'M19.38 10c0 1.49-.48 2.94-1.38 4.13a.63.63 0 1 1-1-.76 5.63 5.63 0 1 0-10.12-3.37.63.63 0 1 1-1.25 0c0-.63.08-1.26.25-1.87h-.25a3.75 3.75 0 0 0 0 7.5H7.5a.63.63 0 1 1 0 1.25H5.63a5 5 0 1 1 .72-9.95A6.88 6.88 0 0 1 19.38 10zm-5.45 3.31-1.43 1.43V10a.63.63 0 1 0-1.25 0v4.74l-1.43-1.43a.63.63 0 0 0-.89.88l2.5 2.5a.63.63 0 0 0 .89 0l2.5-2.5a.63.63 0 0 0-.89-.88z';
const UL = 'M13.93 12.94 12.5 11.51v4.74a.63.63 0 1 1-1.25 0v-4.74l-1.43 1.43a.63.63 0 0 1-.89-.88l2.5-2.5a.63.63 0 0 1 .89 0l2.5 2.5a.63.63 0 0 1-.89.88zM12.5 3.13a6.88 6.88 0 0 0-6.15 3.8 5 5 0 1 0-.72 9.95h3.12a.63.63 0 1 0 0-1.25H5.63a3.75 3.75 0 0 1 0-7.5h.25A6.88 6.88 0 0 0 5.63 10a.63.63 0 1 0 1.25 0 5.63 5.63 0 1 1 7.87 5.16.63.63 0 0 0 .25 1.19.6.6 0 0 0 .25-.05A6.88 6.88 0 0 0 12.5 3.13z';

/** Скорость (открывает лист «Скорость») и «+». */
function TopBar({ on, down, up }: { on: boolean; down: string; up: string }) {
  const win = useWin();
  const c = on ? 'var(--text)' : 'var(--text3)';
  return (
    <div style={css('display: flex; align-items: center; gap: 10px')}>
      <button onClick={() => win.openSheet('speed')} className="sf spd" aria-label="Скорость — подробнее" title="Скорость — подробнее" style={css('display: flex; align-items: center; gap: 8px; height: 40px; padding: 0 16px 0 14px; border: 0; border-radius: 24px; color: var(--text); font-family: inherit')}>
        <svg width="20" height="20" viewBox="0 0 20 20" fill={c} aria-label="Загрузка">
          <path d={DL} />
        </svg>
        <span style={{ ...css('font-size: 13px; font-weight: 500; font-variant-numeric: tabular-nums'), color: c }}>{down}</span>
        <span aria-hidden="true" style={css('flex: none; width: 1px; height: 16px; background: var(--line)')} />
        <svg width="20" height="20" viewBox="0 0 20 20" fill={c} aria-label="Отдача">
          <path d={UL} />
        </svg>
        <span style={{ ...css('font-size: 13px; font-weight: 500; font-variant-numeric: tabular-nums'), color: c }}>{up}</span>
      </button>
      <span style={css('flex: 1')} />
      <button onClick={() => win.openAdd()} aria-label="Добавить подписку" className="sf tp" style={css('width: 40px; height: 40px; border: 0; border-radius: 24px; color: var(--text); display: flex; align-items: center; justify-content: center; cursor: pointer')}>
        <svg width="22" height="22" viewBox="0 0 22 22" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
          <path d="M3.67 11h14.66M11 18.33V3.67" />
        </svg>
      </button>
    </div>
  );
}

function EmptyHome() {
  const win = useWin();
  const { transport } = useStore();
  return (
    <div style={css('position: relative; z-index: 0; display: flex; flex-direction: column; gap: 32px; animation: tabin .28s cubic-bezier(.2,.7,.2,1)')}>
      <TopBar on={false} down="0 Мб/с" up="0 Мб/с" />
      <div style={css('display: flex; flex-direction: column; align-items: center; text-align: center; padding: 36px 12px 0; animation: fadeUp .45s cubic-bezier(.2,.8,.2,1) both')}>
        <div aria-hidden="true" style={css('width: 76px; height: 76px; border-radius: 50%; background: var(--surface); box-shadow: inset 0 0 0 .4px var(--line); color: var(--text3); display: flex; align-items: center; justify-content: center')}>
          <svg width="40" height="40" viewBox="0 0 40 40" fill="currentColor">
            <path d={POWER} />
          </svg>
        </div>
        <div style={css('margin-top: 20px; font-size: 20px; line-height: 26px; font-weight: 600')}>Пока нет подключений</div>
        <div style={css('margin-top: 6px; font-size: 13.5px; line-height: 19px; color: var(--text2)')}>Скопируйте ссылку на подписку у своего VPN-провайдера и добавьте её — серверы подтянутся сами.</div>
        <button onClick={() => win.openAdd()} className="tp" style={css('margin-top: 22px; height: 48px; padding: 0 26px; border: 0; border-radius: 999px; background: var(--accent); color: var(--onAccent); font-size: 15px; font-weight: 600; cursor: pointer; font-family: inherit')}>
          Добавить подписку
        </button>
        <button
          onClick={() =>
            void transport
              .clipboardText()
              .catch(() => '')
              .then((t) => win.openAdd({ link: t.trim(), name: '', from: null }))
          }
          style={css('margin-top: 12px; border: 0; padding: 4px 8px; background: transparent; color: var(--text2); font-size: 13px; cursor: pointer; font-family: inherit')}
        >
          Вставить ссылку из буфера
        </button>
      </div>
    </div>
  );
}
