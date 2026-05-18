export function formatEta(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  if (minutes <= 0) return `${secs}s`;
  return `${minutes}m ${secs}s`;
}
