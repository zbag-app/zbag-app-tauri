import { useEffect, useState } from 'react';

export function useNowMs(enabled: boolean, intervalMs = 1000): number {
  const [nowMs, setNowMs] = useState(() => Date.now());

  useEffect(() => {
    if (!enabled) return;
    setNowMs(Date.now());

    const id = window.setInterval(() => setNowMs(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [enabled, intervalMs]);

  return nowMs;
}
