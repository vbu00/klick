import { useEffect, useState } from 'react';
import type { Incoming } from './screens/Add';
import { BottomNav, Offline, TitleBar, Toasts, type Tab } from './components/Chrome';
import { useStore } from './lib/store';
import { Add } from './screens/Add';
import { Connection } from './screens/Connection';
import { Home } from './screens/Home';
import { Settings } from './screens/Settings';
import { Tray } from './screens/Tray';

/** Нажали на уведомление: на какую вкладку оно ведёт. Экран внутри вкладки откроет сам. */
const NAV_TAB: Record<string, Tab> = { servers: 'home', card: 'home', neighbors: 'home', browser_proxy: 'home', killswitch: 'settings', log: 'settings', settings: 'settings' };

export function App() {
  const { transport } = useStore();
  return transport.window === 'tray' ? <Tray /> : <MainWindow />;
}

function MainWindow() {
  const { nav, transport, toast } = useStore();
  const [tab, setTab] = useState<Tab>('home');
  const [incoming, setIncoming] = useState<Incoming | null>(null);

  // Ссылка klick://add: при запуске и каждый раз, когда приходит новая. Сама она ничего не добавляет.
  useEffect(() => {
    const take = async () => {
      const p = await transport.takePendingLink().catch(() => null);
      if (!p) return;
      if (p.kind === 'invalid') {
        toast('Ссылка не подходит', 'kl!ck принимает только https-ссылки на подписку', 'bad');
        return;
      }
      setIncoming({ url: p.url, name: p.name, seq: Date.now() });
      setTab('add');
    };
    void take();
    return transport.onAddLink(() => void take());
  }, [transport, toast]);

  useEffect(() => {
    const t = nav ? NAV_TAB[nav.target] : undefined;
    if (t) setTab(t);
  }, [nav]);

  return (
    <div className="window">
      <TitleBar />
      <main className="content" key={tab}>
        {tab === 'home' ? <Home onTab={setTab} /> : null}
        {tab === 'connection' ? <Connection /> : null}
        {tab === 'add' ? <Add onTab={setTab} incoming={incoming} /> : null}
        {tab === 'settings' ? <Settings /> : null}
      </main>
      <Toasts />
      <BottomNav tab={tab} onTab={setTab} />
      <Offline />
    </div>
  );
}
