import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { MenuEvents } from '../constants/menuEvents';
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
  /** Callback when wallet is locked via menu. */
  onLocked?: () => void;
  /** Callback when user logs out via menu. */
  onLogout?: () => void;
  /** Callback to update Tor state after toggle. */
  onTorStateChanged?: (enabled: boolean) => void;
}

/**
 * Hook that listens for native menu events and dispatches actions.
 * Must be used within a Router context.
 */
export function useMenuEvents(options: UseMenuEventsOptions): void {
  const { walletId, onLocked, onLogout, onTorStateChanged } = options;
  const navigate = useNavigate();

  // Use refs to avoid stale closures in event handlers
  const walletIdRef = useRef(walletId);
  const onLockedRef = useRef(onLocked);
  const onLogoutRef = useRef(onLogout);
  const onTorStateChangedRef = useRef(onTorStateChanged);

  useEffect(() => {
    walletIdRef.current = walletId;
    onLockedRef.current = onLocked;
    onLogoutRef.current = onLogout;
    onTorStateChangedRef.current = onTorStateChanged;
  }, [walletId, onLocked, onLogout, onTorStateChanged]);

  useEffect(() => {
    let mounted = true;
    const unlisteners: UnlistenFn[] = [];

    function ensureWalletLoaded(): string | null {
      const id = walletIdRef.current;
      if (id) return id;
      navigate('/wallets');
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
            console.error(`Menu: handler failed for ${event}`, err);
          }
        });
        if (mounted) {
          unlisteners.push(unlisten);
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

      // Navigation events (no wallet required)
      listeners.push(addListener(MenuEvents.NEW_WALLET, () => navigate('/create')));
      listeners.push(addListener(MenuEvents.RESTORE_WALLET, () => navigate('/restore')));
      listeners.push(addListener(MenuEvents.SWITCH_WALLET, () => navigate('/wallets')));

      // Navigation events (wallet required)
      listeners.push(
        addListener(MenuEvents.PREFERENCES, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/settings');
        })
      );
      listeners.push(
        addListener(MenuEvents.SEND, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/send');
        })
      );
      listeners.push(
        addListener(MenuEvents.RECEIVE, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/receive');
        })
      );
      listeners.push(
        addListener(MenuEvents.SWAP, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/swap');
        })
      );
      listeners.push(
        addListener(MenuEvents.ACTIVITY, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/activity');
        })
      );
      listeners.push(
        addListener(MenuEvents.VIEW_SEED, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/backup/flow');
        })
      );
      listeners.push(
        addListener(MenuEvents.VERIFY_BACKUP, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/backup');
        })
      );
      listeners.push(
        addListener(MenuEvents.HARDWARE_WALLET, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/keystone/import');
        })
      );
      listeners.push(
        addListener(MenuEvents.SERVER_SETTINGS, () => {
          if (!ensureWalletLoaded()) return;
          navigate('/settings/servers');
        })
      );

      // Lock wallet
      listeners.push(
        addListener(MenuEvents.LOCK_WALLET, async () => {
          const id = ensureWalletLoaded();
          if (!id) return;
          const res = await lockWallet({ wallet_id: id });
          if ('ok' in res && res.ok.locked) {
            onLockedRef.current?.();
          } else if ('err' in res) {
            console.error('Menu: failed to lock wallet', res.err);
          }
        })
      );

      // Logout
      listeners.push(
        addListener(MenuEvents.LOGOUT, async () => {
          const id = ensureWalletLoaded();
          if (!id) return;
          // Stop sync first to satisfy engine contract
          const stopRes = await stopSync({ wallet_id: id });
          if ('err' in stopRes) {
            console.error('Menu: failed to stop sync before logout', stopRes.err);
          }
          const res = await logoutWallet({ wallet_id: id });
          if ('ok' in res) {
            onLogoutRef.current?.();
          } else if ('err' in res) {
            console.error('Menu: failed to logout wallet', res.err);
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
            console.error('Menu: failed to start sync', res.err);
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
            console.error('Menu: failed to stop sync', res.err);
          }
        })
      );

      // Toggle Tor
      listeners.push(
        addListener(MenuEvents.TOGGLE_TOR, async () => {
          const stateRes = await getTorState();
          if ('ok' in stateRes) {
            const currentEnabled = stateRes.ok.state.enabled;
            const res = await setTorEnabled({ enabled: !currentEnabled });
            if ('ok' in res) {
              onTorStateChangedRef.current?.(!currentEnabled);
            } else if ('err' in res) {
              console.error('Menu: failed to toggle Tor', res.err);
            }
          } else if ('err' in stateRes) {
            console.error('Menu: failed to get Tor state', stateRes.err);
          }
        })
      );

      // Open logs folder
      listeners.push(
        addListener(MenuEvents.OPEN_LOGS, async () => {
          const res = await getLogLocation();
          if ('ok' in res && res.ok.log_directory) {
            try {
              await revealItemInDir(res.ok.log_directory);
            } catch (err) {
              console.error('Menu: failed to reveal logs directory', err);
            }
          } else if ('err' in res) {
            console.error('Menu: failed to get log location', res.err);
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
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [navigate]);
}
