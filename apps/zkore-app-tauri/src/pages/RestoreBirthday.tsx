import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
import { loadWallet, restoreWallet, startSync } from '../services/ipc';
import type { RestoreFlowData } from './RestoreWallet';

export function RestoreBirthday(props: {
  flow: RestoreFlowData | null;
  onClearFlow: () => void;
  onRestored: (args: { wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
}) {
  const { flow, onClearFlow, onRestored } = props;
  const navigate = useNavigate();

  const [birthdayDate, setBirthdayDate] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const goBack = () => {
    onClearFlow();
    setBirthdayDate('');
    navigate('/restore');
  };

  const submit = async () => {
    if (!flow) return;
    setError(null);

    let birthdayMs: number | null = null;
    if (birthdayDate) {
      birthdayMs = Date.parse(birthdayDate);
      if (Number.isNaN(birthdayMs)) {
        setError('Invalid date.');
        return;
      }
    }

    setSubmitting(true);
    try {
      const restored = await restoreWallet({ ...flow, birthday_date: birthdayMs });
      if ('err' in restored) {
        setError(restored.err.message);
        return;
      }

      const walletId = restored.ok.wallet.id;

      const load = await loadWallet({ wallet_id: walletId });
      if ('err' in load) {
        setError(load.err.message);
        return;
      }

      onClearFlow();
      onRestored({ wallet: load.ok.wallet, accounts: load.ok.accounts });

      await startSync({ wallet_id: walletId });
      navigate('/');
    } finally {
      setSubmitting(false);
    }
  };

  if (!flow) {
    return (
      <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 640 }}>
        <h1 style={{ margin: 0 }}>Restore wallet</h1>
        <p style={{ margin: 0 }}>Restore details are missing.</p>
        <Link to="/restore">Start restore</Link>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 640 }}>
      <h1 style={{ margin: 0 }}>First transaction date (optional)</h1>
      <p style={{ margin: 0 }}>
        If you remember roughly when this wallet first received funds, adding a date can reduce
        restore scan time.
      </p>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Date</span>
        <input
          type="date"
          value={birthdayDate}
          onChange={(e) => setBirthdayDate(e.currentTarget.value)}
        />
      </label>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        <button type="button" onClick={goBack} disabled={submitting}>
          Back
        </button>
        <button type="button" onClick={submit} disabled={submitting}>
          {submitting ? 'Restoring…' : 'Restore'}
        </button>
      </div>
    </div>
  );
}

