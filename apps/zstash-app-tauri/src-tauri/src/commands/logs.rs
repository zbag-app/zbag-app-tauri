use tauri::State;

use zstash_core::ipc::v1::commands::logs::{GetLogLocationRequest, GetLogLocationResponse};
use zstash_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::state::AppState;

use super::util::map_anyhow;

#[tauri::command(rename = "zstash_get_log_location")]
pub fn zstash_get_log_location(
    state: State<'_, AppState>,
    request: GetLogLocationRequest,
) -> IpcResult<GetLogLocationResponse> {
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
