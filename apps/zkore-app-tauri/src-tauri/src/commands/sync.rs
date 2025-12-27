use std::sync::Arc;

use tauri::State;

use zkore_core::errors;
use zkore_core::ipc::v1::common::{ensure_schema_version, IpcResult, SCHEMA_VERSION};
use zkore_core::ipc::v1::commands::sync::{
    GetSyncProgressRequest, GetSyncProgressResponse, StartSyncRequest, StartSyncResponse,
    StopSyncRequest, StopSyncResponse,
};

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
            return Err(zkore_engine::error::ipc_err(errors::WALLET_LOCKED, "wallet locked"));
        }

        let handler = Arc::new(move |event| {
            let _ = events::emit_sync_progress(&app, event);
        });
        state
            .sync_service
            .start_sync(mgr.app_db(), wallet.id, wallet.network, Some(handler))?;

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
        state.sync_service.stop_sync(request.wallet_id, Some(handler))?;
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
