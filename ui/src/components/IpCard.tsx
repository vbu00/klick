// «Как вас видят сайты»: две колонки, через VPN и напрямую, и проверка утечек. Проверка — по кнопке.

import { useState } from 'react';
import { errorText } from '../lib/i18n';
import { useStore } from '../lib/store';
import type { ErrorInfo, IpColumn, IpReport } from '../lib/types';
import { SectionHead } from './Controls';
import { Icon } from './Icon';

/** Последний отчёт живёт, пока открыто окно: при возврате на вкладку не нужно проверять заново. */
let lastReport: IpReport | null = null;

const regionNames = (() => {
  try {
    return new Intl.DisplayNames(['ru'], { type: 'region' });
  } catch {
    return null;
  }
})();

function country(c: IpColumn): string | null {
  if (c.country_code) return regionNames?.of(c.country_code.toUpperCase()) ?? c.country ?? c.country_code;
  return c.country;
}

type Cell = { text: string; cls?: string; title?: string };

function cells(c: IpColumn | null, vpnColumn: boolean): Record<string, Cell> {
  const dash = { text: '—', cls: 'dim' };
  if (!c) return { ip: { text: 'VPN выключен', cls: 'dim' }, ipv6: dash, country: dash, city: dash, provider: dash, rdns: dash, vpn: dash };
  if (c.error && !c.ipv4 && !c.ipv6) return { ip: { text: 'не удалось', cls: 'bad' }, ipv6: dash, country: dash, city: dash, provider: dash, rdns: dash, vpn: dash };
  const detected: Cell =
    c.vpn_detected == null ? dash : c.vpn_detected ? { text: 'да', cls: vpnColumn ? 'warn' : 'bad' } : { text: 'нет', cls: vpnColumn ? 'good' : undefined };
  return {
    ip: { text: c.ipv4 ?? '—', cls: 'mono', title: c.ipv4 ?? undefined },
    ipv6: c.ipv6 ? { text: c.ipv6, cls: 'mono', title: c.ipv6 } : { text: 'нет', cls: 'dim' },
    country: { text: country(c) ?? '—' },
    city: { text: c.city ?? '—' },
    provider: { text: c.provider ?? '—', title: c.provider ? `${c.provider}${c.asn ? ` · AS${c.asn}` : ''}` : undefined },
    rdns: c.reverse_dns ? { text: c.reverse_dns, cls: 'mono', title: c.reverse_dns } : dash,
    vpn: detected,
  };
}

const ROWS: [keyof ReturnType<typeof cells>, string][] = [
  ['ip', 'IP'],
  ['ipv6', 'IPv6'],
  ['country', 'Страна'],
  ['city', 'Город, примерно'],
  ['provider', 'Провайдер'],
  ['rdns', 'Имя адреса'],
  ['vpn', 'Похоже на VPN'],
];

export function IpCard() {
  const { transport, toast } = useStore();
  const [report, setReport] = useState<IpReport | null>(lastReport);
  const [busy, setBusy] = useState(false);

  const check = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const r = await transport.call<IpReport>('ip_check');
      lastReport = r;
      setReport(r);
    } catch (e) {
      toast(errorText((e as ErrorInfo)?.code ?? 'unknown'), undefined, 'bad');
    }
    setBusy(false);
  };

  const vpn = report ? cells(report.via_vpn, true) : null;
  const direct = report ? cells(report.direct, false) : null;
  const leak = report?.ipv6_leak;
  const dns = report?.dns_protected;

  return (
    <>
      <SectionHead title="Как вас видят сайты">
        {report ? (
          <button className="sec-act" onClick={check} disabled={busy}>
            <Icon name="refresh" size={14} className={busy ? 'spin' : undefined} />
            {busy ? 'Проверяю…' : 'Проверить'}
          </button>
        ) : null}
      </SectionHead>
      <div className="ip-card">
        {report && vpn && direct ? (
          <>
            <div className="ip-grid">
              <span />
              <span className="h">через VPN</span>
              <span className="h">напрямую</span>
              {ROWS.map(([key, label]) => (
                <Line key={key} label={label} a={vpn[key]} b={direct[key]} />
              ))}
              <span className="ip-sep" />
              <span className="k">Утечка IPv6</span>
              <span className={leak == null ? 'v dim' : leak ? 'v bad' : 'v good'}>{leak == null ? '—' : leak ? 'есть' : 'нет'}</span>
              <span />
              <span className="k">DNS через kl!ck</span>
              <span className={dns == null ? 'v dim' : dns ? 'v good' : 'v bad'}>{dns == null ? '—' : dns ? 'да' : 'нет'}</span>
              <span />
            </div>
            <div className="foot-note">
              Город примерный: базы геолокации расходятся. «Похоже на VPN» — по базе одного сервиса, у других может отличаться. Утечки проверяются в режиме VPN (TUN); WebRTC — только в браузере.
              {' '}Проверено в {new Date(report.checked_at * 1000).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}.
            </div>
          </>
        ) : (
          <>
            <div className="ip-empty">Какой адрес, страну и провайдера видят сайты — через VPN и напрямую. Заодно проверим, не уходят ли IPv6 и DNS мимо туннеля.</div>
            <button className="btn-card in-card" onClick={check} disabled={busy}>
              <Icon name="refresh" size={18} className={busy ? 'spin' : undefined} />
              {busy ? 'Проверяю…' : 'Проверить'}
            </button>
          </>
        )}
      </div>
    </>
  );
}

function Line({ label, a, b }: { label: string; a: Cell; b: Cell }) {
  return (
    <>
      <span className="k">{label}</span>
      <span className={`v ${a.cls ?? ''}`} title={a.title}>
        {a.text}
      </span>
      <span className={`v ${b.cls ?? ''}`} title={b.title}>
        {b.text}
      </span>
    </>
  );
}
