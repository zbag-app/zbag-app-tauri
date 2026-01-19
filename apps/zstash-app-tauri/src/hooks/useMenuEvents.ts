import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import {
  lockWallet,
  logoutWallet,
  startSync,
  stopSync,
  setTorEnabled,
  getTorState,
  getLogLocation,
} from '../services/ipc';

/** Menu event channel names from the backend. */
const MenuEvents = {
  NEW_WALLET: 'menu:new-wallet',
  RESTORE_WALLET: 'menu:restore-wallet',
  SWITCH_WALLET: 'menu:switch-wallet',
  LOCK_WALLET: 'menu:lock-wallet',
  LOGOUT: 'menu:logout',
  SEND: 'menu:send',
  RECEIVE: 'menu:receive',
  SWAP: 'menu:swap',
  ACTIVITY: 'menu:activity',
  SYNC_NOW: 'menu:sync-now',
  STOP_SYNC: 'menu:stop-sync',
  VIEW_SEED: 'menu:view-seed',
  VERIFY_BACKUP: 'menu:verify-backup',
  HARDWARE_WALLET: 'menu:hardware-wallet',
  TOGGLE_TOR: 'menu:toggle-tor',
  SERVER_SETTINGS: 'menu:server-settings',
  PREFERENCES: 'menu:preferences',
  OPEN_LOGS: 'menu:open-logs',
} as const;

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

    async function addListener(
      event: string,
      handler: () => void | Promise<void>
    ) {
      const unlisten = await listen(event, handler);
      if (mounted) {
        unlisteners.push(unlisten);
      } else {
        // Component unmounted before listener was set up - clean up immediately
        unlisten();
      }
    }

    async function setupListeners() {
      // Navigation events (no wallet required)
      await addListener(MenuEvents.NEW_WALLET, () => navigate('/create'));
      await addListener(MenuEvents.RESTORE_WALLET, () => navigate('/restore'));
      await addListener(MenuEvents.SWITCH_WALLET, () => navigate('/wallets'));
      await addListener(MenuEvents.PREFERENCES, () => navigate('/settings'));

      // Navigation events (wallet required)
      await addListener(MenuEvents.SEND, () => {
        if (walletIdRef.current) navigate('/send');
      });
      await addListener(MenuEvents.RECEIVE, () => {
        if (walletIdRef.current) navigate('/receive');
      });
      await addListener(MenuEvents.SWAP, () => {
        if (walletIdRef.current) navigate('/swap');
      });
      await addListener(MenuEvents.ACTIVITY, () => {
        if (walletIdRef.current) navigate('/activity');
      });
      await addListener(MenuEvents.VIEW_SEED, () => {
        if (walletIdRef.current) navigate('/backup/flow');
      });
      await addListener(MenuEvents.VERIFY_BACKUP, () => {
        if (walletIdRef.current) navigate('/backup');
      });
      await addListener(MenuEvents.HARDWARE_WALLET, () =>
        navigate('/keystone/import')
      );
      await addListener(MenuEvents.SERVER_SETTINGS, () =>
        navigate('/settings/servers')
      );

      // Lock wallet
      await addListener(MenuEvents.LOCK_WALLET, async () => {
        const id = walletIdRef.current;
        if (!id) return;
        const res = await lockWallet({ wallet_id: id });
        if ('ok' in res && res.ok.locked) {
          onLockedRef.current?.();
        } else if ('err' in res) {
          console.error('Menu: failed to lock wallet', res.err);
        }
      });

      // Logout
      await addListener(MenuEvents.LOGOUT, async () => {
        const id = walletIdRef.current;
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
      });

      // Sync now
      await addListener(MenuEvents.SYNC_NOW, async () => {
        const id = walletIdRef.current;
        if (!id) return;
        const res = await startSync({ wallet_id: id });
        if ('err' in res) {
          console.error('Menu: failed to start sync', res.err);
        }
      });

      // Stop sync
      await addListener(MenuEvents.STOP_SYNC, async () => {
        const id = walletIdRef.current;
        if (!id) return;
        const res = await stopSync({ wallet_id: id });
        if ('err' in res) {
          console.error('Menu: failed to stop sync', res.err);
        }
      });

      // Toggle Tor
      await addListener(MenuEvents.TOGGLE_TOR, async () => {
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
      });

      // Open logs folder
      await addListener(MenuEvents.OPEN_LOGS, async () => {
        const res = await getLogLocation();
        if ('ok' in res && res.ok.log_directory) {
          await revealItemInDir(res.ok.log_directory);
        } else if ('err' in res) {
          console.error('Menu: failed to get log location', res.err);
        }
      });
    }

    setupListeners();

    return () => {
      mounted = false;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [navigate]);
}
