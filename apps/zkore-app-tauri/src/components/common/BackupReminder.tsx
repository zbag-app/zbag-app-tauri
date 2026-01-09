import { useNavigate } from 'react-router-dom';
import type * as IPC from '../../types/ipc';
import { ViewSeedPhraseDialog } from './ViewSeedPhraseDialog';
import { Button } from '../ui/button';

export function BackupReminder(props: { walletId: string; status: IPC.WalletStatus }) {
  const { walletId, status } = props;
  const navigate = useNavigate();

  const required = status.backup_status === 'Required';

  return (
    <div className="rounded-xl border border-warning/50 bg-warning/10 p-3 grid gap-2">
      <strong className="text-warning">Backup</strong>
      <div className="text-sm">
        Status: <strong>{status.backup_status}</strong>
      </div>
      {required ? (
        <div className="text-sm text-muted-foreground">
          Backup is required before sending funds.
        </div>
      ) : null}
      <div className="flex gap-2 flex-wrap">
        {required ? (
          <Button variant="outline" size="sm" onClick={() => navigate('/backup')}>
            Verify backup
          </Button>
        ) : null}
        <ViewSeedPhraseDialog walletId={walletId} triggerLabel="View seed phrase" />
      </div>
    </div>
  );
}
