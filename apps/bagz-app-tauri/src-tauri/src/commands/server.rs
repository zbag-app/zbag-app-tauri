use std::time::Duration;

use tauri::State;

use zstash_core::ipc::v1::commands::server::{
    AddServerRequest, AddServerResponse, ListServersRequest, ListServersResponse,
    SetDefaultServerRequest, SetDefaultServerResponse, TestServerRequest, TestServerResponse,
};
use zstash_core::ipc::v1::common::{IpcResult, ensure_schema_version};

use crate::server_logic;
use crate::state::AppState;

use super::util::map_anyhow;

/// Timeout for server probe to avoid UI blocking when offline.
///
/// This is a UX guardrail: long enough for slow networks/Tor circuits, short enough to keep the
/// UI responsive when the endpoint is unreachable.
const SERVER_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

#[tauri::command(rename = "zstash_add_server")]
pub fn zstash_add_server(
    state: State<'_, AppState>,
    request: AddServerRequest,
) -> IpcResult<AddServerResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        tauri::async_runtime::block_on(server_logic::add_server(
            state.inner(),
            request,
            SERVER_PROBE_TIMEOUT,
        ))
    })
}

#[tauri::command(rename = "zstash_set_default_server")]
pub fn zstash_set_default_server(
    state: State<'_, AppState>,
    request: SetDefaultServerRequest,
) -> IpcResult<SetDefaultServerResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| server_logic::set_default_server(state.inner(), request))
}

#[tauri::command(rename = "zstash_list_servers")]
pub fn zstash_list_servers(
    state: State<'_, AppState>,
    request: ListServersRequest,
) -> IpcResult<ListServersResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| server_logic::list_servers(state.inner()))
}

#[tauri::command(rename = "zstash_test_server")]
pub fn zstash_test_server(
    state: State<'_, AppState>,
    request: TestServerRequest,
) -> IpcResult<TestServerResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        tauri::async_runtime::block_on(server_logic::test_server(
            state.inner(),
            request,
            SERVER_PROBE_TIMEOUT,
        ))
    })
}
