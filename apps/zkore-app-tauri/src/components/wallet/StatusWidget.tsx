import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import type * as IPC from '../../types/ipc';
import { onWalletStatus } from '../../services/events';
import { getWalletStatus } from '../../services/ipc';
import { ShieldPrompt } from './ShieldPrompt';
import { formatZatoshisToZec } from '../../utils/zec';

function Card(props: { title: string; children: ReactNode }) {
  return (
    <div style={{ border: '1px solid #e5e7eb', borderRadius: 12, padding: 12, display: 'grid', gap: 8 }}>
      <strong>{props.title}</strong>
      {props.children}
    </div>
  );
}

export function StatusWidget(props: {
  walletId: string;
  activeAccountId: number | null;
  onStatusChange?: (status: IPC.WalletStatus) => void;
}) {
  const { walletId, activeAccountId, onStatusChange } = props;
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
    if (status.sync_status === 'Synced') return 'Synced';
    if (typeof status.sync_status === 'object' && 'Syncing' in status.sync_status) {
      // Cap at 99% while still syncing (matches SyncProgressWidget)
      const percent = Math.min(status.sync_status.Syncing.progress_percent, 99);
      return `Syncing (${percent}%)`;
    }
    if (typeof status.sync_status === 'object' && 'Error' in status.sync_status) {
      return `Error: ${status.sync_status.Error.message}`;
    }
    return 'Unknown';
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
    <div style={{ display: 'grid', gap: 10 }}>
      <h2 style={{ margin: 0 }}>Status</h2>
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 10 }}>
        {backupRequired ? (
          <Card title="Backup">
            <div style={{ fontSize: 14 }}>
              Status: <strong>Required</strong>
            </div>
            <div style={{ fontSize: 12, opacity: 0.85 }}>Backup is required before sending funds.</div>
            <button type="button" onClick={() => navigate('/backup')}>
              Verify backup
            </button>
          </Card>
        ) : null}

        <Card title="Sync">
          <div style={{ fontSize: 14 }}>
            Status: <strong>{syncLabel ?? 'Loading…'}</strong>
          </div>
        </Card>

        <Card title="Shielding">
          <div style={{ fontSize: 14 }}>
            Status:{' '}
            <strong>{shieldAmount && shieldAmount !== '0' ? `Available (${formatZatoshisToZec(shieldAmount)} ZEC)` : 'None'}</strong>
          </div>
          {shieldAmount && shieldAmount !== '0' && activeAccountId !== null ? (
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
          <div style={{ fontSize: 14 }}>
            Posture: <strong>{status?.privacy_posture ?? 'Loading…'}</strong>
          </div>
          {status?.privacy_posture === 'Optimal' ? (
            <div style={{ fontSize: 12, opacity: 0.85 }}>Backed up and shielded-by-default.</div>
          ) : (
            <div style={{ fontSize: 12, opacity: 0.85 }}>
              Complete backup and shield transparent funds to improve privacy.
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
