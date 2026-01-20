use std::sync::Arc;
use std::time::Instant;

use tauri::State;
use tracing::warn;
use uuid::Uuid;

use zstash_core::domain::{Network, ServerInfo};
use zstash_core::errors;
use zstash_core::ipc::v1::commands::server::{
    AddServerRequest, AddServerResponse, ListServersRequest, ListServersResponse,
    SetDefaultServerRequest, SetDefaultServerResponse, TestServerRequest, TestServerResponse,
};
use zstash_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zstash_engine::error::{find_engine_ipc_error, ipc_err};
use zstash_engine::grpc_url::validate_grpc_url;

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

#[tauri::command(rename = "zstash_add_server")]
pub fn zstash_add_server(
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
        validate_grpc_url(grpc_url)?;

        let client = zstash_network::grpc_client::GrpcClient::new_with_tor(
            grpc_url.to_string(),
            Arc::clone(&state.tor_manager),
        );

        let started = Instant::now();
        let info =
            tauri::async_runtime::block_on(async { client.probe_server().await }).map_err(|e| {
                ipc_err(
                    errors::SERVER_UNAVAILABLE,
                    format!("server probe failed: {e}"),
                )
            })?;
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
            validation_error: None,
        };

        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::server_meta::insert_server(mgr.app_db().conn(), &server, now_ms)
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(AddServerResponse {
            schema_version: SCHEMA_VERSION,
            server,
        })
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

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let server =
            zstash_engine::db::server_meta::get_server(mgr.app_db().conn(), request.server_id)
                .map_err(|e| anyhow::anyhow!(e))?
                .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "server not found"))?;
        // Defense-in-depth: stored values may be tampered with or come from legacy versions.
        validate_grpc_url(&server.grpc_url).map_err(|err| {
            warn!(
                server_id = %server.id,
                error = ?err,
                "stored server URL failed validation"
            );
            err
        })?;
        // `set_default_server` is state-changing: invalid stored configuration should fail the
        // command (propagating an IPC error) so the caller can prompt the user to fix it.
        mgr.ensure_server_network_matches_active_wallet(server.network)?;

        zstash_engine::db::server_meta::set_default_server(
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

#[tauri::command(rename = "zstash_list_servers")]
pub fn zstash_list_servers(
    state: State<'_, AppState>,
    request: ListServersRequest,
) -> IpcResult<ListServersResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let servers = zstash_engine::db::server_meta::list_servers(mgr.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?;
        // Validate each server's URL and populate validation_error so the frontend can display
        // invalid servers with a warning. Invalid servers are still rejected when used (set default,
        // test, resolve), but informing the frontend lets it show them greyed out with a warning.
        let servers = servers
            .into_iter()
            .map(|mut server| {
                if let Err(err) = validate_grpc_url(&server.grpc_url) {
                    let message = zstash_engine::error::find_engine_ipc_error(&err)
                        .map(|e| e.message.clone())
                        .unwrap_or_else(|| err.to_string());
                    server.validation_error = Some(message);
                }
                server
            })
            .collect();

        Ok(ListServersResponse {
            schema_version: SCHEMA_VERSION,
            servers,
        })
    })
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
        let server = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            zstash_engine::db::server_meta::get_server(mgr.app_db().conn(), request.server_id)
                .map_err(|e| anyhow::anyhow!(e))?
                .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "server not found"))?
        };

        // Defense-in-depth: re-validate stored URLs in case of DB tampering or legacy values.
        // `test_server` is a health-check: invalid stored configuration is reported as
        // `success: false` rather than failing the IPC command itself.
        if let Err(err) = validate_grpc_url(&server.grpc_url) {
            let message = find_engine_ipc_error(&err)
                .map(|engine| engine.message.clone())
                .unwrap_or_else(|| err.to_string());
            warn!(server_id = %server.id, error = %message, "stored server URL failed validation");
            return Ok(TestServerResponse {
                schema_version: SCHEMA_VERSION,
                success: false,
                latency_ms: None,
                error: Some(format!("stored server configuration is invalid: {message}")),
            });
        }

        let client = zstash_network::grpc_client::GrpcClient::new_with_tor(
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
                let _ = zstash_engine::db::server_meta::update_last_success_at(
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
