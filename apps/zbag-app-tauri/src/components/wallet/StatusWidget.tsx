import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import type * as IPC from '../../types/ipc';
import { onWalletStatus } from '../../services/events';
import { getWalletStatus } from '../../services/ipc';
import { ShieldPrompt } from './ShieldPrompt';
import { formatEta } from '../../utils/time';
import { formatZatoshisToZec } from '../../utils/zec';
import { Button } from '../ui/button';

function getSyncLabel(syncStatus: IPC.SyncStatus): string {
  if (syncStatus === 'Synced') return 'Synced';
  if ('Syncing' in syncStatus) {
    // Cap at 99% while still syncing (matches SyncProgressWidget)
    const percent = Math.min(syncStatus.Syncing.progress_percent, 99);
    return `Syncing (${percent}%)`;
  }
  if ('Offline' in syncStatus) {
    const retryInSeconds = syncStatus.Offline.retry_in_seconds;
    if (retryInSeconds <= 0) return 'Offline (retrying...)';
    return `Offline (retry in ${formatEta(retryInSeconds)})`;
  }
  if ('Error' in syncStatus) {
    return `Error: ${syncStatus.Error.message}`;
  }
  return 'Unknown';
}

function Card(props: { title: string; children: ReactNode }) {
  return (
    <div className="rounded-xl border border-border p-3 grid gap-2">
      <strong>{props.title}</strong>
      {props.children}
    </div>
  );
}

export function StatusWidget(props: {
  walletId: string;
  walletType: IPC.WalletType;
  activeAccountId: number | null;
  onStatusChange?: (status: IPC.WalletStatus) => void;
}) {
  const { walletId, walletType, activeAccountId, onStatusChange } = props;
  const navigate = useNavigate();

  const [status, setStatus] = useState<IPC.WalletStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const backupRequired = status?.backup_status === 'Required';
  const shieldAmount =
    status?.shield_status && typeof status.shield_status === 'object' && 'Available' in status.shield_status
      ? status.shield_status.Available.amount
      : null;

  const syncLabel = useMemo(() => {
    if (!status) return null;
    return getSyncLabel(status.sync_status);
  }, [status]);

  const refresh = async () => {
    const res = await getWalletStatus({ wallet_id: walletId });
    if ('err' in res) {
      setError(res.err.message);
      return;
    }
    setError(null);
    setStatus(res.ok.status);
    onStatusChange?.(res.ok.status);
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [walletId]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    onWalletStatus((evt) => {
      setStatus(evt.status);
      onStatusChange?.(evt.status);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, [onStatusChange]);

  return (
    <div className="grid gap-2.5">
      <h2 className="text-lg font-semibold m-0">Status</h2>
      {error ? <div className="text-sm text-destructive">{error}</div> : null}

      <div className="grid grid-cols-[repeat(auto-fit,minmax(240px,1fr))] gap-2.5">
        {backupRequired ? (
          <Card title="Backup">
            <div className="text-sm">
              Status: <strong>Required</strong>
            </div>
            <div className="text-xs text-muted-foreground">Backup is required before sending funds.</div>
            <Button variant="outline" size="sm" onClick={() => navigate('/backup')}>
              Verify backup
            </Button>
          </Card>
        ) : null}

        <Card title="Sync">
          <div className="text-sm">
            Status: <strong>{syncLabel ?? 'Loading...'}</strong>
          </div>
        </Card>

        <Card title="Shielding">
          <div className="text-sm">
            Status:{' '}
            <strong>
              {walletType === 'WatchOnly'
                ? 'N/A (hardware wallet)'
                : shieldAmount && shieldAmount !== '0'
                  ? `Available (${formatZatoshisToZec(shieldAmount)} ZEC)`
                  : 'None'}
            </strong>
          </div>
          {shieldAmount && shieldAmount !== '0' && activeAccountId !== null && walletType !== 'WatchOnly' ? (
            <ShieldPrompt
              walletId={walletId}
              accountId={activeAccountId}
              transparentTotal={shieldAmount}
              disabled={backupRequired}
              onShielded={refresh}
            />
          ) : null}
        </Card>

        <Card title="Privacy">
          <div className="text-sm">
            Posture: <strong>{status?.privacy_posture ?? 'Loading...'}</strong>
          </div>
          {status?.privacy_posture === 'Optimal' ? (
            <div className="text-xs text-muted-foreground">Backed up and shielded-by-default.</div>
          ) : (
            <div className="text-xs text-muted-foreground">
              Complete backup and shield transparent funds to improve privacy.
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
