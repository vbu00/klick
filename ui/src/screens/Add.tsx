// «Добавить подключение» — как в макете: по ссылке или из файла.

import { useEffect, useRef, useState } from 'react';
import { Icon } from '../components/Icon';
import type { Tab } from '../components/Chrome';
import { protocolName } from '../lib/format';
import { useStore } from '../lib/store';

const LINK_SCHEMES = ['vless', 'vmess', 'trojan', 'ss', 'ssr', 'socks', 'socks5', 'hysteria', 'hysteria2', 'hy2', 'tuic', 'wireguard', 'wg', 'anytls'];

type Detect = { kind: 'empty' | 'sub' | 'link' | 'bad'; title: string; text: string; color: string };

function detect(input: string): Detect {
  const s = input.trim();
  if (!s) return { kind: 'empty', title: 'Ожидаем ссылку', text: 'Скопируйте её у провайдера VPN и вставьте выше — тип определится сам.', color: 'var(--dim)' };
  const scheme = s.includes('://') ? s.split('://')[0].toLowerCase() : '';
  if ((scheme === 'https' || scheme === 'http') && !/\s/.test(s)) {
    return { kind: 'sub', title: 'Ссылка на подписку', text: 'Загрузим список серверов, лимит трафика и срок действия. Будет обновляться автоматически.', color: 'var(--accent)' };
  }
  if (scheme === 'ssconf' && !/\s/.test(s)) {
    return { kind: 'sub', title: 'Ключ доступа Outline', text: 'Загрузим сервер Shadowsocks по ключу и будем обновлять его автоматически.', color: 'var(--accent)' };
  }
  if (LINK_SCHEMES.includes(scheme) && !/\s/.test(s)) {
    return { kind: 'link', title: `Одиночная конфигурация · ${protocolName(scheme === 'hy2' ? 'hysteria2' : scheme)}`, text: 'Прямое соединение с одним сервером. Без лимитов и срока — только адрес и ключ.', color: 'var(--accent)' };
  }
  return { kind: 'bad', title: 'Формат не распознан', text: 'Ожидается https://… или vless://, vmess://, trojan://, ss://, hysteria2://', color: 'var(--red)' };
}

export function Add({ onTab }: { onTab: (t: Tab) => void }) {
  const store = useStore();
  const { incoming, consumeIncoming } = store;
  const [tab, setTab] = useState<'link' | 'file'>('link');
  const [link, setLink] = useState('');
  const [name, setName] = useState('');
  /** Подписка пришла ссылкой klick://add: вместо всей ссылки — её домен. */
  const [fromPage, setFromPage] = useState<{ host: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [over, setOver] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const addRef = useRef<HTMLButtonElement>(null);
  const d = detect(link);
  const canAdd = (d.kind === 'sub' || d.kind === 'link') && !busy;

  // Ссылка klick://add: подписка уже вставлена, добавляет её человек своей кнопкой.
  useEffect(() => {
    if (!incoming) return;
    setTab('link');
    setLink(incoming.url);
    setName(incoming.name);
    setFromPage({ host: incoming.host });
    consumeIncoming();
  }, [incoming, consumeIncoming]);

  useEffect(() => {
    if (fromPage) addRef.current?.focus();
  }, [fromPage]);

  const reset = () => {
    setLink('');
    setName('');
    setFromPage(null);
  };

  const addLink = async () => {
    if (!canAdd) return;
    setBusy(true);
    const ok = await store.addLink(link.trim(), d.kind === 'sub' ? name.trim() : undefined);
    setBusy(false);
    if (ok) {
      reset();
      onTab('home');
    }
  };

  if (fromPage && tab === 'link') {
    return (
      <div className="screen">
        <div className="screen-title">Добавить подписку</div>
        <div className="from-page">
          <div className="caps">Ссылка со страницы в браузере</div>
          <div className="from-page-host">{fromPage.host || 'ссылка подписки'}</div>
          <span>Загрузим список серверов, лимит трафика и срок действия. Добавляйте, только если эту страницу открыли вы сами — у своего VPN-сервиса.</span>
        </div>
        <input className="name-input" value={name} maxLength={64} onChange={(e) => setName(e.target.value)} placeholder="Название (необязательно)" />
        <button ref={addRef} className="btn-primary" style={{ marginTop: 14 }} disabled={!canAdd} onClick={addLink} aria-label="Добавить подписку">
          {busy ? 'Добавляем…' : 'Добавить'}
        </button>
        <button
          className="btn-card"
          disabled={busy}
          onClick={() => {
            reset();
            onTab('home');
          }}
        >
          Отмена
        </button>
      </div>
    );
  }

  const addFile = async (file: File | undefined) => {
    if (!file || busy) return;
    setBusy(true);
    const ok = await store.importFile(file);
    setBusy(false);
    if (ok) onTab('home');
  };

  return (
    <div className="screen">
      <div className="screen-title">Добавить подключение</div>
      <div className="seg" style={{ marginTop: 18 }}>
        <button className={tab === 'link' ? 'on' : ''} onClick={() => setTab('link')}>
          По ссылке
        </button>
        <button className={tab === 'file' ? 'on' : ''} onClick={() => setTab('file')}>
          Из файла
        </button>
      </div>

      {tab === 'link' ? (
        <>
          <textarea
            className={`link-input ${d.kind === 'bad' ? 'bad' : d.kind === 'empty' ? '' : 'ok'}`}
            value={link}
            onChange={(e) => setLink(e.target.value)}
            placeholder="https://panel.example.com/sub/a1b2… или vless://…"
            spellCheck={false}
            aria-label="Ссылка на подписку или конфигурацию"
          />
          <div className="detect">
            <div className="dot8" style={{ background: d.color }} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <b>{d.title}</b>
              <span>{d.text}</span>
            </div>
          </div>
          {d.kind === 'sub' ? <input className="name-input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Название (необязательно)" /> : null}
          <button className="btn-primary" style={{ marginTop: 14 }} disabled={!canAdd} onClick={addLink}>
            {busy ? 'Добавляем…' : 'Добавить'}
          </button>

          <div className="caps" style={{ marginTop: 26 }}>
            Что можно вставить
          </div>
          <div className="help">
            <div className="help-row">
              <div className="help-ico">
                <Icon name="globe" size={18} />
              </div>
              <div>
                <b>Ссылка на подписку</b>
                <span>https://… от Remnawave, Marzban и подобных — в форматах Clash, sing-box, Xray или списком ссылок; ключ Outline ssconf://. Даёт список серверов, лимит трафика и срок действия. Обновляется автоматически.</span>
              </div>
            </div>
            <div className="help-row">
              <div className="help-ico">
                <Icon name="rules" size={18} />
              </div>
              <div>
                <b>Одиночная конфигурация</b>
                <span>vless://, vmess://, trojan://, ss://, hysteria2:// — прямое соединение с одним сервером.</span>
              </div>
            </div>
          </div>
        </>
      ) : (
        <>
          <div
            className={over ? 'drop over' : 'drop'}
            role="button"
            tabIndex={0}
            onClick={() => fileRef.current?.click()}
            onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && fileRef.current?.click()}
            onDragOver={(e) => {
              e.preventDefault();
              setOver(true);
            }}
            onDragLeave={() => setOver(false)}
            onDrop={(e) => {
              e.preventDefault();
              setOver(false);
              void addFile(e.dataTransfer.files[0]);
            }}
          >
            <Icon name="upload" size={34} />
            <b>{busy ? 'Импортируем…' : 'Перетащите файл сюда'}</b>
            <span>или нажмите, чтобы выбрать</span>
          </div>
          <input ref={fileRef} type="file" accept=".yaml,.yml,.json,.conf,.txt" hidden onChange={(e) => void addFile(e.target.files?.[0])} />
          <div className="note-card">
            Поддерживаются конфиги Mihomo / Clash (<b>.yaml</b>, <b>.json</b>), sing-box и Xray (<b>.json</b>), WireGuard и AmneziaWG (<b>.conf</b>), Shadowsocks и Outline (<b>.json</b>), списки ссылок (<b>.txt</b>). Из файла берутся серверы; файл копируется в приложение — оригинал можно удалить.
          </div>
        </>
      )}
    </div>
  );
}
