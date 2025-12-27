import { invoke } from '@tauri-apps/api/core';
import * as IPC from '../types/ipc';

function versioned<T extends object>(request: T): T & IPC.VersionedPayload {
  return { ...request, schema_version: IPC.SCHEMA_VERSION };
}

export async function listWallets(): Promise<IPC.IpcResult<IPC.ListWalletsResponse>> {
  return invoke(IPC.Commands.LIST_WALLETS, { request: versioned({}) });
}

export async function loadWallet(
  request: Omit<IPC.LoadWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.LoadWalletResponse>> {
  return invoke(IPC.Commands.LOAD_WALLET, { request: versioned(request) });
}

export async function unlockWallet(
  request: Omit<IPC.UnlockWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.UnlockWalletResponse>> {
  return invoke(IPC.Commands.UNLOCK_WALLET, { request: versioned(request) });
}

export async function createWallet(
  request: Omit<IPC.CreateWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.CreateWalletResponse>> {
  return invoke(IPC.Commands.CREATE_WALLET, { request: versioned(request) });
}

export async function getWalletStatus(
  request: Omit<IPC.GetWalletStatusRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.GetWalletStatusResponse>> {
  return invoke(IPC.Commands.GET_WALLET_STATUS, { request: versioned(request) });
}

export async function lockWallet(
  request: Omit<IPC.LockWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.LockWalletResponse>> {
  return invoke(IPC.Commands.LOCK_WALLET, { request: versioned(request) });
}

export async function reauthWallet(
  request: Omit<IPC.ReauthWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.ReauthWalletResponse>> {
  return invoke(IPC.Commands.REAUTH_WALLET, { request: versioned(request) });
}

export async function viewSeedPhrase(
  request: Omit<IPC.ViewSeedPhraseRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.ViewSeedPhraseResponse>> {
  return invoke(IPC.Commands.VIEW_SEED_PHRASE, { request: versioned(request) });
}

export async function getBalance(
  request: Omit<IPC.GetBalanceRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.GetBalanceResponse>> {
  return invoke(IPC.Commands.GET_BALANCE, { request: versioned(request) });
}

export async function getReceiveAddress(
  request: Omit<IPC.GetReceiveAddressRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.GetReceiveAddressResponse>> {
  return invoke(IPC.Commands.GET_RECEIVE_ADDRESS, { request: versioned(request) });
}

export async function getBackupChallenge(
  request: Omit<IPC.GetBackupChallengeRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.GetBackupChallengeResponse>> {
  return invoke(IPC.Commands.GET_BACKUP_CHALLENGE, { request: versioned(request) });
}

export async function verifyBackup(
  request: Omit<IPC.VerifyBackupRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.VerifyBackupResponse>> {
  return invoke(IPC.Commands.VERIFY_BACKUP, { request: versioned(request) });
}

export async function startSync(
  request: Omit<IPC.StartSyncRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.StartSyncResponse>> {
  return invoke(IPC.Commands.START_SYNC, { request: versioned(request) });
}

export async function stopSync(
  request: Omit<IPC.StopSyncRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.StopSyncResponse>> {
  return invoke(IPC.Commands.STOP_SYNC, { request: versioned(request) });
}

export async function getSyncProgress(
  request: Omit<IPC.GetSyncProgressRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.GetSyncProgressResponse>> {
  return invoke(IPC.Commands.GET_SYNC_PROGRESS, { request: versioned(request) });
}

export async function listTransactions(
  request: Omit<IPC.ListTransactionsRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.ListTransactionsResponse>> {
  return invoke(IPC.Commands.LIST_TRANSACTIONS, { request: versioned(request) });
}

export async function prepareSend(
  request: Omit<IPC.PrepareSendRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.PrepareSendResponse>> {
  return invoke(IPC.Commands.PREPARE_SEND, { request: versioned(request) });
}

export async function confirmSend(
  request: Omit<IPC.ConfirmSendRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.ConfirmSendResponse>> {
  return invoke(IPC.Commands.CONFIRM_SEND, { request: versioned(request) });
}

export async function cancelSend(
  request: Omit<IPC.CancelSendRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.CancelSendResponse>> {
  return invoke(IPC.Commands.CANCEL_SEND, { request: versioned(request) });
}

export async function retryBroadcast(
  request: Omit<IPC.RetryBroadcastRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.RetryBroadcastResponse>> {
  return invoke(IPC.Commands.RETRY_BROADCAST, { request: versioned(request) });
}
