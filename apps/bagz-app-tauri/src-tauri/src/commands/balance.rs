use tauri::State;

use bagz_core::ipc::v1::commands::balance::{GetBalanceRequest, GetBalanceResponse};
use bagz_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "bagz_get_balance")]
pub fn bagz_get_balance(
    state: State<'_, AppState>,
    request: GetBalanceRequest,
) -> IpcResult<GetBalanceResponse> {
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
