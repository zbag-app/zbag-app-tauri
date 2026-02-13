export function isEffectivelyAtTip(progress: {
  wallet_tip_height: number;
  scan_frontier_height: number;
}): boolean {
  return progress.wallet_tip_height > 0 && progress.scan_frontier_height >= progress.wallet_tip_height;
}

export function getDisplaySyncPercent(progress: {
  phase: string;
  progress_percent: number;
  wallet_tip_height: number;
  scan_frontier_height: number;
}): number {
  const showSyncedTerminal = progress.phase === 'CatchingUp' && isEffectivelyAtTip(progress);
  if (progress.phase === 'Idle' || showSyncedTerminal) {
    return showSyncedTerminal ? 100 : progress.progress_percent;
  }
  // Cap at 99% for non-terminal phases to avoid "100% but still syncing" confusion.
  return Math.min(progress.progress_percent, 99);
}
