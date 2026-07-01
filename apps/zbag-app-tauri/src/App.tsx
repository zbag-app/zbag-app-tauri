import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { HashRouter, Navigate, Route, Routes, useNavigate } from 'react-router-dom';
import { Shield } from 'lucide-react';
import type * as IPC from './types/ipc';
import { ErrorCodes } from './types/ipc';
import { createCancellableSleep } from './lib/cancellableSleep';
import { Logo } from './components/brand/Logo';
import { ErrorBoundary } from './components/common/ErrorBoundary';
import { ErrorDialog } from './components/common/ErrorDialog';
import { LogoutDialogModal } from './components/common/LogoutDialogModal';
import { TorErrorDialog } from './components/common/TorErrorDialog';
import { WalletPicker } from './components/wallet/WalletPicker';
import { AppShell } from './components/layout/AppShell';
import { Button } from './components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from './components/ui/card';
import { Input } from './components/ui/input';
import { Label } from './components/ui/label';
import { useActiveAccount } from './hooks/useActiveAccount';
import { useMenuEvents } from './hooks/useMenuEvents';
import { useThrottledCallback } from './hooks/useThrottle';
import { getTorState, getSyncProgress, listWallets, loadWallet, lockWallet, resumePendingSwaps, setTorEnabled, unlockWallet } from './services/ipc';
import { onTorStatus, onSyncProgress } from './services/events';
import { BackupChallenge } from './pages/BackupChallenge';
import { BackupFlow } from './pages/BackupFlow';
import { CreateWallet } from './pages/CreateWallet';
import { OnboardingBackup } from './pages/OnboardingBackup';
import { Home } from './pages/Home';
import { Receive } from './pages/Receive';
import { Send } from './pages/Send';
import { SendConfirm } from './pages/SendConfirm';
import { Settings } from './pages/Settings';
import { Activity } from './pages/Activity';
import { RestoreBirthday } from './pages/RestoreBirthday';
import { RestoreWallet } from './pages/RestoreWallet';
import { ImportKeystone } from './pages/ImportKeystone';
import { KeystoneSetup } from './pages/KeystoneSetup';
import { Signing } from './pages/Signing';
import { CrossPay } from './pages/CrossPay';
import { Swap } from './pages/Swap';
import { SwapDeposit } from './pages/SwapDeposit';
import { SwapFromZec } from './pages/SwapFromZec';
import { SwapQuote } from './pages/SwapQuote';
import { ServerSettings } from './pages/ServerSettings';
import { Wallets } from './pages/Wallets';
import { FiatDisplayProvider } from './context/FiatDisplayContext';

const queryClient = new QueryClient();

const RESUME_PENDING_SWAPS_RETRY_DELAYS_MS = [0, 1000, 3000] as const;
const RESTRICTED_CONTEXT_MENU_WIDTH = 180;
const RESTRICTED_CONTEXT_MENU_HEIGHT = 132;
const RESTRICTED_CONTEXT_MENU_MARGIN = 8;

type StartupState =
  | { kind: 'loading' }
  | { kind: 'no-wallets' }
  | { kind: 'wallet-selection' }
  | { kind: 'locked'; wallet: IPC.WalletInfo }
  | { kind: 'ready'; wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }
  | { kind: 'error'; error: IPC.IpcError };

type RestrictedContextMenuState = {
  x: number;
  y: number;
};

function nearestEditableElement(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement)) return null;
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return target;
  return target.closest('[contenteditable="true"]');
}

function insertTextIntoEditable(editable: HTMLElement, text: string) {
  if (editable instanceof HTMLInputElement || editable instanceof HTMLTextAreaElement) {
    const start = editable.selectionStart ?? editable.value.length;
    const end = editable.selectionEnd ?? start;
    editable.setRangeText(text, start, end, 'end');
    editable.dispatchEvent(new Event('input', { bubbles: true }));
    return;
  }

  editable.focus();
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) {
    document.execCommand('insertText', false, text);
    return;
  }

  const range = selection.getRangeAt(0);
  range.deleteContents();
  range.insertNode(document.createTextNode(text));
  range.collapse(false);
  selection.removeAllRanges();
  selection.addRange(range);
  editable.dispatchEvent(new Event('input', { bubbles: true }));
}

function selectedTextFromEditable(editable: HTMLElement): string {
  if (editable instanceof HTMLInputElement || editable instanceof HTMLTextAreaElement) {
    const start = editable.selectionStart ?? 0;
    const end = editable.selectionEnd ?? start;
    if (start !== end) {
      return editable.value.slice(start, end);
    }
    return editable.value;
  }

  return window.getSelection()?.toString() ?? '';
}

function allTextFromEditable(editable: HTMLElement): string {
  if (editable instanceof HTMLInputElement || editable instanceof HTMLTextAreaElement) {
    return editable.value;
  }
  return editable.textContent ?? '';
}

function AppInner() {
  const navigate = useNavigate();
  const [startup, setStartup] = useState<StartupState>({ kind: 'loading' });
  const [accounts, setAccounts] = useState<IPC.AccountInfo[]>([]);
  const [seedPhrase, setSeedPhrase] = useState<string[] | null>(null);
  const [torState, setTorState] = useState<IPC.TorState | null>(null);
  const [torToggleError, setTorToggleError] = useState<IPC.IpcError | null>(null);
  const [resumePendingSwapsError, setResumePendingSwapsError] = useState<IPC.IpcError | null>(null);
  const [resumePendingSwapsRetryNonce, setResumePendingSwapsRetryNonce] = useState(0);
  const [menuError, setMenuError] = useState<{ title: string; error: IPC.IpcError } | null>(null);
  const [syncProgress, setSyncProgress] = useState<IPC.SyncProgress | null>(null);
  const [dismissedTorError, setDismissedTorError] = useState(false);
  const [menuLogoutWalletId, setMenuLogoutWalletId] = useState<string | null>(null);
  const [menuLogoutOpen, setMenuLogoutOpen] = useState(false);
  const [restrictedContextMenu, setRestrictedContextMenu] = useState<RestrictedContextMenuState | null>(null);
  const restrictedContextMenuRef = useRef<HTMLDivElement | null>(null);
  const contextMenuTargetRef = useRef<HTMLElement | null>(null);

  const activeWalletId = useMemo(() => {
    if (startup.kind === 'locked' || startup.kind === 'ready') return startup.wallet.id;
    return null;
  }, [startup]);
  const resumePendingSwapsWalletId = startup.kind === 'ready' ? startup.wallet.id : null;

  const { activeAccountId, setActiveAccountId: _setActiveAccountId } = useActiveAccount(activeWalletId, accounts);
  const activeAccount = useMemo(() => {
    if (activeAccountId == null) return null;
    return accounts.find((a) => a.id === activeAccountId) ?? null;
  }, [accounts, activeAccountId]);

  const closeMenuLogout = useCallback(() => {
    setMenuLogoutOpen(false);
    setMenuLogoutWalletId(null);
  }, []);

  const closeRestrictedContextMenu = useCallback(() => {
    setRestrictedContextMenu(null);
  }, []);

  const handleLogout = useCallback(() => {
    closeMenuLogout();
    setAccounts([]);
    setSyncProgress(null);
    setStartup({ kind: 'wallet-selection' });
    navigate('/wallets');
  }, [closeMenuLogout, navigate]);

  const handleWalletSelectionRequested = useCallback(() => {
    if (startup.kind === 'locked') {
      setStartup({ kind: 'wallet-selection' });
    }
  }, [startup]);

  // Menu events handler
  useMenuEvents({
    walletId: activeWalletId,
    walletUnlocked: startup.kind === 'ready',
    onWalletSelectionRequested: handleWalletSelectionRequested,
    onLocked: () => {
      if (startup.kind === 'ready') {
        setStartup({ kind: 'locked', wallet: startup.wallet });
        setAccounts([]);
        setSyncProgress(null);
      }
    },
    onLogoutRequested: (walletId) => {
      setMenuLogoutWalletId(walletId);
      setMenuLogoutOpen(true);
    },
    onLogout: handleLogout,
    onTorStateChanged: (state) => setTorState(state),
    onError: (title, error) => setMenuError({ title, error }),
  });

  const menuErrorDialog = menuError ? (
    <ErrorDialog
      title={menuError.title}
      error={{ code: menuError.error.code, message: menuError.error.message }}
      primaryAction={{ label: 'Dismiss', onClick: () => setMenuError(null) }}
    />
  ) : null;

  const menuLogoutDialog = (
    <LogoutDialogModal
      walletId={menuLogoutWalletId}
      open={menuLogoutOpen}
      onClose={closeMenuLogout}
      onLogout={handleLogout}
    />
  );

  const handleRestrictedContextCopy = useCallback(async () => {
    closeRestrictedContextMenu();
    const editable = contextMenuTargetRef.current ?? nearestEditableElement(document.activeElement);
    const text =
      (editable ? selectedTextFromEditable(editable) : window.getSelection()?.toString())?.trim() ??
      '';
    if (!text) return;

    try {
      await navigator.clipboard.writeText(text);
    } catch {
      document.execCommand('copy');
    }
  }, [closeRestrictedContextMenu]);

  const handleRestrictedContextPaste = useCallback(async () => {
    closeRestrictedContextMenu();
    const editable = contextMenuTargetRef.current ?? nearestEditableElement(document.activeElement);
    if (!editable) return;

    let text = '';
    try {
      text = await navigator.clipboard.readText();
    } catch {
      return;
    }
    if (!text) return;
    insertTextIntoEditable(editable, text);
  }, [closeRestrictedContextMenu]);

  const handleRestrictedContextCopyAll = useCallback(async () => {
    closeRestrictedContextMenu();
    const editable = contextMenuTargetRef.current ?? nearestEditableElement(document.activeElement);
    if (!editable) return;

    const text = allTextFromEditable(editable).trim();
    if (!text) return;

    try {
      await navigator.clipboard.writeText(text);
    } catch {
      if (editable instanceof HTMLInputElement || editable instanceof HTMLTextAreaElement) {
        editable.select();
      } else {
        const range = document.createRange();
        range.selectNodeContents(editable);
        const selection = window.getSelection();
        selection?.removeAllRanges();
        selection?.addRange(range);
      }
      document.execCommand('copy');
    }
  }, [closeRestrictedContextMenu]);

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

  // CEF lock-down: block native context menu except app-controlled editable fields.
  useEffect(() => {
    const onContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();

      const target = nearestEditableElement(event.target);
      contextMenuTargetRef.current = target;

      if (!target) {
        closeRestrictedContextMenu();
        return;
      }
      target.focus();

      const x = Math.max(
        RESTRICTED_CONTEXT_MENU_MARGIN,
        Math.min(
          event.clientX,
          window.innerWidth - RESTRICTED_CONTEXT_MENU_WIDTH - RESTRICTED_CONTEXT_MENU_MARGIN,
        ),
      );
      const y = Math.max(
        RESTRICTED_CONTEXT_MENU_MARGIN,
        Math.min(
          event.clientY,
          window.innerHeight - RESTRICTED_CONTEXT_MENU_HEIGHT - RESTRICTED_CONTEXT_MENU_MARGIN,
        ),
      );
      setRestrictedContextMenu({ x, y });
    };

    const onMouseDown = (event: MouseEvent) => {
      if (
        restrictedContextMenuRef.current &&
        event.target instanceof Node &&
        restrictedContextMenuRef.current.contains(event.target)
      ) {
        return;
      }
      closeRestrictedContextMenu();
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        closeRestrictedContextMenu();
      }
    };

    const disableDrag = (event: Event) => {
      event.preventDefault();
      event.stopPropagation();
      if ('stopImmediatePropagation' in event) {
        event.stopImmediatePropagation();
      }
    };

    const hardenNoDragAttributes = (root: ParentNode) => {
      for (const node of root.querySelectorAll<HTMLElement>('a,img,[draggable]')) {
        node.setAttribute('draggable', 'false');
        node.style.setProperty('user-drag', 'none');
        node.style.setProperty('-webkit-user-drag', 'none');
      }
    };

    const hardenCredentialAutofill = (root: ParentNode) => {
      for (const form of root.querySelectorAll<HTMLFormElement>('form')) {
        form.setAttribute('autocomplete', 'off');
      }

      for (const input of root.querySelectorAll<HTMLInputElement>('input')) {
        const inputType = (input.getAttribute('type') ?? input.type ?? 'text').toLowerCase();
        input.setAttribute('autocorrect', 'off');
        input.setAttribute('autocapitalize', 'off');
        input.spellcheck = false;

        if (inputType === 'password') {
          input.setAttribute('autocomplete', 'new-password');
          input.setAttribute('data-lpignore', 'true');
          input.setAttribute('data-1p-ignore', 'true');
          input.setAttribute('data-form-type', 'other');
          if (!input.hasAttribute('name') || input.getAttribute('name') === 'password') {
            input.setAttribute('name', 'zbag-secret');
          }
          continue;
        }

        if (['text', 'search', 'email', 'tel', 'url'].includes(inputType)) {
          input.setAttribute('autocomplete', 'off');
        }
      }

      for (const textarea of root.querySelectorAll<HTMLTextAreaElement>('textarea')) {
        textarea.setAttribute('autocomplete', 'off');
        textarea.setAttribute('autocorrect', 'off');
        textarea.setAttribute('autocapitalize', 'off');
        textarea.spellcheck = false;
      }
    };

    document.addEventListener('contextmenu', onContextMenu, true);
    window.addEventListener('contextmenu', onContextMenu, true);
    document.addEventListener('mousedown', onMouseDown, true);
    document.addEventListener('keydown', onKeyDown, true);
    document.addEventListener('dragstart', disableDrag, true);
    document.addEventListener('drag', disableDrag, true);
    document.addEventListener('dragend', disableDrag, true);
    document.addEventListener('dragenter', disableDrag, true);
    document.addEventListener('dragleave', disableDrag, true);
    document.addEventListener('dragover', disableDrag, true);
    document.addEventListener('drop', disableDrag, true);
    hardenNoDragAttributes(document);
    hardenCredentialAutofill(document);

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        for (const node of mutation.addedNodes) {
          if (!(node instanceof HTMLElement)) continue;
          node.setAttribute('draggable', 'false');
          node.style.setProperty('user-drag', 'none');
          node.style.setProperty('-webkit-user-drag', 'none');
          hardenNoDragAttributes(node);
          hardenCredentialAutofill(node);
        }
      }
    });
    observer.observe(document.body, { childList: true, subtree: true });

    window.addEventListener('blur', closeRestrictedContextMenu);
    window.addEventListener('resize', closeRestrictedContextMenu);

    return () => {
      document.removeEventListener('contextmenu', onContextMenu, true);
      window.removeEventListener('contextmenu', onContextMenu, true);
      document.removeEventListener('mousedown', onMouseDown, true);
      document.removeEventListener('keydown', onKeyDown, true);
      document.removeEventListener('dragstart', disableDrag, true);
      document.removeEventListener('drag', disableDrag, true);
      document.removeEventListener('dragend', disableDrag, true);
      document.removeEventListener('dragenter', disableDrag, true);
      document.removeEventListener('dragleave', disableDrag, true);
      document.removeEventListener('dragover', disableDrag, true);
      document.removeEventListener('drop', disableDrag, true);
      observer.disconnect();
      window.removeEventListener('blur', closeRestrictedContextMenu);
      window.removeEventListener('resize', closeRestrictedContextMenu);
    };
  }, [closeRestrictedContextMenu]);

  // Resume pending swaps when wallet becomes ready
  useEffect(() => {
    if (resumePendingSwapsWalletId == null) return;
    setResumePendingSwapsError(null);

    // Resume polling for any in-progress swaps from previous sessions
    let cancelled = false;
    const { sleep, cancel: cancelSleep } = createCancellableSleep();

    async function run() {
      const delaysMs = RESUME_PENDING_SWAPS_RETRY_DELAYS_MS;
      for (let attempt = 0; attempt < delaysMs.length; attempt++) {
        if (cancelled) return;

        const delayMs = delaysMs[attempt];
        if (delayMs > 0) {
          await sleep(delayMs);
          if (cancelled) return;
        }

        try {
          const res = await resumePendingSwaps();
          if (cancelled) return;
          if ('err' in res) {
            console.warn('resume_pending_swaps attempt failed', {
              attempt: attempt + 1,
              maxAttempts: delaysMs.length,
              error: res.err,
            });
            if (attempt === delaysMs.length - 1) {
              console.warn('Failed to resume pending swaps:', res.err);
              setResumePendingSwapsError(res.err);
            }
            continue;
          }
          return;
        } catch (err) {
          if (cancelled) return;
          console.warn('resume_pending_swaps attempt threw', {
            attempt: attempt + 1,
            maxAttempts: delaysMs.length,
            error: err,
          });
          if (attempt === delaysMs.length - 1) {
            console.warn('Failed to resume pending swaps:', err);
            setResumePendingSwapsError({
              code: ErrorCodes.INTERNAL_ERROR,
              message: err instanceof Error ? err.message : String(err),
            });
          }
        }
      }
    }

    run().catch(() => {
      // Errors are handled inside run()
    });

    return () => {
      cancelled = true;
      cancelSleep();
    };
  }, [resumePendingSwapsWalletId, resumePendingSwapsRetryNonce]);

  // Throttled sync progress updates
  const throttledSetSync = useThrottledCallback(
    (progress: IPC.SyncProgress) => setSyncProgress(progress),
    200
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    onSyncProgress((evt) => throttledSetSync(evt.progress))
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
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

  let content: ReactNode;

  if (startup.kind === 'error') {
    content = (
      <ErrorDialog
        title="Request failed"
        error={{ code: startup.error.code, message: startup.error.message }}
        primaryAction={{ label: 'Reload app', onClick: () => window.location.reload() }}
      />
    );
  } else if (startup.kind === 'no-wallets') {
    content = (
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
        <Route path="/restore" element={<RestoreWallet />} />
        <Route
          path="/restore/birthday"
          element={
            <RestoreBirthday
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
  } else if (startup.kind === 'locked') {
    content = (
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
  } else if (startup.kind === 'wallet-selection') {
    content = (
      <WalletSelectionRoutes
        onLoaded={(resp) => {
          setSeedPhrase(null);
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
        onRestored={(args) => {
          setStartup({ kind: 'ready', wallet: args.wallet, accounts: args.accounts });
          setAccounts(args.accounts);
        }}
      />
    );
  } else {
    const torErrorState =
      torState != null && torState.enabled && torState.status === 'Error' && !dismissedTorError
        ? torState
        : null;
    const resumePendingSwapsDialogError =
      resumePendingSwapsError != null && torErrorState == null && torToggleError == null
        ? resumePendingSwapsError
        : null;

    // Ready state - Main app with sidebar
    content = (
      <FiatDisplayProvider>
      <AppShell
        wallet={startup.wallet}
        torState={torState}
        syncProgress={syncProgress}
        onLock={handleQuickLock}
      >
        {/* Tor Error Dialog */}
        {torErrorState ? (
          <TorErrorDialog
            state={torErrorState}
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

        {resumePendingSwapsDialogError ? (
          <ErrorDialog
            title="Failed to resume swaps"
            error={{
              code: resumePendingSwapsDialogError.code,
              message: resumePendingSwapsDialogError.message,
            }}
            primaryAction={{ label: 'Dismiss', onClick: () => setResumePendingSwapsError(null) }}
            secondaryAction={{
              label: 'Retry',
              onClick: () => {
                setResumePendingSwapsError(null);
                setResumePendingSwapsRetryNonce((n) => n + 1);
              },
            }}
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
          <Route path="/restore" element={<RestoreWallet />} />
          <Route
            path="/restore/birthday"
            element={
              <RestoreBirthday
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
          <Route path="/swap/crosspay" element={<CrossPay wallet={startup.wallet} activeAccountId={activeAccountId} />} />
          <Route path="/swap/quote" element={<SwapQuote />} />
          <Route path="/swap/deposit" element={<SwapDeposit />} />
          <Route path="/swap/deposit/:swapId" element={<SwapDeposit />} />
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
                onLogout={handleLogout}
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
            path="/backup"
            element={<BackupChallenge walletId={startup.wallet.id} onVerified={() => setSeedPhrase(null)} />}
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
      </FiatDisplayProvider>
    );
  }

  return (
    <>
      {menuErrorDialog}
      {menuLogoutDialog}
      {content}
      {restrictedContextMenu ? (
        <div
          ref={restrictedContextMenuRef}
          className="fixed z-[9999] min-w-[180px] rounded-none border border-border bg-card p-1 shadow-lg"
          style={{ left: restrictedContextMenu.x, top: restrictedContextMenu.y }}
        >
          <button
            type="button"
            className="w-full px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
            onClick={() => void handleRestrictedContextCopy()}
          >
            Copy
          </button>
          <button
            type="button"
            className="w-full px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
            onClick={() => void handleRestrictedContextPaste()}
          >
            Paste
          </button>
          <button
            type="button"
            className="w-full px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
            onClick={() => void handleRestrictedContextCopyAll()}
          >
            Copy All
          </button>
        </div>
      ) : null}
    </>
  );
}

function WalletSelectionRoutes(props: {
  onLoaded: (resp: IPC.LoadWalletResponse) => void;
  onCreated: (args: { seedPhrase: string[]; wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
  onRestored: (args: { wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
}) {
  const { onLoaded, onCreated, onRestored } = props;
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
      <Route path="/restore" element={<RestoreWallet />} />
      <Route
        path="/restore/birthday"
        element={<RestoreBirthday onRestored={onRestored} />}
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
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setError(null);
    const unlockRes = await unlockWallet({
      wallet_id: wallet.id,
      password,
      remember_unlock: false,
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
