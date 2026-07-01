use tauri::State;

use zbag_core::ipc::v1::commands::address::{GetReceiveAddressRequest, GetReceiveAddressResponse};
use zbag_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zbag_get_receive_address")]
pub fn zbag_get_receive_address(
    state: State<'_, AppState>,
    request: GetReceiveAddressRequest,
) -> IpcResult<GetReceiveAddressResponse> {
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
