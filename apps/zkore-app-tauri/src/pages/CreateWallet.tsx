import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
import { createWallet, loadWallet } from '../services/ipc';

export function CreateWallet(props: {
  onCreated: (args: {
    wallet: IPC.WalletInfo;
    accounts: IPC.AccountInfo[];
    seedPhrase: string[];
  }) => void;
}) {
  const { onCreated } = props;
  const navigate = useNavigate();

  const [name, setName] = useState('');
  const [network, setNetwork] = useState<IPC.Network>('Testnet');
  const [password, setPassword] = useState('');
  const [passwordConfirm, setPasswordConfirm] = useState('');
  const [rememberUnlock, setRememberUnlock] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setError(null);
    if (!name.trim()) {
      setError('Wallet name is required.');
      return;
    }
    if (!password) {
      setError('Password is required.');
      return;
    }
    if (password !== passwordConfirm) {
      setError('Passwords do not match.');
      return;
    }

    setSubmitting(true);
    try {
      const created = await createWallet({
        name: name.trim(),
        network,
        password,
        remember_unlock: rememberUnlock,
      });
      if ('err' in created) {
        setError(created.err.message);
        return;
      }

      const load = await loadWallet({ wallet_id: created.ok.wallet.id });
      if ('err' in load) {
        setError(load.err.message);
        return;
      }

      onCreated({
        wallet: created.ok.wallet,
        accounts: load.ok.accounts,
        seedPhrase: created.ok.seed_phrase,
      });

      navigate('/seed');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 520 }}>
      <h1>Create wallet</h1>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Network</span>
        <select value={network} onChange={(e) => setNetwork(e.currentTarget.value as IPC.Network)}>
          <option value="Mainnet">Mainnet</option>
          <option value="Testnet">Testnet</option>
        </select>
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Wallet name</span>
        <input value={name} onChange={(e) => setName(e.currentTarget.value)} />
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Password</span>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.currentTarget.value)}
        />
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Confirm password</span>
        <input
          type="password"
          value={passwordConfirm}
          onChange={(e) => setPasswordConfirm(e.currentTarget.value)}
        />
      </label>

      <label style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <input
          type="checkbox"
          checked={rememberUnlock}
          onChange={(e) => setRememberUnlock(e.currentTarget.checked)}
        />
        <span>Remember unlock (stores unlock material in OS keychain)</span>
      </label>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <button type="button" onClick={submit} disabled={submitting}>
        {submitting ? 'Creating…' : 'Create wallet'}
      </button>
    </div>
  );
}

