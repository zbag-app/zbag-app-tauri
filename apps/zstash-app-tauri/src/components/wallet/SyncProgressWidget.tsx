import type * as IPC from '../../types/ipc';

function formatEta(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  if (minutes <= 0) return `${secs}s`;
  return `${minutes}m ${secs}s`;
}

// Map backend phases to display string - collapse Downloading/Scanning to "Syncing"
function getDisplayPhase(phase: IPC.SyncPhase): string {
  switch (phase) {
    case 'Downloading':
    case 'Scanning':
      return 'Syncing';
    case 'CatchingUp':
      return 'Catching up';
    default:
      return phase;
  }
}

export function SyncProgressWidget(props: { progress: IPC.SyncProgress }) {
  const { progress } = props;

  // Cap at 99% unless Idle to avoid "100% but still syncing" confusion
  const displayPercent =
    progress.phase === 'Idle' || progress.phase === 'CatchingUp'
      ? progress.progress_percent
      : Math.min(progress.progress_percent, 99);

  return (
    <div style={{ display: 'grid', gap: 6 }}>
      <div style={{ display: 'flex', gap: 12, alignItems: 'baseline', flexWrap: 'wrap' }}>
        <strong>{getDisplayPhase(progress.phase)}</strong>
        <span style={{ fontSize: 12, opacity: 0.8 }}>{displayPercent}%</span>
        {progress.eta_seconds !== null ? (
          <span style={{ fontSize: 12, opacity: 0.8 }}>ETA {formatEta(progress.eta_seconds)}</span>
        ) : null}
      </div>

      <progress value={displayPercent} max={100} style={{ width: '100%' }} />

      <div style={{ fontSize: 12, opacity: 0.8 }}>
        frontier {progress.scan_frontier_height} / tip {progress.wallet_tip_height}
      </div>
    </div>
  );
}
