//! Server management command handlers.

use zbag_core::ipc::v1::commands::server::{
    AddServerRequest, AddServerResponse, ListServersRequest, ListServersResponse,
    SetDefaultServerRequest, SetDefaultServerResponse, TestServerRequest, TestServerResponse,
};
use zbag_core::ipc::v1::common::IpcResult;

use crate::server_logic;
use crate::state::AppState;
use crate::test_bridge::helpers::{block_on, map_anyhow, server_probe_timeout};

pub fn add_server_impl(
    state: &AppState,
    request: AddServerRequest,
) -> IpcResult<AddServerResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        block_on(server_logic::add_server(
            state,
            request,
            server_probe_timeout(),
        ))
    })
}

pub fn set_default_server_impl(
    state: &AppState,
    request: SetDefaultServerRequest,
) -> IpcResult<SetDefaultServerResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| server_logic::set_default_server(state, request))
}

pub fn list_servers_impl(
    state: &AppState,
    request: ListServersRequest,
) -> IpcResult<ListServersResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| server_logic::list_servers(state))
}

pub fn test_server_impl(
    state: &AppState,
    request: TestServerRequest,
) -> IpcResult<TestServerResponse> {
    use zbag_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        block_on(server_logic::test_server(
            state,
            request,
            server_probe_timeout(),
        ))
    })
}
