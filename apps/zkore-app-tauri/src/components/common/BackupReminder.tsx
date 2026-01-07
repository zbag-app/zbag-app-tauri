import { useNavigate } from 'react-router-dom';
import type * as IPC from '../../types/ipc';
import { ViewSeedPhraseDialog } from './ViewSeedPhraseDialog';

export function BackupReminder(props: { walletId: string; status: IPC.WalletStatus }) {
  const { walletId, status } = props;
  const navigate = useNavigate();

  const required = status.backup_status === 'Required';

  return (
    <div
      style={{
        border: '1px solid #f1c40f',
        background: '#fff8e1',
        borderRadius: 12,
        padding: 12,
        display: 'grid',
        gap: 8,
      }}
    >
      <strong>Backup</strong>
      <div style={{ fontSize: 14 }}>
        Status: <strong>{status.backup_status}</strong>
      </div>
      {required ? (
        <div style={{ fontSize: 14 }}>
          Backup is required before sending funds.
        </div>
      ) : null}
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        {required ? (
          <button type="button" onClick={() => navigate('/backup')}>
            Verify backup
          </button>
        ) : null}
        <ViewSeedPhraseDialog walletId={walletId} triggerLabel="View seed phrase" />
      </div>
    </div>
  );
}

