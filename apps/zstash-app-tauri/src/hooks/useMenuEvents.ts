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
    const unlisteners: UnlistenFn[] = [];

    async function setupListeners() {
      // Navigation events
      unlisteners.push(
        await listen(MenuEvents.NEW_WALLET, () => navigate('/create'))
      );
      unlisteners.push(
        await listen(MenuEvents.RESTORE_WALLET, () => navigate('/restore'))
      );
      unlisteners.push(
        await listen(MenuEvents.SWITCH_WALLET, () => navigate('/wallets'))
      );
      unlisteners.push(
        await listen(MenuEvents.SEND, () => navigate('/send'))
      );
      unlisteners.push(
        await listen(MenuEvents.RECEIVE, () => navigate('/receive'))
      );
      unlisteners.push(
        await listen(MenuEvents.SWAP, () => navigate('/swap'))
      );
      unlisteners.push(
        await listen(MenuEvents.ACTIVITY, () => navigate('/activity'))
      );
      unlisteners.push(
        await listen(MenuEvents.VIEW_SEED, () => navigate('/backup/flow'))
      );
      unlisteners.push(
        await listen(MenuEvents.VERIFY_BACKUP, () => navigate('/backup'))
      );
      unlisteners.push(
        await listen(MenuEvents.HARDWARE_WALLET, () => navigate('/keystone/import'))
      );
      unlisteners.push(
        await listen(MenuEvents.SERVER_SETTINGS, () => navigate('/settings/servers'))
      );
      unlisteners.push(
        await listen(MenuEvents.PREFERENCES, () => navigate('/settings'))
      );

      // Lock wallet
      unlisteners.push(
        await listen(MenuEvents.LOCK_WALLET, async () => {
          const id = walletIdRef.current;
          if (!id) return;
          const res = await lockWallet({ wallet_id: id });
          if ('ok' in res && res.ok.locked) {
            onLockedRef.current?.();
          }
        })
      );

      // Logout
      unlisteners.push(
        await listen(MenuEvents.LOGOUT, async () => {
          const id = walletIdRef.current;
          if (!id) return;
          const res = await logoutWallet({ wallet_id: id });
          if ('ok' in res) {
            onLogoutRef.current?.();
          }
        })
      );

      // Sync now
      unlisteners.push(
        await listen(MenuEvents.SYNC_NOW, async () => {
          const id = walletIdRef.current;
          if (!id) return;
          await startSync({ wallet_id: id });
        })
      );

      // Stop sync
      unlisteners.push(
        await listen(MenuEvents.STOP_SYNC, async () => {
          const id = walletIdRef.current;
          if (!id) return;
          await stopSync({ wallet_id: id });
        })
      );

      // Toggle Tor
      unlisteners.push(
        await listen(MenuEvents.TOGGLE_TOR, async () => {
          const stateRes = await getTorState();
          if ('ok' in stateRes) {
            const currentEnabled = stateRes.ok.state.enabled;
            const res = await setTorEnabled({ enabled: !currentEnabled });
            if ('ok' in res) {
              onTorStateChangedRef.current?.(!currentEnabled);
            }
          }
        })
      );

      // Open logs folder
      unlisteners.push(
        await listen(MenuEvents.OPEN_LOGS, async () => {
          const res = await getLogLocation();
          if ('ok' in res && res.ok.log_directory) {
            await revealItemInDir(res.ok.log_directory);
          }
        })
      );
    }

    setupListeners();

    return () => {
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [navigate]);
}
