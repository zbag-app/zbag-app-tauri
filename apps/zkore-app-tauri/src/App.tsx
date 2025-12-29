import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { HashRouter, Link, Navigate, Route, Routes } from 'react-router-dom';
import type * as IPC from './types/ipc';
import { ErrorBoundary } from './components/common/ErrorBoundary';
import { ErrorDialog } from './components/common/ErrorDialog';
import { TorErrorDialog } from './components/common/TorErrorDialog';
import { NetworkBadge } from './components/common/NetworkBadge';
import { TorStatusBadge } from './components/common/TorStatusBadge';
import { AccountSelector } from './components/wallet/AccountSelector';
import { useActiveAccount } from './hooks/useActiveAccount';
import { getTorState, listWallets, loadWallet, setTorEnabled, unlockWallet } from './services/ipc';
import { onTorStatus } from './services/events';
import { BackupChallenge } from './pages/BackupChallenge';
import { CreateWallet } from './pages/CreateWallet';
import { Home } from './pages/Home';
import { Receive } from './pages/Receive';
import { SeedDisplay } from './pages/SeedDisplay';
import { Send } from './pages/Send';
import { SendConfirm } from './pages/SendConfirm';
import { Settings } from './pages/Settings';
import { Activity } from './pages/Activity';
import { RestoreBirthday } from './pages/RestoreBirthday';
import { RestoreWallet, type RestoreFlowData } from './pages/RestoreWallet';
import { ImportKeystone } from './pages/ImportKeystone';
import { Signing } from './pages/Signing';
import { Swap } from './pages/Swap';
import { SwapDeposit } from './pages/SwapDeposit';
import { SwapFromZec } from './pages/SwapFromZec';
import { SwapQuote } from './pages/SwapQuote';
import { ServerSettings } from './pages/ServerSettings';
import { Wallets } from './pages/Wallets';
import './App.css';

const queryClient = new QueryClient();

type StartupState =
  | { kind: 'loading' }
  | { kind: 'no-wallets' }
  | { kind: 'locked'; wallet: IPC.WalletInfo }
  | { kind: 'ready'; wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }
  | { kind: 'error'; error: IPC.IpcError };

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
  const [seedPhrase, setSeedPhrase] = useState<string[] | null>(null);
  const [restoreFlow, setRestoreFlow] = useState<RestoreFlowData | null>(null);
  const [torState, setTorState] = useState<IPC.TorState | null>(null);
  const [dismissedTorError, setDismissedTorError] = useState(false);

  const activeWalletId = useMemo(() => {
    if (startup.kind === 'locked' || startup.kind === 'ready') return startup.wallet.id;
    return null;
  }, [startup]);

  const { activeAccountId, setActiveAccountId } = useActiveAccount(activeWalletId, accounts);
  const activeAccount = useMemo(() => {
    if (activeAccountId == null) return null;
    return accounts.find((a) => a.id === activeAccountId) ?? null;
  }, [accounts, activeAccountId]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    async function initTor() {
      const stateRes = await getTorState();
      if (!cancelled && 'ok' in stateRes) {
        setTorState(stateRes.ok.state);
      }

      unlisten = await onTorStatus((event) => {
        setTorState(event.state);
      });
    }

    initTor();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (torState?.enabled && torState.status === 'Error') {
      setDismissedTorError(false);
    }
  }, [torState?.enabled, torState?.status]);

  const toggleTor = async (enabled: boolean) => {
    const res = await setTorEnabled({ enabled });
    if ('ok' in res) {
      setTorState(res.ok.state);
    }
  };

  useEffect(() => {
    let cancelled = false;

    async function runStartup() {
      const walletsRes = await listWallets();
      if (cancelled) return;
      if ('err' in walletsRes) {
        setStartup({ kind: 'error', error: walletsRes.err });
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
        setStartup({ kind: 'error', error: load1.err });
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

  if (startup.kind === 'error') {
    return (
      <ErrorDialog
        title="Request failed"
        error={{ code: startup.error.code, message: startup.error.message }}
        primaryAction={{ label: 'Reload app', onClick: () => window.location.reload() }}
      />
    );
  }

  if (startup.kind === 'no-wallets') {
    return (
      <Routes>
        <Route
          path="/"
          element={
            <CreateWallet
              onCreated={(args) => {
                setSeedPhrase(args.seedPhrase);
                setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
                setAccounts(args.accounts);
              }}
            />
          }
        />
        <Route
          path="/restore"
          element={
            <RestoreWallet
              onContinue={(data) => {
                setRestoreFlow(data);
              }}
            />
          }
        />
        <Route
          path="/restore/birthday"
          element={
            <RestoreBirthday
              flow={restoreFlow}
              onClearFlow={() => setRestoreFlow(null)}
              onRestored={(args) => {
                setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
                setAccounts(args.accounts);
              }}
            />
          }
        />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    );
  }

  if (startup.kind === 'locked') {
    return (
      <UnlockGate
        wallet={startup.wallet}
        onUnlocked={(a) => {
          setStartup(a);
          if (a.kind === 'ready') {
            setAccounts(a.accounts);
          } else {
            setAccounts([]);
          }
        }}
      />
    );
  }

  return (
    <div style={{ display: 'grid', gap: 16, padding: 16 }}>
      {torState && torState.enabled && torState.status === 'Error' && !dismissedTorError ? (
        <TorErrorDialog
          state={torState}
          onClose={() => setDismissedTorError(true)}
          onDisable={() => toggleTor(false)}
          onRetry={() => toggleTor(true)}
        />
      ) : null}

      <header style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
        <strong>{startup.wallet.name}</strong>
        <NetworkBadge network={startup.wallet.network} />
        <AccountSelector
          accounts={accounts}
          activeAccountId={activeAccountId}
          onChange={setActiveAccountId}
        />
        <TorStatusBadge state={torState} />
        <nav style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
          <Link to="/">Home</Link>
          <Link to="/wallets">Wallets</Link>
          <Link to="/receive">Receive</Link>
          <Link to="/send">Send</Link>
          <Link to="/swap">Swap</Link>
          <Link to="/activity">Activity</Link>
          <Link to="/settings">Settings</Link>
        </nav>
      </header>

      <Routes>
        <Route
          path="/"
          element={
            <Home
              wallet={startup.wallet}
              accounts={accounts}
              activeAccountId={activeAccountId}
              onChangeAccount={setActiveAccountId}
            />
          }
        />
        <Route
          path="/wallets"
          element={
            <Wallets
              activeWalletId={startup.wallet.id}
              onLoaded={(resp) => {
                setSeedPhrase(null);
                setRestoreFlow(null);
                if (resp.lock_status === 'Locked') {
                  setStartup({ kind: 'locked', wallet: resp.wallet });
                  setAccounts([]);
                  return;
                }
                setStartup({ kind: 'ready', wallet: resp.wallet, accounts: resp.accounts });
                setAccounts(resp.accounts);
              }}
            />
          }
        />
        <Route
          path="/create"
          element={
            <CreateWallet
              onCreated={(args) => {
                setSeedPhrase(args.seedPhrase);
                setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
                setAccounts(args.accounts);
              }}
            />
          }
        />
        <Route
          path="/restore"
          element={
            <RestoreWallet
              onContinue={(data) => {
                setRestoreFlow(data);
              }}
            />
          }
        />
        <Route
          path="/restore/birthday"
          element={
            <RestoreBirthday
              flow={restoreFlow}
              onClearFlow={() => setRestoreFlow(null)}
              onRestored={(args) => {
                setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
                setAccounts(args.accounts);
              }}
            />
          }
        />
        <Route path="/receive" element={<Receive activeAccountId={activeAccountId} />} />
        <Route path="/send" element={<Send activeAccount={activeAccount} />} />
        <Route path="/send/confirm" element={<SendConfirm walletId={startup.wallet.id} />} />
        <Route path="/signing" element={<Signing walletId={startup.wallet.id} />} />
        <Route
          path="/swap"
          element={<Swap wallet={startup.wallet} activeAccountId={activeAccountId} />}
        />
        <Route path="/swap/from-zec" element={<SwapFromZec wallet={startup.wallet} />} />
        <Route path="/swap/quote" element={<SwapQuote />} />
        <Route path="/swap/deposit" element={<SwapDeposit />} />
        <Route
          path="/activity"
          element={<Activity walletId={startup.wallet.id} activeAccountId={activeAccountId} />}
        />
        <Route
          path="/settings"
          element={<Settings wallet={startup.wallet} torState={torState} onSetTorEnabled={toggleTor} />}
        />
        <Route path="/settings/servers" element={<ServerSettings wallet={startup.wallet} />} />
        <Route
          path="/keystone/import"
          element={
            <ImportKeystone
              walletId={startup.wallet.id}
              onAccountsUpdated={(next) => setAccounts(next)}
            />
          }
        />
        <Route
          path="/seed"
          element={
            <SeedDisplay
              seedPhrase={seedPhrase ?? []}
              onCleared={() => setSeedPhrase(null)}
            />
          }
        />
        <Route
          path="/backup"
          element={<BackupChallenge walletId={startup.wallet.id} onVerified={() => {}} />}
        />
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
        <ErrorBoundary>
          <AppInner />
        </ErrorBoundary>
      </HashRouter>
    </QueryClientProvider>
  );
}
