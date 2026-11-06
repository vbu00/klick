// Время подключения: тикает само раз в секунду и не перерисовывает весь экран.

import { useEffect, useState } from 'react';

export function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [active]);
  return now;
}

export function Elapsed({ since, format }: { since: number | null; format: (since: number | null, now: number) => string }) {
  const now = useNow(since != null);
  return <>{format(since, now)}</>;
}
