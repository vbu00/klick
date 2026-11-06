import { MainWindow } from './bm/Shell';
import { useStore } from './lib/store';
import { Tray } from './screens/Tray';

export function App() {
  const { transport } = useStore();
  return transport.window === 'tray' ? <Tray /> : <MainWindow />;
}
