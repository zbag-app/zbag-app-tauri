import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { HashRouter, Navigate, Route, Routes, useNavigate } from 'react-router-dom';
import { Shield } from 'lucide-react';
import type * as IPC from './types/ipc';
import { Logo } from './components/brand/Logo';
import { ErrorBoundary } from './components/common/ErrorBoundary';
import { ErrorDialog } from './components/common/ErrorDialog';
import { TorErrorDialog } from './components/common/TorErrorDialog';
import { WalletPicker } from './components/wallet/WalletPicker';
import { AppShell } from './components/layout/AppShell';
import { Button } from './components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from './components/ui/card';
import { Input } from './components/ui/input';
import { Label } from './components/ui/label';
import { useActiveAccount } from './hooks/useActiveAccount';
import { useThrottledCallback } from './hooks/useThrottle';
import { getTorState, getSyncProgress, listWallets, loadWallet, lockWallet, setTorEnabled, unlockWallet } from './services/ipc';
import { onTorStatus, onSyncProgress } from './services/events';
import { BackupChallenge } from './pages/BackupChallenge';
import { BackupFlow } from './pages/BackupFlow';
import { CreateWallet } from './pages/CreateWallet';
import { OnboardingBackup } from './pages/OnboardingBackup';
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
import { KeystoneSetup } from './pages/KeystoneSetup';
import { Signing } from './pages/Signing';
import { Swap } from './pages/Swap';
import { SwapDeposit } from './pages/SwapDeposit';
import { SwapFromZec } from './pages/SwapFromZec';
import { SwapQuote } from './pages/SwapQuote';
import { ServerSettings } from './pages/ServerSettings';
import { Wallets } from './pages/Wallets';

const queryClient = new QueryClient();

type StartupState =
  | { kind: 'loading' }
  | { kind: 'no-wallets' }
  | { kind: 'wallet-selection' }
  | { kind: 'locked'; wallet: IPC.WalletInfo }
  | { kind: 'ready'; wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }
  | { kind: 'error'; error: IPC.IpcError };

function AppInner() {
  const navigate = useNavigate();
  const [startup, setStartup] = useState<StartupState>({ kind: 'loading' });
  const [accounts, setAccounts] = useState<IPC.AccountInfo[]>([]);
  const [seedPhrase, setSeedPhrase] = useState<string[] | null>(null);
  const [restoreFlow, setRestoreFlow] = useState<RestoreFlowData | null>(null);
  const [torState, setTorState] = useState<IPC.TorState | null>(null);
  const [torToggleError, setTorToggleError] = useState<IPC.IpcError | null>(null);
  const [syncProgress, setSyncProgress] = useState<IPC.SyncProgress | null>(null);
  const [dismissedTorError, setDismissedTorError] = useState(false);

  const activeWalletId = useMemo(() => {
    if (startup.kind === 'locked' || startup.kind === 'ready') return startup.wallet.id;
    return null;
  }, [startup]);

  const { activeAccountId, setActiveAccountId: _setActiveAccountId } = useActiveAccount(activeWalletId, accounts);
  const activeAccount = useMemo(() => {
    if (activeAccountId == null) return null;
    return accounts.find((a) => a.id === activeAccountId) ?? null;
  }, [accounts, activeAccountId]);

  // Tor state initialization and subscription
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

  // Sync progress initialization and subscription
  useEffect(() => {
    let cancelled = false;

    async function initSync() {
      if (startup.kind !== 'ready') return;
      const res = await getSyncProgress({ wallet_id: startup.wallet.id });
      if (!cancelled && 'ok' in res) {
        setSyncProgress(res.ok.progress);
      }
    }

    initSync();
    return () => {
      cancelled = true;
    };
  }, [startup]);

  // Throttled sync progress updates
  const throttledSetSync = useThrottledCallback(
    (progress: IPC.SyncProgress) => setSyncProgress(progress),
    200
  );

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    onSyncProgress((evt) => throttledSetSync(evt.progress))
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, [throttledSetSync]);

  useEffect(() => {
    if (torState?.enabled && torState.status === 'Error') {
      setDismissedTorError(false);
    }
  }, [torState?.enabled, torState?.status]);

  const toggleTor = async (enabled: boolean) => {
    const res = await setTorEnabled({ enabled });
    if ('ok' in res) {
      setTorState(res.ok.state);
    } else {
      setTorToggleError(res.err);
    }
  };

  const handleQuickLock = async () => {
    if (startup.kind !== 'ready') return;
    const res = await lockWallet({ wallet_id: startup.wallet.id });
    if ('ok' in res && res.ok.locked) {
      setStartup({ kind: 'locked', wallet: startup.wallet });
      setAccounts([]);
      setSyncProgress(null);
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

      setStartup({ kind: 'wallet-selection' });
    }

    runStartup();
    return () => {
      cancelled = true;
    };
  }, []);

  // Loading state
  if (startup.kind === 'loading') {
    return (
      <div className="flex h-screen items-center justify-center">
        <Logo size={120} className="animate-pulse" />
      </div>
    );
  }

  // Error state
  if (startup.kind === 'error') {
    return (
      <ErrorDialog
        title="Request failed"
        error={{ code: startup.error.code, message: startup.error.message }}
        primaryAction={{ label: 'Reload app', onClick: () => window.location.reload() }}
      />
    );
  }

  // No wallets state
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
        <Route
          path="/keystone/setup"
          element={
            <KeystoneSetup
              onCreated={(args) => {
                setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
                setAccounts(args.accounts);
              }}
            />
          }
        />
        <Route
          path="/onboarding-backup"
          element={
            <OnboardingBackup
              seedPhrase={seedPhrase ?? []}
              onCleared={() => setSeedPhrase(null)}
            />
          }
        />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    );
  }

  // Locked state
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
        onBack={() => setStartup({ kind: 'wallet-selection' })}
      />
    );
  }

  // Wallet selection state
  if (startup.kind === 'wallet-selection') {
    return (
      <WalletSelectionRoutes
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
        onCreated={(args) => {
          setSeedPhrase(args.seedPhrase);
          setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
          setAccounts(args.accounts);
        }}
        onRestoreFlow={setRestoreFlow}
        restoreFlow={restoreFlow}
        onClearRestoreFlow={() => setRestoreFlow(null)}
        onRestored={(args) => {
          setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
          setAccounts(args.accounts);
        }}
      />
    );
  }

  // Ready state - Main app with sidebar
  return (
    <AppShell
      wallet={startup.wallet}
      torState={torState}
      syncProgress={syncProgress}
      onLock={handleQuickLock}
    >
      {/* Tor Error Dialog */}
      {torState && torState.enabled && torState.status === 'Error' && !dismissedTorError ? (
        <TorErrorDialog
          state={torState}
          onClose={() => setDismissedTorError(true)}
          onDisable={() => toggleTor(false)}
          onRetry={() => toggleTor(true)}
        />
      ) : null}

      {/* Tor Toggle Error Dialog */}
      {torToggleError ? (
        <ErrorDialog
          title="Tor toggle failed"
          error={{ code: torToggleError.code, message: torToggleError.message }}
          primaryAction={{ label: 'Dismiss', onClick: () => setTorToggleError(null) }}
        />
      ) : null}

      <Routes>
        <Route
          path="/"
          element={
            <Home
              wallet={startup.wallet}
              activeAccountId={activeAccountId}
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
        <Route path="/swap/from-zec" element={<SwapFromZec wallet={startup.wallet} activeAccountId={activeAccountId} />} />
        <Route path="/swap/quote" element={<SwapQuote />} />
        <Route path="/swap/deposit" element={<SwapDeposit />} />
        <Route
          path="/activity"
          element={<Activity walletId={startup.wallet.id} activeAccountId={activeAccountId} />}
        />
        <Route
          path="/settings"
          element={
            <Settings
              wallet={startup.wallet}
              torState={torState}
              onSetTorEnabled={toggleTor}
              onLogout={() => {
                setAccounts([]);
                setSyncProgress(null);
                setStartup({ kind: 'wallet-selection' });
                navigate('/wallets');
              }}
            />
          }
        />
        <Route path="/settings/servers" element={<ServerSettings wallet={startup.wallet} />} />
        <Route
          path="/keystone/import"
          element={
            <ImportKeystone
              walletId={startup.wallet.id}
              walletNetwork={startup.wallet.network}
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
        <Route
          path="/backup/flow"
          element={<BackupFlow walletId={startup.wallet.id} />}
        />
        <Route
          path="/onboarding-backup"
          element={
            <OnboardingBackup
              seedPhrase={seedPhrase ?? []}
              onCleared={() => setSeedPhrase(null)}
            />
          }
        />
      </Routes>
    </AppShell>
  );
}

function WalletSelectionRoutes(props: {
  onLoaded: (resp: IPC.LoadWalletResponse) => void;
  onCreated: (args: { seedPhrase: string[]; wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
  onRestoreFlow: (data: RestoreFlowData) => void;
  restoreFlow: RestoreFlowData | null;
  onClearRestoreFlow: () => void;
  onRestored: (args: { wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
}) {
  const { onLoaded, onCreated, onRestoreFlow, restoreFlow, onClearRestoreFlow, onRestored } = props;
  const navigate = useNavigate();

  const pickerElement = (
    <WalletPicker
      onLoaded={onLoaded}
      onCreateNew={() => navigate('/create')}
      onRestore={() => navigate('/restore')}
    />
  );

  return (
    <Routes>
      <Route path="/" element={pickerElement} />
      <Route path="/wallets" element={pickerElement} />
      <Route
        path="/create"
        element={<CreateWallet onCreated={onCreated} />}
      />
      <Route
        path="/restore"
        element={<RestoreWallet onContinue={onRestoreFlow} />}
      />
      <Route
        path="/restore/birthday"
        element={
          <RestoreBirthday
            flow={restoreFlow}
            onClearFlow={onClearRestoreFlow}
            onRestored={onRestored}
          />
        }
      />
      <Route
        path="/keystone/setup"
        element={<KeystoneSetup onCreated={onRestored} />}
      />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

function UnlockGate(props: {
  wallet: IPC.WalletInfo;
  onUnlocked: (state: Extract<StartupState, { kind: 'ready' | 'locked' }>) => void;
  onBack: () => void;
}) {
  const { wallet, onUnlocked, onBack } = props;
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
    <div className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-md animate-[scale-in_0.3s_ease-out]">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-none bg-primary/10">
            <Shield className="h-8 w-8 text-primary" />
          </div>
          <CardTitle className="font-display text-2xl">Unlock Wallet</CardTitle>
          <p className="text-sm text-muted-foreground">{wallet.name}</p>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="password">Password</Label>
            <Input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              placeholder="Enter your password"
              onKeyDown={(e) => {
                if (e.key === 'Enter' && password) {
                  submit();
                }
              }}
            />
          </div>
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={rememberUnlock}
              onChange={(e) => setRememberUnlock(e.currentTarget.checked)}
              className="rounded-none border-border h-4 w-4 accent-primary"
            />
            <span className="text-muted-foreground">Remember unlock</span>
          </label>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <div className="flex gap-3">
            <Button onClick={submit} disabled={!password} className="flex-1">
              Unlock
            </Button>
            <Button variant="outline" onClick={onBack}>
              Back
            </Button>
          </div>
        </CardContent>
      </Card>
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
