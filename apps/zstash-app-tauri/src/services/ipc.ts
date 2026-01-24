import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import * as IPC from '../types/ipc';

// Test bridge URL for E2E testing (Playwright/Chrome MCP)
const TEST_BRIDGE_URL = 'http://127.0.0.1:19816';
const USE_TEST_BRIDGE = import.meta.env.VITE_TEST_BRIDGE === 'true';
// Timeout for test bridge requests (configurable for slow CI runners)
const DEFAULT_TEST_BRIDGE_TIMEOUT = 10000;
const TEST_BRIDGE_TIMEOUT = (() => {
  const parsed = parseInt(import.meta.env.VITE_TEST_BRIDGE_TIMEOUT ?? '', 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : DEFAULT_TEST_BRIDGE_TIMEOUT;
})();

/**
 * Invoke a Tauri command, using HTTP transport when VITE_TEST_BRIDGE is enabled.
 *
 * In test mode, commands are sent to the test bridge HTTP server instead of
 * Tauri's IPC. This enables E2E testing with Playwright and Chrome MCP against
 * the real Rust backend without requiring the Tauri webview.
 */
async function invoke<T>(cmd: string, args: { request: unknown }): Promise<T> {
  if (USE_TEST_BRIDGE) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), TEST_BRIDGE_TIMEOUT);

    try {
      const res = await fetch(`${TEST_BRIDGE_URL}/invoke/${cmd}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(args),
        signal: controller.signal,
      });
      if (!res.ok) {
        const text = await res.text();
        throw new Error(`Test bridge error: ${res.status} ${text}`);
      }
      return res.json() as Promise<T>;
    } finally {
      clearTimeout(timeout);
    }
  }
  return tauriInvoke<T>(cmd, args);
}

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

export async function logoutWallet(
  request: Omit<IPC.LogoutWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.LogoutWalletResponse>> {
  return invoke(IPC.Commands.LOGOUT_WALLET, { request: versioned(request) });
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

export async function restoreWallet(
  request: Omit<IPC.RestoreWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.RestoreWalletResponse>> {
  return invoke(IPC.Commands.RESTORE_WALLET, { request: versioned(request) });
}

export async function importUfvk(
  request: Omit<IPC.ImportUfvkRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.ImportUfvkResponse>> {
  return invoke(IPC.Commands.IMPORT_UFVK, { request: versioned(request) });
}

export async function buildSigningRequest(
  request: Omit<IPC.BuildSigningRequestRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.BuildSigningRequestResponse>> {
  return invoke(IPC.Commands.BUILD_SIGNING_REQUEST, { request: versioned(request) });
}

export async function finalizeSigning(
  request: Omit<IPC.FinalizeSigningRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.FinalizeSigningResponse>> {
  return invoke(IPC.Commands.FINALIZE_SIGNING, { request: versioned(request) });
}

export async function createKeystoneWallet(
  request: Omit<IPC.CreateKeystoneWalletRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.CreateKeystoneWalletResponse>> {
  return invoke(IPC.Commands.CREATE_KEYSTONE_WALLET, { request: versioned(request) });
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

export async function shieldFunds(
  request: Omit<IPC.ShieldFundsRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.ShieldFundsResponse>> {
  return invoke(IPC.Commands.SHIELD_FUNDS, { request: versioned(request) });
}

export async function requestSwapQuote(
  request: Omit<IPC.RequestSwapQuoteRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.RequestSwapQuoteResponse>> {
  return invoke(IPC.Commands.REQUEST_SWAP_QUOTE, { request: versioned(request) });
}

export async function startSwap(
  request: Omit<IPC.StartSwapRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.StartSwapResponse>> {
  return invoke(IPC.Commands.START_SWAP, { request: versioned(request) });
}

export async function getSwapStatus(
  request: Omit<IPC.GetSwapStatusRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.GetSwapStatusResponse>> {
  return invoke(IPC.Commands.GET_SWAP_STATUS, { request: versioned(request) });
}

export async function listSwaps(): Promise<IPC.IpcResult<IPC.ListSwapsResponse>> {
  return invoke(IPC.Commands.LIST_SWAPS, { request: versioned({}) });
}

export async function setTorEnabled(
  request: Omit<IPC.SetTorEnabledRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.SetTorEnabledResponse>> {
  return invoke(IPC.Commands.SET_TOR_ENABLED, { request: versioned(request) });
}

export async function getTorState(): Promise<IPC.IpcResult<IPC.GetTorStateResponse>> {
  return invoke(IPC.Commands.GET_TOR_STATE, { request: versioned({}) });
}

export async function addServer(
  request: Omit<IPC.AddServerRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.AddServerResponse>> {
  return invoke(IPC.Commands.ADD_SERVER, { request: versioned(request) });
}

export async function setDefaultServer(
  request: Omit<IPC.SetDefaultServerRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.SetDefaultServerResponse>> {
  return invoke(IPC.Commands.SET_DEFAULT_SERVER, { request: versioned(request) });
}

export async function testServer(
  request: Omit<IPC.TestServerRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.TestServerResponse>> {
  return invoke(IPC.Commands.TEST_SERVER, { request: versioned(request) });
}

export async function listServers(): Promise<IPC.IpcResult<IPC.ListServersResponse>> {
  return invoke(IPC.Commands.LIST_SERVERS, { request: versioned({}) });
}

export async function getLogLocation(): Promise<IPC.IpcResult<IPC.GetLogLocationResponse>> {
  return invoke(IPC.Commands.GET_LOG_LOCATION, { request: versioned({}) });
}

export async function getVersion(): Promise<IPC.IpcResult<IPC.GetVersionResponse>> {
  return invoke(IPC.Commands.GET_VERSION, { request: versioned({}) });
}

export async function getFiatSettings(): Promise<IPC.IpcResult<IPC.GetFiatSettingsResponse>> {
  return invoke(IPC.Commands.GET_FIAT_SETTINGS, { request: versioned({}) });
}

export async function setFiatSettings(
  request: Omit<IPC.SetFiatSettingsRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.SetFiatSettingsResponse>> {
  return invoke(IPC.Commands.SET_FIAT_SETTINGS, { request: versioned(request) });
}

export async function getExchangeRate(
  request: Omit<IPC.GetExchangeRateRequest, 'schema_version'>
): Promise<IPC.IpcResult<IPC.GetExchangeRateResponse>> {
  return invoke(IPC.Commands.GET_EXCHANGE_RATE, { request: versioned(request) });
}
