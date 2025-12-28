import type * as IPC from '../../types/ipc';

function formatEta(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  if (minutes <= 0) return `${secs}s`;
  return `${minutes}m ${secs}s`;
}

export function SyncProgressWidget(props: { progress: IPC.SyncProgress }) {
  const { progress } = props;

  return (
    <div style={{ display: 'grid', gap: 6 }}>
      <div style={{ display: 'flex', gap: 12, alignItems: 'baseline', flexWrap: 'wrap' }}>
        <strong>{progress.phase}</strong>
        <span style={{ fontSize: 12, opacity: 0.8 }}>{progress.progress_percent}%</span>
        {progress.eta_seconds !== null ? (
          <span style={{ fontSize: 12, opacity: 0.8 }}>ETA {formatEta(progress.eta_seconds)}</span>
        ) : null}
      </div>

      <progress value={progress.progress_percent} max={100} style={{ width: '100%' }} />

      <div style={{ fontSize: 12, opacity: 0.8 }}>
        frontier {progress.scan_frontier_height} / tip {progress.wallet_tip_height}
      </div>
    </div>
  );
}

