import { useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';

export type RestoreFlowData = {
  name: string;
  network: IPC.Network;
  password: string;
  remember_unlock: boolean;
  seed_phrase: string;
};

function countWords(value: string): number {
  return value
    .trim()
    .split(/\s+/)
    .map((w) => w.trim())
    .filter(Boolean).length;
}

export function RestoreWallet(props: { onContinue: (data: RestoreFlowData) => void }) {
  const { onContinue } = props;
  const navigate = useNavigate();

  const [name, setName] = useState('');
  const [network, setNetwork] = useState<IPC.Network>('Testnet');
  const [password, setPassword] = useState('');
  const [passwordConfirm, setPasswordConfirm] = useState('');
  const [rememberUnlock, setRememberUnlock] = useState(false);
  const [seedPhrase, setSeedPhrase] = useState('');
  const [error, setError] = useState<string | null>(null);

  const seedWordCount = useMemo(() => countWords(seedPhrase), [seedPhrase]);

  const submit = () => {
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
    if (seedWordCount !== 24) {
      setError('Seed phrase must be exactly 24 words.');
      return;
    }

    onContinue({
      name: name.trim(),
      network,
      password,
      remember_unlock: rememberUnlock,
      seed_phrase: seedPhrase.trim(),
    });

    setSeedPhrase('');
    setPassword('');
    setPasswordConfirm('');

    navigate('/restore/birthday');
  };

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 640 }}>
      <header style={{ display: 'flex', gap: 12, alignItems: 'baseline', flexWrap: 'wrap' }}>
        <h1 style={{ margin: 0 }}>Restore wallet</h1>
        <Link to="/" style={{ fontSize: 14 }}>
          Create new wallet
        </Link>
      </header>

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

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Seed phrase (24 words)</span>
        <textarea
          value={seedPhrase}
          onChange={(e) => setSeedPhrase(e.currentTarget.value)}
          rows={4}
          style={{ fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace' }}
          placeholder="Enter your 24-word seed phrase…"
        />
        <span style={{ fontSize: 12, opacity: 0.8 }}>{seedWordCount} / 24 words</span>
      </label>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <button type="button" onClick={submit}>
        Continue
      </button>
    </div>
  );
}

