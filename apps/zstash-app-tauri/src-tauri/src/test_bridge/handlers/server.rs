//! Server management command handlers.

use std::sync::Arc;
use std::time::Instant;

use tracing::warn;

use zstash_core::errors;
use zstash_core::ipc::v1::commands::server::{
    AddServerRequest, AddServerResponse, ListServersRequest, ListServersResponse,
    SetDefaultServerRequest, SetDefaultServerResponse, TestServerRequest, TestServerResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;
use crate::test_bridge::helpers::{
    map_anyhow, parse_network, probe_chain_name_with_timeout, system_time_to_unix_ms,
};

pub fn add_server_impl(
    state: &AppState,
    request: AddServerRequest,
) -> IpcResult<AddServerResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};
    use zstash_engine::error::ipc_err;

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
        zstash_engine::grpc_url::validate_grpc_url(grpc_url)?;

        let client = zstash_network::grpc_client::GrpcClient::new_with_tor(
            grpc_url.to_string(),
            Arc::clone(&state.tor_manager),
        );

        let chain_name = probe_chain_name_with_timeout(&client).map_err(|e| {
            ipc_err(
                errors::SERVER_UNAVAILABLE,
                format!("server probe failed: {e}"),
            )
        })?;

        let network = parse_network(&chain_name)?;

        let now_ms = system_time_to_unix_ms(std::time::SystemTime::now())?;
        let server = zstash_core::domain::ServerInfo {
            id: uuid::Uuid::new_v4(),
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

pub fn set_default_server_impl(
    state: &AppState,
    request: SetDefaultServerRequest,
) -> IpcResult<SetDefaultServerResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};
    use zstash_engine::error::ipc_err;
    use zstash_engine::grpc_url::validate_grpc_url;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let server =
            zstash_engine::db::server_meta::get_server(mgr.app_db().conn(), request.server_id)
                .map_err(|e| anyhow::anyhow!(e))?
                .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "server not found"))?;
        validate_grpc_url(&server.grpc_url).map_err(|err| {
            warn!(
                server_id = %server.id,
                error = ?err,
                "stored server URL failed validation"
            );
            err
        })?;
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

pub fn list_servers_impl(
    state: &AppState,
    request: ListServersRequest,
) -> IpcResult<ListServersResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};
    use zstash_engine::grpc_url::validate_grpc_url;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let servers = zstash_engine::db::server_meta::list_servers(mgr.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?;
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

pub fn test_server_impl(
    state: &AppState,
    request: TestServerRequest,
) -> IpcResult<TestServerResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};
    use zstash_engine::error::ipc_err;
    use zstash_engine::grpc_url::validate_grpc_url;

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

        if let Err(err) = validate_grpc_url(&server.grpc_url) {
            let message = zstash_engine::error::find_engine_ipc_error(&err)
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
        let probe = probe_chain_name_with_timeout(&client);
        let latency_ms = u64::try_from(started.elapsed().as_millis()).ok();

        match probe {
            Ok(chain_name) => {
                let network = parse_network(&chain_name)?;
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
