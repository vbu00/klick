import { useEffect, useState } from 'react';
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
  const { nav, incoming } = useStore();
  const [tab, setTab] = useState<Tab>('home');

  useEffect(() => {
    // `conn:<id>` — показать подключение на главной («Уже добавлено»).
    const t = nav ? (nav.target.startsWith('conn:') ? 'home' : NAV_TAB[nav.target]) : undefined;
    if (t) setTab(t);
  }, [nav]);

  // Ссылка klick://add: экран «Добавить» с уже вставленной подпиской.
  useEffect(() => {
    if (incoming) setTab('add');
  }, [incoming]);

  return (
    <div className="window">
      <TitleBar />
      <main className="content" key={tab}>
        {tab === 'home' ? <Home onTab={setTab} /> : null}
        {tab === 'connection' ? <Connection /> : null}
        {tab === 'add' ? <Add onTab={setTab} /> : null}
        {tab === 'settings' ? <Settings /> : null}
      </main>
      <Toasts />
      <BottomNav tab={tab} onTab={setTab} />
      <Offline />
    </div>
  );
}
