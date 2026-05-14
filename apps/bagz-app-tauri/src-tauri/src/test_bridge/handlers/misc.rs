//! Miscellaneous command handlers (balance, address, logs, version).

use zstash_core::ipc::v1::commands::address::{
    GetReceiveAddressRequest, GetReceiveAddressResponse,
};
use zstash_core::ipc::v1::commands::balance::{GetBalanceRequest, GetBalanceResponse};
use zstash_core::ipc::v1::commands::logs::{GetLogLocationRequest, GetLogLocationResponse};
use zstash_core::ipc::v1::commands::version::{GetVersionRequest, GetVersionResponse};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::map_anyhow;

pub fn get_balance_impl(
    state: &AppState,
    request: GetBalanceRequest,
) -> IpcResult<GetBalanceResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let balance = mgr.get_balance(request.account_id)?;
        Ok(GetBalanceResponse {
            schema_version: SCHEMA_VERSION,
            balance,
        })
    })
}

pub fn get_receive_address_impl(
    state: &AppState,
    request: GetReceiveAddressRequest,
) -> IpcResult<GetReceiveAddressResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let address = mgr.get_receive_address(request.account_id, request.address_type)?;
        Ok(GetReceiveAddressResponse {
            schema_version: SCHEMA_VERSION,
            address,
        })
    })
}

pub fn get_log_location_impl(
    state: &AppState,
    request: GetLogLocationRequest,
) -> IpcResult<GetLogLocationResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let guard = state.logging_guard.lock().expect("mutex poisoned");
        Ok(GetLogLocationResponse {
            schema_version: SCHEMA_VERSION,
            log_directory: guard.log_directory().display().to_string(),
            current_log_file: guard.current_log_file().display().to_string(),
        })
    })
}

pub fn get_version_impl(
    _state: &AppState,
    request: GetVersionRequest,
) -> IpcResult<GetVersionResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    IpcResult::ok(GetVersionResponse {
        schema_version: SCHEMA_VERSION,
        version_info: zstash_core::version::VersionInfo::current(),
    })
}
