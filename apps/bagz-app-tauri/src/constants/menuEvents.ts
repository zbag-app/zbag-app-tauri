/**
 * Menu event channel names from the backend.
 *
 * Keep in sync with `apps/bagz-app-tauri/src-tauri/src/menu.rs`.
 */
export const MenuEvents = {
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

