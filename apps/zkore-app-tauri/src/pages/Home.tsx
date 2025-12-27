import { useEffect, useState } from 'react';
import type * as IPC from '../types/ipc';
import { BackupReminder } from '../components/common/BackupReminder';
import { AccountSelector } from '../components/wallet/AccountSelector';
import { onSyncProgress } from '../services/events';
import { getBalance, getSyncProgress, getWalletStatus, startSync } from '../services/ipc';

export function Home(props: {
  wallet: IPC.WalletInfo;
  accounts: IPC.AccountInfo[];
  activeAccountId: number | null;
  onChangeAccount: (accountId: number) => void;
}) {
  const { wallet, accounts, activeAccountId, onChangeAccount } = props;

  const [status, setStatus] = useState<IPC.WalletStatus | null>(null);
  const [balance, setBalance] = useState<IPC.Balance | null>(null);
  const [sync, setSync] = useState<IPC.SyncProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshStatus = async () => {
    const res = await getWalletStatus({ wallet_id: wallet.id });
    if ('err' in res) {
      setError(res.err.message);
      return;
    }
    setStatus(res.ok.status);
  };

  useEffect(() => {
    refreshStatus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wallet.id]);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      if (activeAccountId === null) {
        setBalance(null);
        return;
      }
      const res = await getBalance({ account_id: activeAccountId });
      if (cancelled) return;
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setBalance(res.ok.balance);
    }

    run();
    return () => {
      cancelled = true;
    };
  }, [activeAccountId]);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      const res = await getSyncProgress({ wallet_id: wallet.id });
      if (cancelled) return;
      if ('err' in res) {
        return;
      }
      setSync(res.ok.progress);
    }

    run();
    return () => {
      cancelled = true;
    };
  }, [wallet.id]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    onSyncProgress((evt) => setSync(evt.progress))
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  const start = async () => {
    const res = await startSync({ wallet_id: wallet.id });
    if ('err' in res) {
      setError(res.err.message);
    }
  };

  const backupRequired = status?.backup_status === 'Required';

  return (
    <div style={{ display: 'grid', gap: 12 }}>
      <div style={{ display: 'flex', gap: 16, alignItems: 'center', flexWrap: 'wrap' }}>
        <AccountSelector
          accounts={accounts}
          activeAccountId={activeAccountId}
          onChange={onChangeAccount}
        />
        <button type="button" onClick={start}>
          Start sync
        </button>
        <button type="button" disabled={backupRequired}>
          Send (coming soon)
        </button>
      </div>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      {status ? <BackupReminder walletId={wallet.id} status={status} /> : null}

      <div style={{ display: 'grid', gap: 8 }}>
        <h2 style={{ margin: 0 }}>Balance</h2>
        {balance ? (
          <div style={{ display: 'grid', gap: 4 }}>
            <div>Shielded spendable: {balance.shielded_spendable}</div>
            <div>Shielded pending: {balance.shielded_pending}</div>
            <div>Transparent total: {balance.transparent_total}</div>
            <div>Total: {balance.total}</div>
          </div>
        ) : (
          <div>{activeAccountId === null ? 'No active account.' : 'Loading…'}</div>
        )}
      </div>

      <div style={{ display: 'grid', gap: 8 }}>
        <h2 style={{ margin: 0 }}>Sync</h2>
        {sync ? (
          <div>
            {sync.phase} — {sync.progress_percent}% (frontier {sync.scan_frontier_height} / tip{' '}
            {sync.wallet_tip_height})
          </div>
        ) : (
          <div>Loading…</div>
        )}
      </div>
    </div>
  );
}

