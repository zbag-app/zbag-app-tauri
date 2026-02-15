import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { MenuEvents } from '../constants/menuEvents';
import { resolveLogRevealPath } from '../lib/logPaths';
import type * as IPC from '../types/ipc';
import {
  lockWallet,
  logoutWallet,
  startSync,
  stopSync,
  setTorEnabled,
  getTorState,
  getLogLocation,
} from '../services/ipc';

export interface UseMenuEventsOptions {
  /** Current wallet ID, if a wallet is loaded. */
  walletId: string | null;
  /** Whether the current wallet is unlocked/ready for wallet routes and actions. */
  walletUnlocked: boolean;
  /** Callback to enter wallet-selection UI when route-only navigation is insufficient (e.g. locked state). */
  onWalletSelectionRequested?: () => void;
  /** Callback when wallet is locked via menu. */
  onLocked?: () => void;
  /** Callback when user requests logout via menu. */
  onLogoutRequested?: (walletId: string) => void;
  /** Callback when user logs out via menu. */
  onLogout?: () => void;
  /** Callback to update Tor state after toggle. */
  onTorStateChanged?: (state: IPC.TorState) => void;
  /** Callback for user-visible error reporting. */
  onError?: (title: string, error: { code: string; message: string }) => void;
}

/**
 * Hook that listens for native menu events and dispatches actions.
 * Must be used within a Router context.
 */
export function useMenuEvents(options: UseMenuEventsOptions): void {
  const {
    walletId,
    walletUnlocked,
    onWalletSelectionRequested,
    onLocked,
    onLogoutRequested,
    onLogout,
    onTorStateChanged,
    onError,
  } = options;
  const navigate = useNavigate();

  // Use refs to avoid stale closures in event handlers
  const walletIdRef = useRef(walletId);
  const walletUnlockedRef = useRef(walletUnlocked);
  const onWalletSelectionRequestedRef = useRef(onWalletSelectionRequested);
  const onLockedRef = useRef(onLocked);
  const onLogoutRequestedRef = useRef(onLogoutRequested);
  const onLogoutRef = useRef(onLogout);
  const onTorStateChangedRef = useRef(onTorStateChanged);
  const onErrorRef = useRef(onError);
  const torToggleInFlightRef = useRef(false);

  useEffect(() => {
    walletIdRef.current = walletId;
    walletUnlockedRef.current = walletUnlocked;
    onWalletSelectionRequestedRef.current = onWalletSelectionRequested;
    onLockedRef.current = onLocked;
    onLogoutRequestedRef.current = onLogoutRequested;
    onLogoutRef.current = onLogout;
    onTorStateChangedRef.current = onTorStateChanged;
    onErrorRef.current = onError;
  }, [
    walletId,
    walletUnlocked,
    onWalletSelectionRequested,
    onLocked,
    onLogoutRequested,
    onLogout,
    onTorStateChanged,
    onError,
  ]);

  useEffect(() => {
    let mounted = true;
    const unlisteners = new Set<UnlistenFn>();

    function reportError(title: string, err: unknown) {
      console.error(`Menu: ${title}`, err);

      const error =
        typeof err === 'object' &&
        err != null &&
        'code' in err &&
        'message' in err &&
        typeof (err as { code: unknown }).code === 'string' &&
        typeof (err as { message: unknown }).message === 'string'
          ? { code: (err as { code: string }).code, message: (err as { message: string }).message }
          : {
              code: 'UNEXPECTED_ERROR',
              message: err instanceof Error ? err.message : String(err),
            };

      onErrorRef.current?.(title, error);
    }

    function ensureWalletLoaded(): string | null {
      const id = walletIdRef.current;
      if (id) return id;
      onErrorRef.current?.('Wallet required', {
        code: 'WALLET_REQUIRED',
        message: 'Create or select a wallet to use this menu item.',
      });
      navigate('/wallets');
      return null;
    }

    function ensureWalletUnlocked(): string | null {
      const id = ensureWalletLoaded();
      if (!id) return null;
      if (walletUnlockedRef.current) return id;
      onErrorRef.current?.('Wallet locked', {
        code: 'WALLET_LOCKED',
        message: 'Unlock your wallet to use this menu item.',
      });
      return null;
    }

    async function addListener(
      event: string,
      handler: () => void | Promise<void>
    ) {
      try {
        const unlisten = await listen(event, async () => {
          try {
            await handler();
          } catch (err) {
            reportError('Menu action failed', err);
          }
        });
        if (mounted) {
          unlisteners.add(unlisten);
        } else {
          // Component unmounted before listener was set up - clean up immediately
          unlisten();
        }
      } catch (err) {
        console.error(`Menu: failed to register listener for ${event}`, err);
      }
    }

    async function setupListeners() {
      const listeners: Array<Promise<void>> = [];

      function navigateViaWalletSelection(path: string) {
        onWalletSelectionRequestedRef.current?.();
        navigate(path);
      }

      // Navigation events (no wallet required)
      listeners.push(addListener(MenuEvents.NEW_WALLET, () => navigateViaWalletSelection('/create')));
      listeners.push(addListener(MenuEvents.RESTORE_WALLET, () => navigateViaWalletSelection('/restore')));
      listeners.push(addListener(MenuEvents.SWITCH_WALLET, () => navigateViaWalletSelection('/wallets')));

      // Navigation events (wallet required)
      listeners.push(
        addListener(MenuEvents.PREFERENCES, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/settings');
        })
      );
      listeners.push(
        addListener(MenuEvents.SEND, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/send');
        })
      );
      listeners.push(
        addListener(MenuEvents.RECEIVE, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/receive');
        })
      );
      listeners.push(
        addListener(MenuEvents.SWAP, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/swap');
        })
      );
      listeners.push(
        addListener(MenuEvents.ACTIVITY, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/activity');
        })
      );
      listeners.push(
        addListener(MenuEvents.VIEW_SEED, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/backup/flow');
        })
      );
      listeners.push(
        addListener(MenuEvents.VERIFY_BACKUP, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/backup');
        })
      );
      listeners.push(
        addListener(MenuEvents.HARDWARE_WALLET, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/keystone/import');
        })
      );
      listeners.push(
        addListener(MenuEvents.SERVER_SETTINGS, () => {
          if (!ensureWalletUnlocked()) return;
          navigate('/settings/servers');
        })
      );

      // Lock wallet
      listeners.push(
        addListener(MenuEvents.LOCK_WALLET, async () => {
          const id = ensureWalletLoaded();
          if (!id) return;
          if (!walletUnlockedRef.current) {
            onErrorRef.current?.('Wallet already locked', {
              code: 'WALLET_LOCKED',
              message: 'Your wallet is already locked.',
            });
            return;
          }
          const res = await lockWallet({ wallet_id: id });
          if ('ok' in res && res.ok.locked) {
            onLockedRef.current?.();
          } else if ('err' in res) {
            reportError('Lock wallet failed', res.err);
          }
        })
      );

      // Logout
      listeners.push(
        addListener(MenuEvents.LOGOUT, async () => {
          const id = ensureWalletLoaded();
          if (!id) return;
          if (onLogoutRequestedRef.current) {
            onLogoutRequestedRef.current(id);
            return;
          }
          // Stop sync first to satisfy engine contract
          try {
            const stopRes = await stopSync({ wallet_id: id });
            if ('err' in stopRes) {
              console.warn(
                'Menu: failed to stop sync before logout',
                stopRes.err.code,
                stopRes.err.message
              );
            }
          } catch (err) {
            console.warn('Menu: failed to stop sync before logout', err);
          }
          const res = await logoutWallet({ wallet_id: id });
          if ('ok' in res) {
            onLogoutRef.current?.();
          } else if ('err' in res) {
            reportError('Logout failed', res.err);
          }
        })
      );

      // Sync now
      listeners.push(
        addListener(MenuEvents.SYNC_NOW, async () => {
          const id = ensureWalletLoaded();
          if (!id) return;
          const res = await startSync({ wallet_id: id });
          if ('err' in res) {
            reportError('Start sync failed', res.err);
          }
        })
      );

      // Stop sync
      listeners.push(
        addListener(MenuEvents.STOP_SYNC, async () => {
          const id = ensureWalletLoaded();
          if (!id) return;
          const res = await stopSync({ wallet_id: id });
          if ('err' in res) {
            reportError('Stop sync failed', res.err);
          }
        })
      );

      // Toggle Tor
      listeners.push(
        addListener(MenuEvents.TOGGLE_TOR, async () => {
          if (torToggleInFlightRef.current) return;
          torToggleInFlightRef.current = true;
          try {
            const stateRes = await getTorState();
            if ('ok' in stateRes) {
              const currentEnabled = stateRes.ok.state.enabled;
              const res = await setTorEnabled({ enabled: !currentEnabled });
              if ('ok' in res) {
                onTorStateChangedRef.current?.(res.ok.state);
              } else if ('err' in res) {
                reportError('Toggle Tor failed', res.err);
              }
            } else if ('err' in stateRes) {
              reportError('Failed to read Tor state', stateRes.err);
            }
          } finally {
            torToggleInFlightRef.current = false;
          }
        })
      );

      // Open logs folder
      listeners.push(
        addListener(MenuEvents.OPEN_LOGS, async () => {
          const res = await getLogLocation();
          if ('ok' in res) {
            const revealPath = resolveLogRevealPath(res.ok);
            if (revealPath == null) {
              reportError('Open logs folder failed', {
                code: 'OPEN_LOGS_EMPTY_PATH',
                message: 'Backend returned an empty log path.',
              });
              return;
            }

            try {
              await revealItemInDir(revealPath);
            } catch (err) {
              reportError('Open logs folder failed', {
                code: 'OPEN_LOGS_FAILED',
                message: err instanceof Error ? err.message : String(err),
              });
            }
          } else if ('err' in res) {
            reportError('Open logs folder failed', res.err);
          }
        })
      );

      await Promise.all(listeners);
    }

    setupListeners().catch((err) => {
      console.error('Menu: failed to set up listeners', err);
    });

    return () => {
      mounted = false;
      const snapshot = Array.from(unlisteners);
      unlisteners.clear();
      for (const unlisten of snapshot) {
        unlisten();
      }
    };
  }, [navigate]);
}
