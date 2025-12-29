use std::sync::Arc;
use std::time::Instant;

use tauri::State;
use uuid::Uuid;

use zkore_core::domain::{Network, ServerInfo};
use zkore_core::errors;
use zkore_core::ipc::v1::commands::server::{
    AddServerRequest, AddServerResponse, ListServersRequest, ListServersResponse,
    SetDefaultServerRequest, SetDefaultServerResponse, TestServerRequest, TestServerResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zkore_engine::error::ipc_err;

use crate::state::AppState;

use super::util::map_anyhow;
use super::util::system_time_to_unix_ms;

fn parse_network(chain_name: &str) -> anyhow::Result<Network> {
    let name = chain_name.trim().to_lowercase();
    match name.as_str() {
        "main" | "mainnet" => Ok(Network::Mainnet),
        "test" | "testnet" => Ok(Network::Testnet),
        other => Err(ipc_err(
            errors::INVALID_REQUEST,
            format!("unsupported chain_name: {other}"),
        )),
    }
}

#[tauri::command(rename = "zkore_add_server")]
pub fn zkore_add_server(
    state: State<'_, AppState>,
    request: AddServerRequest,
) -> IpcResult<AddServerResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let name = request.name.trim();
        if name.is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "server name required"));
        }
        let grpc_url = request.grpc_url.trim();
        if grpc_url.is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "grpc_url required"));
        }

        let client = zkore_network::grpc_client::GrpcClient::new_with_tor(
            grpc_url.to_string(),
            Arc::clone(&state.tor_manager),
        );

        let started = Instant::now();
        let info = tauri::async_runtime::block_on(async { client.probe_server().await })
            .map_err(|e| ipc_err(errors::SERVER_UNAVAILABLE, format!("server probe failed: {e}")))?;
        let _latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        let network = parse_network(&info.chain_name)?;

        let now_ms = system_time_to_unix_ms(std::time::SystemTime::now())?;
        let server = ServerInfo {
            id: Uuid::new_v4(),
            name: name.to_string(),
            grpc_url: grpc_url.to_string(),
            network,
            is_default: false,
            last_success_at: Some(now_ms),
        };

        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        zkore_engine::db::server_meta::insert_server(mgr.app_db().conn(), &server, now_ms)
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(AddServerResponse {
            schema_version: SCHEMA_VERSION,
            server,
        })
    })
}

#[tauri::command(rename = "zkore_set_default_server")]
pub fn zkore_set_default_server(
    state: State<'_, AppState>,
    request: SetDefaultServerRequest,
) -> IpcResult<SetDefaultServerResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let server = zkore_engine::db::server_meta::get_server(mgr.app_db().conn(), request.server_id)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "server not found"))?;
        mgr.ensure_server_network_matches_active_wallet(server.network)?;

        zkore_engine::db::server_meta::set_default_server(
            mgr.app_db_mut().conn_mut(),
            request.server_id,
        )
        .map_err(|e| anyhow::anyhow!(e))?;

        Ok(SetDefaultServerResponse {
            schema_version: SCHEMA_VERSION,
            success: true,
        })
    })
}

#[tauri::command(rename = "zkore_list_servers")]
pub fn zkore_list_servers(
    state: State<'_, AppState>,
    request: ListServersRequest,
) -> IpcResult<ListServersResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let servers =
            zkore_engine::db::server_meta::list_servers(mgr.app_db().conn()).map_err(|e| {
                anyhow::anyhow!(e)
            })?;

        Ok(ListServersResponse {
            schema_version: SCHEMA_VERSION,
            servers,
        })
    })
}

#[tauri::command(rename = "zkore_test_server")]
pub fn zkore_test_server(
    state: State<'_, AppState>,
    request: TestServerRequest,
) -> IpcResult<TestServerResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let server = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            zkore_engine::db::server_meta::get_server(mgr.app_db().conn(), request.server_id)
                .map_err(|e| anyhow::anyhow!(e))?
                .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "server not found"))?
        };

        let client = zkore_network::grpc_client::GrpcClient::new_with_tor(
            server.grpc_url.clone(),
            Arc::clone(&state.tor_manager),
        );

        let started = Instant::now();
        let probe = tauri::async_runtime::block_on(async { client.probe_server().await });
        let latency_ms = u64::try_from(started.elapsed().as_millis()).ok();

        match probe {
            Ok(info) => {
                let network = parse_network(&info.chain_name)?;
                if network != server.network {
                    return Ok(TestServerResponse {
                        schema_version: SCHEMA_VERSION,
                        success: false,
                        latency_ms,
                        error: Some("server network mismatch".to_string()),
                    });
                }

                let now_ms = system_time_to_unix_ms(std::time::SystemTime::now())?;
                let mgr = state.wallet_manager.lock().expect("mutex poisoned");
                let _ = zkore_engine::db::server_meta::update_last_success_at(
                    mgr.app_db().conn(),
                    server.id,
                    now_ms,
                );

                Ok(TestServerResponse {
                    schema_version: SCHEMA_VERSION,
                    success: true,
                    latency_ms,
                    error: None,
                })
            }
            Err(err) => Ok(TestServerResponse {
                schema_version: SCHEMA_VERSION,
                success: false,
                latency_ms,
                error: Some(err.to_string()),
            }),
        }
    })
}
