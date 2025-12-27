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

