// «Проверка серверов»: из этой сети — какие серверы подписки работают, какие провайдер не пускает,
// а какие «замораживает» ТСПУ (сервер отвечает, но загрузка через него встаёт). Запускается только по кнопке
// и идёт до полутора минут; итоги хранит служба (probe_last), отметки у серверов — в списках на главной и в трее.

import { countryName, parseServer } from '../lib/countries';
import { protocolName } from '../lib/format';
import { errorText } from '../lib/i18n';
import { betterServer, probeCounts, probeMark } from '../lib/probe';
import { plural } from '../lib/rules';
import { useStore } from '../lib/store';
import type { ProbeReport, ServerView } from '../lib/types';
import { Elapsed } from '../screens/Home';
import { ProbeMark, SectionHead, Toggle } from './Controls';
import { Flag } from './Flag';
import { Icon } from './Icon';

const clock = (since: number | null, now: number) => {
  const s = Math.max(0, Math.floor(now / 1000 - (since ?? now / 1000)));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
};

/** «Финляндия» и «Hysteria2» вместо «🇫🇮 FI · HY2»: эмодзи-флаги Windows не рисует. */
function serverLabel(s: ServerView): { code: string | null; head: string; sub: string; text: string } {
  const p = parseServer(s.name, s.kind);
  const proto = protocolName(s.kind);
  const head = p.code ? countryName(p.code) : s.name;
  const sub = p.code ? [p.title || proto, ...(p.title && !p.named ? [proto] : [])].join(' · ') : proto;
  return { code: p.code, head, sub, text: `${head} · ${sub}` };
}

/** Тип сети по базе proxycheck (служба: ipcheck::fill); незнакомое не показываем. */
const NET_TYPE: Record<string, string> = { mobile: 'мобильный интернет', home: 'домашний интернет', business: 'офисная сеть', hosting: 'хостинг', vpn: 'VPN или прокси' };

function networkText(n: ProbeReport['network']): string | null {
  const place = n.region ?? (n.country ? countryName(n.country.toUpperCase()) : null);
  const parts = [n.operator, n.net_type ? NET_TYPE[n.net_type] : null, place].filter(Boolean);
  return parts.length ? parts.join(' · ') : null;
}

const SHARED: Record<NonNullable<ProbeReport['shared']>, { text: string; cls: string; title?: string }> = {
  sent: { text: 'итоги отправлены панели подписки', cls: 'good' },
  off: { text: 'сейчас не принимает итоги', cls: 'dim' },
  later: { text: 'отправка отложена', cls: 'dim', title: 'Недавно уже отправляли, или панель просит прислать позже' },
  failed: { text: 'отправить не удалось', cls: 'warn' },
};

/** Заголовок совета по вердикту текущего сервера. */
const TROUBLE: Record<string, string> = {
  freeze: 'Провайдер «замораживает» ваш сервер',
  tcp_blocked: 'Провайдер не пускает к вашему серверу',
  handshake_failed: 'Протокол вашего сервера не проходит',
  no_answer: 'Ваш сервер не отвечает',
};

function when(unix: number): string {
  const d = new Date(unix * 1000);
  const time = d.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
  return d.toDateString() === new Date().toDateString() ? `в ${time}` : `${d.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' })} в ${time}`;
}

export function ProbeCard() {
  const store = useStore();
  const { state, servers, probing, probeError, isRunning } = store;
  const conn = state?.connection ?? null;
  const list = conn ? servers[conn.id] ?? [] : [];
  const last = conn?.probe_last ?? null;
  const busy = probing != null;
  const error = !busy && conn && probeError?.id === conn.id ? probeError.error : null;
  const tunOn = state?.mode === 'tun' && isRunning;
  const run = () => void store.probe();

  return (
    <>
      <SectionHead title="Проверка серверов">
        {last ? (
          <button className="sec-act" onClick={run} disabled={busy}>
            <Icon name="refresh" size={14} className={busy ? 'spin' : undefined} />
            {busy ? 'Проверяю…' : 'Проверить снова'}
          </button>
        ) : null}
      </SectionHead>
      <section className="ip-card pr-card" aria-label="Проверка серверов" aria-busy={busy}>
        {busy && probing != null ? (
          <div className="pr-run">
            <div className="pr-head">
              <span className="pr-run-title" role="status">
                {list.length ? `Проверяю ${plural(list.length, ['сервер', 'сервера', 'серверов'])}…` : 'Проверяю серверы…'}
              </span>
              <span className="pr-when num">
                <Elapsed since={probing} format={clock} />
              </span>
            </div>
            <div className="pr-track" aria-hidden="true">
              <i />
            </div>
            <div className="pr-text">Через каждый сервер скачиваю пробный файл — по четыре сервера сразу. Обычно 15–40 секунд, не дольше полутора минут.</div>
          </div>
        ) : null}

        {error ? (
          <div className="pr-error" role="alert">
            <b>Проверка не удалась</b>
            <span>{errorText(error.code, error.params)}</span>
          </div>
        ) : null}

        {!busy && last ? <Result report={last.report} checkedAt={last.checked_at} list={list} /> : null}

        {!busy && !last ? (
          <>
            {!error ? (
              <div className="ip-empty">
                {conn
                  ? 'Проверим каждый сервер подписки из вашей сети: какие работают, какие провайдер блокирует, а какие «замораживает» — сервер отвечает, но загрузка встаёт. Так глушит VPN ТСПУ — оборудование провайдера для блокировок.'
                  : 'Добавьте подключение — тогда можно будет проверить его серверы из вашей сети.'}
              </div>
            ) : null}
            <button className="btn-card in-card" onClick={run} disabled={!conn}>
              <Icon name={error ? 'refresh' : 'server'} size={18} />
              {error ? 'Попробовать снова' : 'Проверить серверы'}
            </button>
            {conn && !error ? <div className="pr-text center">Обычно 15–40 секунд, не дольше полутора минут</div> : null}
            {conn && tunOn ? (
              <div className="foot-note">Включён VPN (TUN): заморозку проверим, а пускает ли провайдер к серверам напрямую и не подменяет ли DNS — нет. Для полной проверки выключите VPN.</div>
            ) : null}
          </>
        ) : null}

        {conn?.info?.probe_report ? <Share on={!!conn.probe_share} /> : null}
      </section>
    </>
  );
}

function Result({ report, checkedAt, list }: { report: ProbeReport; checkedAt: number; list: ServerView[] }) {
  const store = useStore();
  const c = probeCounts(list);
  // Список серверов ещё не пришёл — хотя бы сколько проверено.
  const checked = c.checked || report.servers;
  const current = list.find((s) => s.selected) ?? list.find((s) => s.name === store.state?.server);
  const verdict = current?.probe?.verdict;
  const better = betterServer(list, current);
  const net = report.network;
  const netText = networkText(net);
  const shared = report.shared ? SHARED[report.shared] : null;
  const verb = c.ok % 10 === 1 && c.ok % 100 !== 11 ? 'работает' : 'работают';
  const cur = current ? serverLabel(current) : null;
  const alt = better ? serverLabel(better) : null;
  const altMs = better ? better.probe?.delay_ms ?? better.delay : null;

  return (
    <>
      <div className="pr-head">
        <span className="pr-sum" role="status">
          {c.checked ? (
            <>
              <b>{c.ok}</b> из {c.checked} {verb}
            </>
          ) : (
            <>Проверено {plural(checked, ['сервер', 'сервера', 'серверов'])}</>
          )}
        </span>
        <span className="pr-when" title={`Проверено ${new Date(checkedAt * 1000).toLocaleString('ru-RU')}`}>
          {when(checkedAt)}
        </span>
      </div>
      {c.checked ? (
        <>
          <div className="pr-bar" aria-hidden="true">
            {c.ok ? <i className="ok" style={{ flexGrow: c.ok }} /> : null}
            {c.freeze ? <i className="warn" style={{ flexGrow: c.freeze }} /> : null}
            {c.down ? <i className="bad" style={{ flexGrow: c.down }} /> : null}
          </div>
          <div className="pr-legend">
            <span>
              <i className="ok" />
              Работают <b>{c.ok}</b>
            </span>
            <span title="Сервер отвечает, но загрузка через него встаёт после нескольких килобайт">
              <i className="warn" />
              Заморозка <b>{c.freeze}</b>
            </span>
            <span title="Провайдер не пускает к серверу, протокол не проходит или сервер молчит">
              <i className="bad" />
              Недоступны <b>{c.down}</b>
            </span>
          </div>
        </>
      ) : null}

      {verdict && verdict !== 'ok' ? (
        <div className={better ? 'pr-tip' : 'pr-tip bad'}>
          <div className="pr-tip-title">{TROUBLE[verdict] ?? 'Ваш сервер не работает в этой сети'}</div>
          {better && alt ? (
            <>
              <div className="pr-tip-sub">Работает в этой сети и быстро отвечает:</div>
              <div className="pr-tip-row">
                {alt.code ? <Flag code={alt.code} /> : null}
                <span className="grow" title={better.name}>
                  <span className="pr-tip-name">{alt.head}</span>
                  <span className="pr-tip-proto">
                    {alt.sub}
                    {altMs != null ? ` · ${altMs} мс` : ''}
                  </span>
                </span>
                <button className="pr-tip-btn" aria-label={`Переключиться на ${alt.text}`} onClick={() => void store.selectServer(better.name)}>
                  Переключить
                </button>
              </div>
            </>
          ) : (
            <div className="pr-tip-sub">Рабочих серверов в этой сети не нашлось. Попробуйте другую сеть или напишите в поддержку подписки.</div>
          )}
        </div>
      ) : null}

      <div className="pr-kv">
        {current && cur ? (
          <>
            <span className="k">Сервер</span>
            <span className="v pr-cur" title={current.name}>
              {cur.code ? <Flag code={cur.code} /> : null}
              <span className="ellipsis">{cur.text}</span>
              {/* Не работает — об этом говорит совет выше; здесь только «работает» или «нестабильно». */}
              {verdict === 'ok' ? probeMark(current.probe) ? <ProbeMark probe={current.probe} /> : <span className="pr-ok">работает</span> : null}
            </span>
          </>
        ) : null}
        {netText ? (
          <>
            <span className="k">Сеть</span>
            <span className="v" title={net.asn ? `AS${net.asn}` : undefined}>
              {netText}
            </span>
          </>
        ) : null}
        {!net.direct_checked ? (
          <>
            <span className="k">Напрямую</span>
            <span className="v dim">не проверяли: был включён VPN (TUN)</span>
          </>
        ) : null}
        {net.direct_checked && c.checked ? (
          <>
            <span className="k">DNS</span>
            <span className={c.dns ? 'v warn' : 'v'} title={c.dns ? 'DNS провайдера отвечает не тем адресом, что настоящий: так он уводит от сервера' : undefined}>
              {c.dns ? `провайдер подменяет адрес у ${plural(c.dns, ['сервера', 'серверов', 'серверов'])}` : 'провайдер не подменяет'}
            </span>
          </>
        ) : null}
        {shared ? (
          <>
            <span className="k">Панель</span>
            <span className={`v ${shared.cls}`} title={shared.title}>
              {shared.text}
            </span>
          </>
        ) : null}
      </div>

      {c.flaky || report.servers < list.length ? (
        <div className="foot-note">
          {c.flaky ? `${plural(c.flaky, ['сервер работает', 'сервера работают', 'серверов работают'])} нестабильно: часть попыток прошла, часть — нет. ` : ''}
          {report.servers < list.length ? `Проверены ${report.servers} из ${list.length}: на остальные не хватило полутора минут.` : ''}
        </div>
      ) : null}
      {!net.direct_checked ? (
        <div className="foot-note">Заморозку видно и через VPN. Чтобы узнать, пускает ли провайдер к серверам напрямую и не подменяет ли DNS, выключите VPN и проверьте снова.</div>
      ) : null}
    </>
  );
}

function Share({ on }: { on: boolean }) {
  const store = useStore();
  const title = 'Отправлять итоги панели подписки';
  return (
    <div className="pr-share">
      <div className="grow">
        <div className="pr-share-title">{title}</div>
        <div className="pr-share-sub">
          Уйдут типы и адреса серверов, как в подписке, итоги проверки, оператор и регион сети — без вашего IP и имени устройства. Это помогает провайдеру VPN чинить серверы под вашего оператора.
        </div>
      </div>
      <Toggle label={title} on={on} onChange={(v) => void store.probeShare(v)} />
    </div>
  );
}
