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

export function onSwapChanged(
  callback: (event: IPC.SwapChangedEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.SWAP, (event) => {
    callback(event.payload as IPC.SwapChangedEvent);
  });
}

export function onTorStatus(
  callback: (event: IPC.TorStatusEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.TOR, (event) => {
    callback(event.payload as IPC.TorStatusEvent);
  });
}

export function onWalletStatus(
  callback: (event: IPC.WalletStatusEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.WALLET_STATUS, (event) => {
    callback(event.payload as IPC.WalletStatusEvent);
  });
}

export function onServerFailover(
  callback: (event: IPC.ServerFailoverEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.SERVER_FAILOVER, (event) => {
    callback(event.payload as IPC.ServerFailoverEvent);
  });
}
