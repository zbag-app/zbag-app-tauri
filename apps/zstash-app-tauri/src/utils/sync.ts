export function isEffectivelyAtTip(progress: {
  wallet_tip_height: number;
  scan_frontier_height: number;
}): boolean {
  return progress.wallet_tip_height > 0 && progress.scan_frontier_height >= progress.wallet_tip_height;
}
