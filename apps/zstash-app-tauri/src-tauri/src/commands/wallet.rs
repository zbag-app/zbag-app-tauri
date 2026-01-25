use tauri::State;
use tracing::warn;

use zstash_core::domain::WalletLockStatus;
use zstash_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse,
    LockWalletRequest, LockWalletResponse, LogoutWalletRequest, LogoutWalletResponse,
    ReauthWalletRequest, ReauthWalletResponse, UnlockWalletRequest, UnlockWalletResponse,
    ViewSeedPhraseRequest, ViewSeedPhraseResponse,
};
use zstash_core::ipc::v1::common::{IpcResult, ensure_schema_version};

use crate::state::AppState;
use crate::wallet_logic;

use super::sync::start_sync_with_handlers;
use super::util::map_anyhow;

/// Timeout for birthday height fetch to avoid UI blocking when offline.
///
/// This is a UX guardrail: if the network is unreachable, wallet creation should still succeed by
/// falling back to a safe default scan start (Sapling activation).
const BIRTHDAY_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tauri::command(rename = "zstash_create_wallet")]
pub fn zstash_create_wallet(
    state: State<'_, AppState>,
    request: CreateWalletRequest,
) -> IpcResult<CreateWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    // Resolve gRPC URL and fetch chain tip for birthday height
    let (grpc_url, tor_manager) = {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let grpc_url =
            zstash_engine::server_resolver::resolve_grpc_url(mgr.app_db(), request.network);
        (grpc_url, Some(state.tor_manager.clone()))
    };

    // Fetch birthday height near chain tip for new wallet
    // This avoids scanning the entire blockchain for a brand new wallet
    let birthday_height = match grpc_url {
        Ok(url) => tauri::async_runtime::block_on(async {
            let fetch_future = zstash_engine::wallet_manager::fetch_birthday_height_for_new_wallet(
                &url,
                tor_manager,
            );
            match tokio::time::timeout(BIRTHDAY_FETCH_TIMEOUT, fetch_future).await {
                Ok(result) => result,
                Err(_) => {
                    warn!("birthday height fetch timed out, will use Sapling activation");
                    None
                }
            }
        }),
        Err(err) => {
            warn!(error = ?err, "failed to resolve gRPC URL for birthday fetch");
            None
        }
    };

    map_anyhow(|| wallet_logic::create_wallet(state.inner(), request, birthday_height))
}

#[tauri::command(rename = "zstash_list_wallets")]
pub fn zstash_list_wallets(
    state: State<'_, AppState>,
    request: ListWalletsRequest,
) -> IpcResult<ListWalletsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::list_wallets(state.inner()))
}

#[tauri::command(rename = "zstash_load_wallet")]
pub fn zstash_load_wallet(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: LoadWalletRequest,
) -> IpcResult<LoadWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let resp = wallet_logic::load_wallet(state.inner(), request.wallet_id)?;
        if resp.lock_status == WalletLockStatus::Unlocked {
            // Auto-start sync (best effort). LoadWallet should succeed even if sync can't start.
            let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
            if let Err(err) = start_sync_with_handlers(&app, &state, &mut mgr, &resp.wallet) {
                warn!(wallet_id = %resp.wallet.id, error = ?err, "auto-sync start failed");
            }
        }
        Ok(resp)
    })
}

#[tauri::command(rename = "zstash_unlock_wallet")]
pub fn zstash_unlock_wallet(
    state: State<'_, AppState>,
    request: UnlockWalletRequest,
) -> IpcResult<UnlockWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::unlock_wallet(state.inner(), request))
}

#[tauri::command(rename = "zstash_lock_wallet")]
pub fn zstash_lock_wallet(
    state: State<'_, AppState>,
    request: LockWalletRequest,
) -> IpcResult<LockWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::lock_wallet(state.inner(), request.wallet_id))
}

#[tauri::command(rename = "zstash_reauth_wallet")]
pub fn zstash_reauth_wallet(
    state: State<'_, AppState>,
    request: ReauthWalletRequest,
) -> IpcResult<ReauthWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::reauth_wallet(state.inner(), request))
}

#[tauri::command(rename = "zstash_view_seed_phrase")]
pub fn zstash_view_seed_phrase(
    state: State<'_, AppState>,
    request: ViewSeedPhraseRequest,
) -> IpcResult<ViewSeedPhraseResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::view_seed_phrase(state.inner(), request))
}

#[tauri::command(rename = "zstash_get_wallet_status")]
pub fn zstash_get_wallet_status(
    state: State<'_, AppState>,
    request: GetWalletStatusRequest,
) -> IpcResult<GetWalletStatusResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::get_wallet_status(state.inner(), request.wallet_id))
}

#[tauri::command(rename = "zstash_logout_wallet")]
pub fn zstash_logout_wallet(
    state: State<'_, AppState>,
    request: LogoutWalletRequest,
) -> IpcResult<LogoutWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| wallet_logic::logout_wallet(state.inner(), request.wallet_id))
}
