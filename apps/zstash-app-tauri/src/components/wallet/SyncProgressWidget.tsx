import { useEffect, useState } from 'react';
import type * as IPC from '../../types/ipc';
import { formatEta } from '../../utils/time';

// NOTE: This countdown is an approximation based on the last `retry_in_seconds` value received from
// the backend. Because we don't have an absolute retry timestamp, small drift (event latency + JS
// timer drift) is expected.
function useRetryCountdown(phase: IPC.SyncPhase, retryInSeconds: number | undefined) {
  const [remainingSeconds, setRemainingSeconds] = useState<number | null>(retryInSeconds ?? null);

  useEffect(() => {
    if (phase !== 'Offline' && phase !== 'Error') {
      setRemainingSeconds(null);
      return;
    }

    if (retryInSeconds == null) {
      setRemainingSeconds(null);
      return;
    }

    if (retryInSeconds <= 0) {
      setRemainingSeconds(0);
      return;
    }

    const endAtMs = Date.now() + retryInSeconds * 1000;
    const tick = () => {
      const remaining = Math.max(0, Math.ceil((endAtMs - Date.now()) / 1000));
      setRemainingSeconds(remaining);
    };

    // Tick immediately so the UI updates without waiting for the first interval.
    tick();
    const intervalId = window.setInterval(tick, 1000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [phase, retryInSeconds]);

  return remainingSeconds;
}

// Map backend phases to display string - collapse Downloading/Scanning to "Syncing"
function getDisplayPhase(phase: Exclude<IPC.SyncPhase, 'Offline' | 'Error'>): string {
  switch (phase) {
    case 'Idle':
      return 'Idle';
    case 'Preparing':
      return 'Preparing';
    case 'Downloading':
    case 'Scanning':
      return 'Syncing';
    case 'Enhancing':
      return 'Enhancing';
    case 'CatchingUp':
      return 'Catching up';
    default:
      // Exhaustiveness check: if a new phase is added to the contract, this forces an update.
      const _exhaustive: never = phase;
      return _exhaustive;
  }
}

function InterruptedSyncProgress(props: {
  label: 'Offline' | 'Error';
  labelColor: string;
  statusText: string;
  detailsText?: string;
  progress: IPC.SyncProgress;
}) {
  const { label, labelColor, statusText, detailsText, progress } = props;

  return (
    <div style={{ display: 'grid', gap: 6 }}>
      <div style={{ display: 'flex', gap: 12, alignItems: 'baseline', flexWrap: 'wrap' }}>
        <strong style={{ color: labelColor }}>{label}</strong>
        <span style={{ fontSize: 12, opacity: 0.8 }}>{statusText}</span>
      </div>

      {detailsText ? <div style={{ fontSize: 12, opacity: 0.9 }}>{detailsText}</div> : null}

      <progress value={progress.progress_percent} max={100} style={{ width: '100%' }} />

      <div style={{ fontSize: 12, opacity: 0.8 }}>
        Last synced to block {progress.scan_frontier_height}
        {progress.wallet_tip_height > 0 ? ` / tip ${progress.wallet_tip_height}` : ''}
      </div>
    </div>
  );
}

export function SyncProgressWidget(props: { progress: IPC.SyncProgress }) {
  const { progress } = props;
  const retryInSeconds = useRetryCountdown(progress.phase, progress.retry_in_seconds);

  // Offline state: show retry countdown, preserve last known progress
  if (progress.phase === 'Offline') {
    const statusText =
      retryInSeconds != null && retryInSeconds > 0
        ? `Retrying in ${formatEta(retryInSeconds)}`
        : 'Retrying...';
    return (
      <InterruptedSyncProgress
        label="Offline"
        labelColor="hsl(var(--warning))"
        statusText={statusText}
        progress={progress}
      />
    );
  }

  // Error state: show error indicator with retry countdown and last known progress
  if (progress.phase === 'Error') {
    const statusText =
      retryInSeconds != null && retryInSeconds > 0
        ? `Sync failed - Retrying in ${formatEta(retryInSeconds)}`
        : 'Sync failed - Retrying...';
    return (
      <InterruptedSyncProgress
        label="Error"
        labelColor="hsl(var(--destructive))"
        statusText={statusText}
        detailsText={progress.error_message}
        progress={progress}
      />
    );
  }

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
