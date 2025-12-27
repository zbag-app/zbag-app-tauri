use std::collections::HashMap;

use anyhow::Context as _;
use tauri::State;
use tracing::warn;

use zkore_core::domain::{AccountInfo, AccountType, WalletLockStatus};
use zkore_core::ipc::v1::common::{ensure_schema_version, IpcResult, SCHEMA_VERSION};
use zkore_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse, LockWalletRequest,
    LockWalletResponse, ReauthWalletRequest, ReauthWalletResponse, UnlockWalletRequest,
    UnlockWalletResponse, ViewSeedPhraseRequest, ViewSeedPhraseResponse,
};

use crate::state::AppState;

use super::util::{map_anyhow, system_time_to_unix_ms};

#[tauri::command(rename = "zkore_create_wallet")]
pub fn zkore_create_wallet(
    state: State<'_, AppState>,
    request: CreateWalletRequest,
) -> IpcResult<CreateWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let created = mgr.create_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
        )?;

        Ok(CreateWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: created.wallet,
            seed_phrase: created.seed_phrase,
            backup_challenge: created.backup_challenge,
        })
    })())
}

#[tauri::command(rename = "zkore_list_wallets")]
pub fn zkore_list_wallets(
    state: State<'_, AppState>,
    request: ListWalletsRequest,
) -> IpcResult<ListWalletsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let wallets = mgr.list_wallets()?;
        Ok(ListWalletsResponse {
            schema_version: SCHEMA_VERSION,
            wallets,
        })
    })())
}

#[tauri::command(rename = "zkore_load_wallet")]
pub fn zkore_load_wallet(
    state: State<'_, AppState>,
    request: LoadWalletRequest,
) -> IpcResult<LoadWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (wallet, lock_status) = mgr.load_wallet(request.wallet_id)?;

        let accounts = if lock_status == WalletLockStatus::Locked {
            vec![]
        } else {
            load_accounts_for_wallet(&mut mgr, wallet.id)?
        };

        Ok(LoadWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet,
            lock_status,
            accounts,
        })
    })())
}

#[tauri::command(rename = "zkore_unlock_wallet")]
pub fn zkore_unlock_wallet(
    state: State<'_, AppState>,
    request: UnlockWalletRequest,
) -> IpcResult<UnlockWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.unlock_wallet(request.wallet_id, &request.password, request.remember_unlock)?;
        Ok(UnlockWalletResponse {
            schema_version: SCHEMA_VERSION,
            unlocked: status == WalletLockStatus::Unlocked,
        })
    })())
}

#[tauri::command(rename = "zkore_lock_wallet")]
pub fn zkore_lock_wallet(
    state: State<'_, AppState>,
    request: LockWalletRequest,
) -> IpcResult<LockWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.lock_wallet(request.wallet_id)?;
        Ok(LockWalletResponse {
            schema_version: SCHEMA_VERSION,
            locked: status == WalletLockStatus::Locked,
        })
    })())
}

#[tauri::command(rename = "zkore_reauth_wallet")]
pub fn zkore_reauth_wallet(
    state: State<'_, AppState>,
    request: ReauthWalletRequest,
) -> IpcResult<ReauthWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (token, expires_at) =
            mgr.reauth_wallet(request.wallet_id, &request.password, request.purpose)?;
        Ok(ReauthWalletResponse {
            schema_version: SCHEMA_VERSION,
            reauth_token: token,
            expires_at: system_time_to_unix_ms(expires_at)?,
        })
    })())
}

#[tauri::command(rename = "zkore_view_seed_phrase")]
pub fn zkore_view_seed_phrase(
    state: State<'_, AppState>,
    request: ViewSeedPhraseRequest,
) -> IpcResult<ViewSeedPhraseResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let seed_phrase = mgr.view_seed_phrase(request.wallet_id, &request.reauth_token)?;
        Ok(ViewSeedPhraseResponse {
            schema_version: SCHEMA_VERSION,
            seed_phrase,
        })
    })())
}

#[tauri::command(rename = "zkore_get_wallet_status")]
pub fn zkore_get_wallet_status(
    state: State<'_, AppState>,
    request: GetWalletStatusRequest,
) -> IpcResult<GetWalletStatusResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.compute_wallet_status(request.wallet_id)?;
        Ok(GetWalletStatusResponse {
            schema_version: SCHEMA_VERSION,
            status,
        })
    })())
}

fn load_accounts_for_wallet(
    mgr: &mut zkore_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<Vec<AccountInfo>> {
    let wallet_db_accounts = mgr.list_wallet_db_account_ids(wallet_id)?;
    let meta_accounts =
        zkore_engine::db::account_meta::list_accounts(mgr.app_db().conn(), wallet_id)
            .map_err(|e| anyhow::anyhow!(e))
            .context("failed to load account metadata")?;

    let meta_by_id: HashMap<u32, AccountInfo> =
        meta_accounts.into_iter().map(|a| (a.id, a)).collect();

    let mut out = Vec::with_capacity(wallet_db_accounts.len());
    for account_id in wallet_db_accounts {
        if let Some(meta) = meta_by_id.get(&account_id) {
            out.push(meta.clone());
            continue;
        }

        warn!(account_id, "Account metadata missing; applying defaults");
        out.push(AccountInfo {
            id: account_id,
            name: format!("Account {}", account_id + 1),
            account_type: if account_id == 0 {
                AccountType::Software
            } else {
                AccountType::HardwareSigner
            },
        });
    }

    Ok(out)
}
