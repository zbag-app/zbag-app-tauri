use tauri::State;

use zkore_core::errors;
use zkore_core::ipc::v1::commands::tor::{
    GetTorStateRequest, GetTorStateResponse, SetTorEnabledRequest, SetTorEnabledResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zkore_engine::db::tor_meta;
use zkore_engine::error::ipc_err;

use crate::state::AppState;

use super::util::{map_anyhow, system_time_to_unix_ms};

#[tauri::command(rename = "zkore_set_tor_enabled")]
pub fn zkore_set_tor_enabled(
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

        Ok(SetTorEnabledResponse {
            schema_version: SCHEMA_VERSION,
            state: next_state,
        })
    })
}

#[tauri::command(rename = "zkore_get_tor_state")]
pub fn zkore_get_tor_state(
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
