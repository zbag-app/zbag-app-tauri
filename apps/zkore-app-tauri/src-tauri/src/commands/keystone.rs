use tauri::State;

use zkore_core::ipc::v1::commands::keystone::{ImportUfvkRequest, ImportUfvkResponse};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zkore_import_ufvk")]
pub fn zkore_import_ufvk(
    state: State<'_, AppState>,
    request: ImportUfvkRequest,
) -> IpcResult<ImportUfvkResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow((|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let account = mgr.import_ufvk(request.wallet_id, &request.ufvk, &request.name)?;
        Ok(ImportUfvkResponse {
            schema_version: SCHEMA_VERSION,
            account,
        })
    })())
}
