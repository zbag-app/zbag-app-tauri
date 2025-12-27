import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import * as IPC from '../types/ipc';

export function onBalanceChanged(
  callback: (event: IPC.BalanceChangedEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.BALANCE, (event) => {
    callback(event.payload as IPC.BalanceChangedEvent);
  });
}

export function onSyncProgress(
  callback: (event: IPC.SyncProgressEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.SYNC, (event) => {
    callback(event.payload as IPC.SyncProgressEvent);
  });
}

export function onTransactionChanged(
  callback: (event: IPC.TransactionChangedEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.TRANSACTION, (event) => {
    callback(event.payload as IPC.TransactionChangedEvent);
  });
}

export function onWalletStatus(
  callback: (event: IPC.WalletStatusEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.WALLET_STATUS, (event) => {
    callback(event.payload as IPC.WalletStatusEvent);
  });
}
