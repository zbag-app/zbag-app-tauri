import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { HashRouter, Route, Routes } from 'react-router-dom';
import type * as IPC from './types/ipc';
import { AccountSelector } from './components/wallet/AccountSelector';
import { useActiveAccount } from './hooks/useActiveAccount';
import { listWallets, loadWallet, unlockWallet } from './services/ipc';
import './App.css';

const queryClient = new QueryClient();

type StartupState =
  | { kind: 'loading' }
  | { kind: 'no-wallets' }
  | { kind: 'locked'; wallet: IPC.WalletInfo }
  | { kind: 'ready'; wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }
  | { kind: 'error'; message: string };

function pickMostRecentWallet(wallets: IPC.WalletInfo[]): IPC.WalletInfo {
  return wallets
    .slice()
    .sort((a, b) => {
      const aT = a.last_opened_at ?? a.created_at;
      const bT = b.last_opened_at ?? b.created_at;
      return bT - aT;
    })[0];
}

function AppInner() {
  const [startup, setStartup] = useState<StartupState>({ kind: 'loading' });
  const [accounts, setAccounts] = useState<IPC.AccountInfo[]>([]);

  const activeWalletId = useMemo(() => {
    if (startup.kind === 'locked' || startup.kind === 'ready') return startup.wallet.id;
    return null;
  }, [startup]);

  const { activeAccountId, setActiveAccountId } = useActiveAccount(activeWalletId, accounts);

  useEffect(() => {
    let cancelled = false;

    async function runStartup() {
      const walletsRes = await listWallets();
      if (cancelled) return;
      if ('err' in walletsRes) {
        setStartup({ kind: 'error', message: walletsRes.err.message });
        return;
      }

      const wallets = walletsRes.ok.wallets;
      if (wallets.length === 0) {
        setStartup({ kind: 'no-wallets' });
        return;
      }

      const mostRecent = pickMostRecentWallet(wallets);
      const load1 = await loadWallet({ wallet_id: mostRecent.id });
      if (cancelled) return;
      if ('err' in load1) {
        setStartup({ kind: 'error', message: load1.err.message });
        return;
      }

      if (load1.ok.lock_status === 'Locked') {
        setStartup({ kind: 'locked', wallet: load1.ok.wallet });
        setAccounts([]);
        return;
      }

      setStartup({ kind: 'ready', wallet: load1.ok.wallet, accounts: load1.ok.accounts });
      setAccounts(load1.ok.accounts);
    }

    runStartup();
    return () => {
      cancelled = true;
    };
  }, []);

  if (startup.kind === 'loading') return <div>Loading…</div>;

  if (startup.kind === 'error') return <div>Error: {startup.message}</div>;

  if (startup.kind === 'no-wallets') return <div>No wallets found. Go to onboarding.</div>;

  if (startup.kind === 'locked') {
    return <UnlockGate wallet={startup.wallet} onUnlocked={(a) => setStartup(a)} />;
  }

  return (
    <div style={{ display: 'grid', gap: 16, padding: 16 }}>
      <header style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
        <strong>{startup.wallet.name}</strong>
        <AccountSelector
          accounts={accounts}
          activeAccountId={activeAccountId}
          onChange={setActiveAccountId}
        />
      </header>

      <Routes>
        <Route path="/" element={<div>Home</div>} />
      </Routes>
    </div>
  );
}

function UnlockGate(props: {
  wallet: IPC.WalletInfo;
  onUnlocked: (state: Extract<StartupState, { kind: 'ready' | 'locked' }>) => void;
}) {
  const { wallet, onUnlocked } = props;
  const [password, setPassword] = useState('');
  const [rememberUnlock, setRememberUnlock] = useState(wallet.remember_unlock_enabled);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setError(null);
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
      onUnlocked({ kind: 'locked', wallet: load2.ok.wallet });
      return;
    }

    onUnlocked({ kind: 'ready', wallet: load2.ok.wallet, accounts: load2.ok.accounts });
  };

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 480 }}>
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
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}
      <button type="button" onClick={submit} disabled={!password}>
        Unlock
      </button>
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <AppInner />
      </HashRouter>
    </QueryClientProvider>
  );
}
