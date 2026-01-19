export function formatCountdown(deadlineMs: number, nowMs: number): string {
  const ms = deadlineMs - nowMs;
  const secs = Math.max(0, Math.floor(ms / 1000));
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}:${rem.toString().padStart(2, '0')}`;
}
