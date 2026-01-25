//! Tor command handlers.

use zstash_core::errors;
use zstash_core::ipc::v1::commands::sync::StartSyncRequest;
use zstash_core::ipc::v1::commands::tor::{
    GetTorStateRequest, GetTorStateResponse, SetTorEnabledRequest, SetTorEnabledResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;
use crate::time_utils::system_time_to_unix_ms;

use super::sync::start_sync_impl;

pub fn set_tor_enabled_impl(
    state: &AppState,
    request: SetTorEnabledRequest,
) -> IpcResult<SetTorEnabledResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handle = tokio::runtime::Handle::try_current().ok();
    let _guard = handle.as_ref().map(|h| h.enter());

    map_anyhow(|| {
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
            .map_err(|e| {
                zstash_engine::error::ipc_err(errors::TOR_CONNECTION_FAILED, e.to_string())
            })?;

        let updated_at_ms = system_time_to_unix_ms(std::time::SystemTime::now()).map_err(|e| {
            zstash_engine::error::ipc_err(errors::INTERNAL_ERROR, format!("time error: {e}"))
        })?;

        {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            zstash_engine::db::tor_meta::upsert_tor_state(
                mgr.app_db().conn(),
                &next_state,
                updated_at_ms,
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        }

        if request.enabled && !running_wallets.is_empty() {
            for wallet_id in running_wallets {
                let _ = start_sync_impl(
                    state,
                    StartSyncRequest {
                        schema_version: SCHEMA_VERSION,
                        wallet_id,
                    },
                );
            }
        }

        Ok(SetTorEnabledResponse {
            schema_version: SCHEMA_VERSION,
            state: next_state,
        })
    })
}

pub fn get_tor_state_impl(
    state: &AppState,
    request: GetTorStateRequest,
) -> IpcResult<GetTorStateResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

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
