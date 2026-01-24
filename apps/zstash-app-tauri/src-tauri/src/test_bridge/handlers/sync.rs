//! Sync-related command handlers.

use zstash_core::ipc::v1::commands::sync::{
    GetSyncProgressRequest, GetSyncProgressResponse, StartSyncRequest, StartSyncResponse,
    StopSyncRequest, StopSyncResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn start_sync_impl(
    state: &AppState,
    request: StartSyncRequest,
) -> IpcResult<StartSyncResponse> {
    use std::path::PathBuf;
    use zstash_core::domain::{SyncPhase, SyncProgress, WalletLockStatus};
    use zstash_core::errors;
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (wallet, lock_status) = mgr.load_wallet(request.wallet_id)?;

        if lock_status != WalletLockStatus::Unlocked {
            return Err(zstash_engine::error::ipc_err(
                errors::WALLET_LOCKED,
                "wallet locked",
            ));
        }

        // Get wallet DB path
        let wallet_db_path =
            zstash_engine::db::wallet_meta::get_wallet(mgr.app_db().conn(), wallet.id)
                .map_err(|e| anyhow::anyhow!(e))?
                .map(|(_, dir)| PathBuf::from(dir).join("wallet.sqlite"))
                .ok_or_else(|| {
                    zstash_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not found")
                })?;

        let wallet_dek = mgr.unlocked_wallet_dek(wallet.id)?;
        let account_ids = mgr.list_wallet_db_account_ids(wallet.id)?;

        // In test bridge mode, we start sync without event handlers since we don't
        // have an AppHandle for emitting events. Progress can be polled via get_sync_progress.
        match state.sync_service.start_sync(
            mgr.app_db(),
            wallet.id,
            wallet.network,
            wallet_db_path,
            wallet_dek,
            account_ids,
            Some(std::sync::Arc::clone(&state.tor_manager)),
            None, // No progress handler
            None, // No balance handler
        ) {
            Ok(()) => {
                mgr.observe_sync_progress(
                    wallet.id,
                    SyncProgress {
                        phase: SyncPhase::Preparing,
                        scan_frontier_height: 0,
                        wallet_tip_height: 0,
                        progress_percent: 0,
                        eta_seconds: None,
                        retry_in_seconds: None,
                        error_message: None,
                    },
                );
                Ok(StartSyncResponse {
                    schema_version: SCHEMA_VERSION,
                    started: true,
                })
            }
            Err(err)
                if zstash_engine::error::find_engine_ipc_error(&err)
                    .is_some_and(|e| e.code == errors::SYNC_IN_PROGRESS) =>
            {
                Ok(StartSyncResponse {
                    schema_version: SCHEMA_VERSION,
                    started: false,
                })
            }
            Err(err) => Err(err),
        }
    })
}

pub fn stop_sync_impl(state: &AppState, request: StopSyncRequest) -> IpcResult<StopSyncResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.observe_sync_stop_requested(request.wallet_id);
    }

    map_anyhow(|| {
        state.sync_service.stop_sync(request.wallet_id, None)?;
        Ok(StopSyncResponse {
            schema_version: SCHEMA_VERSION,
            stopped: true,
        })
    })
}

pub fn get_sync_progress_impl(
    state: &AppState,
    request: GetSyncProgressRequest,
) -> IpcResult<GetSyncProgressResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        Ok(GetSyncProgressResponse {
            schema_version: SCHEMA_VERSION,
            progress: state.sync_service.get_progress(request.wallet_id),
        })
    })
}
