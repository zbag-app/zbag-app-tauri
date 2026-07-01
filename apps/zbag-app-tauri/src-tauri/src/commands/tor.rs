use tauri::{Runtime, State};

use zbag_core::domain::WalletLockStatus;
use zbag_core::errors;
use zbag_core::ipc::v1::commands::tor::{
    GetTorStateRequest, GetTorStateResponse, SetTorEnabledRequest, SetTorEnabledResponse,
};
use zbag_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zbag_engine::db::tor_meta;
use zbag_engine::error::ipc_err;

use super::sync::start_sync_with_handlers;
use crate::state::AppState;

use super::util::{map_anyhow, system_time_to_unix_ms};

#[tauri::command(rename = "zbag_set_tor_enabled")]
pub fn zbag_set_tor_enabled<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    request: SetTorEnabledRequest,
) -> IpcResult<SetTorEnabledResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    // Enter the tokio runtime context so TorManager can spawn tasks
    let tauri::async_runtime::RuntimeHandle::Tokio(handle) = tauri::async_runtime::handle();
    let _guard = handle.enter();

    map_anyhow(|| {
        // Stop any running syncs to kill cached direct channels
        let running_wallets = if request.enabled {
            let wallets = state.sync_service.running_wallet_ids();
            for wallet_id in &wallets {
                let _ = state.sync_service.stop_sync(*wallet_id, None);
            }
            wallets
        } else {
            Vec::new()
        };

        let next_state = state
            .tor_manager
            .set_enabled(request.enabled)
            .map_err(|e| ipc_err(errors::TOR_CONNECTION_FAILED, e.to_string()))?;

        let updated_at_ms = system_time_to_unix_ms(std::time::SystemTime::now())
            .map_err(|e| ipc_err(errors::INTERNAL_ERROR, format!("time error: {e}")))?;

        {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            tor_meta::upsert_tor_state(mgr.app_db().conn(), &next_state, updated_at_ms)
                .map_err(|e| anyhow::anyhow!(e))?;
        }

        // Restart syncs (they will wait for Tor in their own task)
        if request.enabled && !running_wallets.is_empty() {
            let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
            let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
            for wallet_id in running_wallets {
                if let Ok((wallet, lock_status)) = mgr.load_wallet(wallet_id, &mut tx_svc) {
                    if lock_status == WalletLockStatus::Unlocked {
                        let _ = start_sync_with_handlers(&app, &state, &mut mgr, &wallet);
                    }
                }
            }
        }

        Ok(SetTorEnabledResponse {
            schema_version: SCHEMA_VERSION,
            state: next_state,
        })
    })
}

#[tauri::command(rename = "zbag_get_tor_state")]
pub fn zbag_get_tor_state(
    state: State<'_, AppState>,
    request: GetTorStateRequest,
) -> IpcResult<GetTorStateResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        Ok(GetTorStateResponse {
            schema_version: SCHEMA_VERSION,
            state: state.tor_manager.state(),
        })
    })
}
