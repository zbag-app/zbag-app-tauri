import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
import { importUfvk, loadWallet } from '../services/ipc';

type ImportMode = 'paste' | 'scan';

export function ImportKeystone(props: {
  walletId: string;
  onAccountsUpdated: (accounts: IPC.AccountInfo[]) => void;
}) {
  const { walletId, onAccountsUpdated } = props;
  const navigate = useNavigate();

  const [mode, setMode] = useState<ImportMode>('paste');
  const [name, setName] = useState('Keystone');
  const [ufvk, setUfvk] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const canSubmit = useMemo(() => {
    if (submitting) return false;
    if (!name.trim()) return false;
    if (!ufvk.trim()) return false;
    return true;
  }, [name, ufvk, submitting]);

  const submit = async () => {
    setSubmitting(true);
    setError(null);

    const res = await importUfvk({ wallet_id: walletId, ufvk, name });
    if ('err' in res) {
      setSubmitting(false);
      setError(res.err.message);
      return;
    }

    const reloaded = await loadWallet({ wallet_id: walletId });
    setSubmitting(false);
    if ('err' in reloaded) {
      setError(reloaded.err.message);
      return;
    }
    if (reloaded.ok.lock_status === 'Locked') {
      setError('Wallet is locked. Unlock and try again.');
      return;
    }

    onAccountsUpdated(reloaded.ok.accounts);
    navigate('/');
  };

  return (
    <div style={{ display: 'grid', gap: 12, maxWidth: 720 }}>
      <h1>Import Keystone</h1>

      <div style={{ fontSize: 14, opacity: 0.85 }}>
        Import a Unified Full Viewing Key (UFVK) to create a watch-only account. Spending from this
        account requires Keystone signing.
      </div>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Account name</span>
        <input value={name} onChange={(e) => setName(e.currentTarget.value)} />
      </label>

      <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
        <button
          type="button"
          onClick={() => setMode('paste')}
          disabled={mode === 'paste'}
        >
          Paste UFVK
        </button>
        <button
          type="button"
          onClick={() => setMode('scan')}
          disabled={mode === 'scan'}
        >
          Scan QR (coming soon)
        </button>
      </div>

      {mode === 'paste' ? (
        <label style={{ display: 'grid', gap: 4 }}>
          <span>UFVK</span>
          <textarea
            value={ufvk}
            onChange={(e) => setUfvk(e.currentTarget.value)}
            rows={4}
            placeholder="uview..."
            style={{ fontFamily: 'monospace' }}
          />
        </label>
      ) : (
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          QR scanning will be available in a future update. For now, paste the UFVK.
        </div>
      )}

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <button type="button" onClick={() => navigate(-1)} disabled={submitting}>
          Back
        </button>
        <button type="button" onClick={submit} disabled={!canSubmit}>
          {submitting ? 'Importing…' : 'Import'}
        </button>
      </div>
    </div>
  );
}

