import { useState } from 'react';
import type * as IPC from '../types/ipc';
import { loadWallet, unlockWallet } from '../services/ipc';

export function UnlockWallet(props: {
  wallet: IPC.WalletInfo;
  onUnlocked: (args: { wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
}) {
  const { wallet, onUnlocked } = props;
  const [password, setPassword] = useState('');
  const [rememberUnlock, setRememberUnlock] = useState(wallet.remember_unlock_enabled);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    setError(null);
    setSubmitting(true);
    try {
      const unlockRes = await unlockWallet({
        wallet_id: wallet.id,
        password,
        remember_unlock: rememberUnlock,
      });
      if ('err' in unlockRes) {
        setError(unlockRes.err.message);
        return;
      }

      const load2 = await loadWallet({ wallet_id: wallet.id });
      if ('err' in load2) {
        setError(load2.err.message);
        return;
      }

      if (load2.ok.lock_status === 'Locked') {
        setError('Wallet is still locked.');
        return;
      }

      onUnlocked({ wallet: load2.ok.wallet, accounts: load2.ok.accounts });
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form
      style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 480 }}
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <h1>Unlock wallet</h1>
      <div>
        <strong>{wallet.name}</strong>
      </div>
      <label style={{ display: 'grid', gap: 4 }}>
        <span>Password</span>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.currentTarget.value)}
        />
      </label>
      <label style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <input
          type="checkbox"
          checked={rememberUnlock}
          onChange={(e) => setRememberUnlock(e.currentTarget.checked)}
        />
        <span>Remember unlock</span>
      </label>
      {error ? <div className="text-sm text-destructive">{error}</div> : null}
      <button type="submit" disabled={!password || submitting}>
        {submitting ? 'Unlocking…' : 'Unlock'}
      </button>
    </form>
  );
}
