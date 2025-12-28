use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;

use zkore_core::errors;
use zkore_core::ipc::v1::commands::sync::{
    GetSyncProgressRequest, GetSyncProgressResponse, StartSyncRequest, StartSyncResponse,
    StopSyncRequest, StopSyncResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::events;
use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zkore_start_sync")]
pub fn zkore_start_sync(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: StartSyncRequest,
) -> IpcResult<StartSyncResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (wallet, lock_status) = mgr.load_wallet(request.wallet_id)?;
        if lock_status != zkore_core::domain::WalletLockStatus::Unlocked {
            return Err(zkore_engine::error::ipc_err(
                errors::WALLET_LOCKED,
                "wallet locked",
            ));
        }

        let wallet_db_path =
            zkore_engine::db::wallet_meta::get_wallet(mgr.app_db().conn(), wallet.id)
                .map_err(|e| anyhow::anyhow!(e))?
                .map(|(_, dir)| PathBuf::from(dir).join("wallet.sqlite"))
                .ok_or_else(|| {
                    zkore_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not found")
                })?;

        let wallet_dek = mgr.unlocked_wallet_dek(wallet.id)?;
        let account_ids = mgr.list_wallet_db_account_ids(wallet.id)?;

        let progress_handler = {
            let app = app.clone();
            Arc::new(move |event| {
                let _ = events::emit_sync_progress(&app, event);
            })
        };

        let balance_handler = {
            let app = app.clone();
            Arc::new(move |event| {
                let _ = events::emit_balance_changed(&app, event);
            })
        };
        state.sync_service.start_sync(
            mgr.app_db(),
            wallet.id,
            wallet.network,
            wallet_db_path,
            wallet_dek,
            account_ids,
            Some(Arc::clone(&state.tor_manager)),
            Some(progress_handler),
            Some(balance_handler),
        )?;

        Ok(StartSyncResponse {
            schema_version: SCHEMA_VERSION,
            started: true,
        })
    })())
}

#[tauri::command(rename = "zkore_stop_sync")]
pub fn zkore_stop_sync(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: StopSyncRequest,
) -> IpcResult<StopSyncResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handler = Arc::new(move |event| {
        let _ = events::emit_sync_progress(&app, event);
    });

    map_anyhow((|| {
        state
            .sync_service
            .stop_sync(request.wallet_id, Some(handler))?;
        Ok(StopSyncResponse {
            schema_version: SCHEMA_VERSION,
            stopped: true,
        })
    })())
}

#[tauri::command(rename = "zkore_get_sync_progress")]
pub fn zkore_get_sync_progress(
    state: State<'_, AppState>,
    request: GetSyncProgressRequest,
) -> IpcResult<GetSyncProgressResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(Ok(GetSyncProgressResponse {
        schema_version: SCHEMA_VERSION,
        progress: state.sync_service.get_progress(request.wallet_id),
    }))
}
