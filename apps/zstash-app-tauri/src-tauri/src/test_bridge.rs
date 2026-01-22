//! HTTP Test Bridge for E2E Testing
//!
//! This module provides an HTTP server that exposes Tauri commands via REST endpoints,
//! enabling Playwright and Claude Code (via Chrome MCP) to test zstash against the real
//! Rust backend without requiring the Tauri webview.
//!
//! **Security:** This server is feature-gated (`test-bridge`) and only binds to
//! `127.0.0.1:19816`. It should NEVER be enabled in release builds.
//!
//! # Architecture
//!
//! ```text
//! Chrome Browser (localhost:1420)
//!   └── React Frontend (Vite dev server)
//!         └── VITE_TEST_BRIDGE=true
//!               └── HTTP fetch
//!                     └── Test Bridge Server (:19816)
//!                           └── AppState (real Rust backend)
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State as AxumState},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};

use zstash_core::ipc::v1::commands::address::{
    GetReceiveAddressRequest, GetReceiveAddressResponse,
};
use zstash_core::ipc::v1::commands::balance::{GetBalanceRequest, GetBalanceResponse};
use zstash_core::ipc::v1::commands::sync::{
    GetSyncProgressRequest, GetSyncProgressResponse, StartSyncRequest, StartSyncResponse,
    StopSyncRequest, StopSyncResponse,
};
use zstash_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse,
    LockWalletRequest, LockWalletResponse, LogoutWalletRequest, LogoutWalletResponse,
    UnlockWalletRequest, UnlockWalletResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;

/// Test bridge server port (localhost-only)
pub const TEST_BRIDGE_PORT: u16 = 19816;

/// Shared state for the test bridge server
pub struct TestBridgeState {
    pub app_state: Arc<AppState>,
}

/// Request body wrapper (matches Tauri's invoke format)
#[derive(Debug, Deserialize)]
struct InvokeBody {
    request: Value,
}

/// Start the test bridge HTTP server
///
/// This function spawns the HTTP server on a background task and returns immediately.
/// The server will run until the application exits.
pub async fn start_test_bridge(app_state: Arc<AppState>) -> anyhow::Result<()> {
    let bridge_state = Arc::new(TestBridgeState { app_state });

    // CORS layer to allow requests from Vite dev server (localhost:1420)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/invoke/:command", post(invoke_command))
        .layer(cors)
        .with_state(bridge_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], TEST_BRIDGE_PORT));
    info!("Test bridge server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("Test bridge server error: {}", e);
        }
    });

    info!("Test bridge server started on http://{}", addr);
    Ok(())
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": "1"
    }))
}

/// Main command dispatch endpoint
async fn invoke_command(
    AxumState(state): AxumState<Arc<TestBridgeState>>,
    Path(command): Path<String>,
    Json(body): Json<InvokeBody>,
) -> impl IntoResponse {
    info!(command = %command, "Test bridge: invoking command");

    match command.as_str() {
        // Wallet commands
        "zstash_list_wallets" => {
            dispatch::<ListWalletsRequest, ListWalletsResponse>(&state, body.request, |s, req| {
                list_wallets_impl(s, req)
            })
        }
        "zstash_create_wallet" => {
            dispatch::<CreateWalletRequest, CreateWalletResponse>(&state, body.request, |s, req| {
                create_wallet_impl(s, req)
            })
        }
        "zstash_load_wallet" => {
            dispatch::<LoadWalletRequest, LoadWalletResponse>(&state, body.request, |s, req| {
                load_wallet_impl(s, req)
            })
        }
        "zstash_get_wallet_status" => dispatch::<GetWalletStatusRequest, GetWalletStatusResponse>(
            &state,
            body.request,
            |s, req| get_wallet_status_impl(s, req),
        ),
        "zstash_unlock_wallet" => {
            dispatch::<UnlockWalletRequest, UnlockWalletResponse>(&state, body.request, |s, req| {
                unlock_wallet_impl(s, req)
            })
        }
        "zstash_lock_wallet" => {
            dispatch::<LockWalletRequest, LockWalletResponse>(&state, body.request, |s, req| {
                lock_wallet_impl(s, req)
            })
        }
        "zstash_logout_wallet" => {
            dispatch::<LogoutWalletRequest, LogoutWalletResponse>(&state, body.request, |s, req| {
                logout_wallet_impl(s, req)
            })
        }
        // Balance commands
        "zstash_get_balance" => {
            dispatch::<GetBalanceRequest, GetBalanceResponse>(&state, body.request, |s, req| {
                get_balance_impl(s, req)
            })
        }
        // Address commands
        "zstash_get_receive_address" => dispatch::<
            GetReceiveAddressRequest,
            GetReceiveAddressResponse,
        >(&state, body.request, |s, req| {
            get_receive_address_impl(s, req)
        }),
        // Sync commands
        "zstash_start_sync" => {
            dispatch::<StartSyncRequest, StartSyncResponse>(&state, body.request, |s, req| {
                start_sync_impl(s, req)
            })
        }
        "zstash_stop_sync" => {
            dispatch::<StopSyncRequest, StopSyncResponse>(&state, body.request, |s, req| {
                stop_sync_impl(s, req)
            })
        }
        "zstash_get_sync_progress" => dispatch::<GetSyncProgressRequest, GetSyncProgressResponse>(
            &state,
            body.request,
            |s, req| get_sync_progress_impl(s, req),
        ),
        _ => {
            warn!(command = %command, "Test bridge: unknown command");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Unknown command: {}", command)
                })),
            )
                .into_response()
        }
    }
}

/// Generic dispatch helper that deserializes the request, calls the handler, and serializes the response
fn dispatch<Req, Resp>(
    state: &TestBridgeState,
    request: Value,
    handler: impl FnOnce(&AppState, Req) -> IpcResult<Resp>,
) -> axum::response::Response
where
    Req: for<'de> Deserialize<'de>,
    Resp: Serialize,
{
    // Deserialize the request
    let req: Req = match serde_json::from_value(request) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid request: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Call the handler
    let result = handler(&state.app_state, req);

    // Serialize and return the response
    (StatusCode::OK, Json(result)).into_response()
}

// ============================================================================
// Command Implementations
//
// These are simplified versions of the Tauri commands that work directly with
// &AppState instead of tauri::State<'_, AppState>.
// ============================================================================

fn list_wallets_impl(
    state: &AppState,
    request: ListWalletsRequest,
) -> IpcResult<ListWalletsResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let wallets = mgr.list_wallets()?;
        Ok(ListWalletsResponse {
            schema_version: SCHEMA_VERSION,
            wallets,
        })
    })
}

fn create_wallet_impl(
    state: &AppState,
    request: CreateWalletRequest,
) -> IpcResult<CreateWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    // In test-bridge mode, skip birthday height fetch to avoid nested runtime issues.
    // Tests can use Sapling activation height (None) which is fine for testing.
    let birthday_height: Option<u32> = None;

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let created = mgr.create_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
            birthday_height,
        )?;

        Ok(CreateWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: created.wallet,
            seed_phrase: created.seed_phrase,
            backup_challenge: created.backup_challenge,
        })
    })
}

fn load_wallet_impl(state: &AppState, request: LoadWalletRequest) -> IpcResult<LoadWalletResponse> {
    use zstash_core::domain::{SyncPhase, SyncProgress, WalletLockStatus};
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");

        // Stop sync for the previously-active wallet (best effort)
        if let Some(prev_wallet_id) = mgr.active_wallet_info().map(|w| w.id)
            && prev_wallet_id != request.wallet_id
        {
            mgr.observe_sync_stop_requested(prev_wallet_id);
            let _ = state.sync_service.stop_sync(prev_wallet_id, None);
            mgr.observe_sync_progress(
                prev_wallet_id,
                SyncProgress {
                    phase: SyncPhase::Idle,
                    scan_frontier_height: 0,
                    wallet_tip_height: 0,
                    progress_percent: 0,
                    eta_seconds: None,
                    retry_in_seconds: None,
                    error_message: None,
                },
            );
        }

        let (wallet, lock_status) = mgr.load_wallet(request.wallet_id)?;

        let accounts = if lock_status == WalletLockStatus::Locked {
            vec![]
        } else {
            load_accounts_for_wallet(&mut mgr, wallet.id)?
        };

        // Note: We skip auto-sync in test bridge mode since we don't have an AppHandle
        // for event emission. Tests can manually call start_sync if needed.

        Ok(LoadWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet,
            lock_status,
            accounts,
        })
    })
}

fn get_wallet_status_impl(
    state: &AppState,
    request: GetWalletStatusRequest,
) -> IpcResult<GetWalletStatusResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.compute_wallet_status(request.wallet_id)?;
        Ok(GetWalletStatusResponse {
            schema_version: SCHEMA_VERSION,
            status,
        })
    })
}

fn unlock_wallet_impl(
    state: &AppState,
    request: UnlockWalletRequest,
) -> IpcResult<UnlockWalletResponse> {
    use zstash_core::domain::WalletLockStatus;
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.unlock_wallet(
            request.wallet_id,
            &request.password,
            request.remember_unlock,
        )?;
        Ok(UnlockWalletResponse {
            schema_version: SCHEMA_VERSION,
            unlocked: status == WalletLockStatus::Unlocked,
        })
    })
}

fn lock_wallet_impl(state: &AppState, request: LockWalletRequest) -> IpcResult<LockWalletResponse> {
    use zstash_core::domain::{SyncPhase, SyncProgress, WalletLockStatus};
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.observe_sync_stop_requested(request.wallet_id);
        let _ = state.sync_service.stop_sync(request.wallet_id, None);
        mgr.observe_sync_progress(
            request.wallet_id,
            SyncProgress {
                phase: SyncPhase::Idle,
                scan_frontier_height: 0,
                wallet_tip_height: 0,
                progress_percent: 0,
                eta_seconds: None,
                retry_in_seconds: None,
                error_message: None,
            },
        );
        let status = mgr.lock_wallet(request.wallet_id)?;
        Ok(LockWalletResponse {
            schema_version: SCHEMA_VERSION,
            locked: status == WalletLockStatus::Locked,
        })
    })
}

fn logout_wallet_impl(
    state: &AppState,
    request: LogoutWalletRequest,
) -> IpcResult<LogoutWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let _ = state.sync_service.stop_sync(request.wallet_id, None);
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.logout_wallet(request.wallet_id)?;
        Ok(LogoutWalletResponse {
            schema_version: SCHEMA_VERSION,
            success: true,
        })
    })
}

fn get_balance_impl(state: &AppState, request: GetBalanceRequest) -> IpcResult<GetBalanceResponse> {
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

fn get_receive_address_impl(
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

fn start_sync_impl(state: &AppState, request: StartSyncRequest) -> IpcResult<StartSyncResponse> {
    use std::path::PathBuf;
    use zstash_core::domain::{SyncPhase, SyncProgress, WalletLockStatus};
    use zstash_core::errors;
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (wallet, lock_status) = mgr.load_wallet(request.wallet_id)?;

        if lock_status != WalletLockStatus::Unlocked {
            return Err(zstash_engine::error::ipc_err(
                errors::WALLET_LOCKED,
                "wallet locked",
            ));
        }

        // Get wallet DB path
        let wallet_db_path =
            zstash_engine::db::wallet_meta::get_wallet(mgr.app_db().conn(), wallet.id)
                .map_err(|e| anyhow::anyhow!(e))?
                .map(|(_, dir)| PathBuf::from(dir).join("wallet.sqlite"))
                .ok_or_else(|| {
                    zstash_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not found")
                })?;

        let wallet_dek = mgr.unlocked_wallet_dek(wallet.id)?;
        let account_ids = mgr.list_wallet_db_account_ids(wallet.id)?;

        // In test bridge mode, we start sync without event handlers since we don't
        // have an AppHandle for emitting events. Progress can be polled via get_sync_progress.
        match state.sync_service.start_sync(
            mgr.app_db(),
            wallet.id,
            wallet.network,
            wallet_db_path,
            wallet_dek,
            account_ids,
            Some(std::sync::Arc::clone(&state.tor_manager)),
            None, // No progress handler
            None, // No balance handler
        ) {
            Ok(()) => {
                mgr.observe_sync_progress(
                    wallet.id,
                    SyncProgress {
                        phase: SyncPhase::Preparing,
                        scan_frontier_height: 0,
                        wallet_tip_height: 0,
                        progress_percent: 0,
                        eta_seconds: None,
                        retry_in_seconds: None,
                        error_message: None,
                    },
                );
                Ok(StartSyncResponse {
                    schema_version: SCHEMA_VERSION,
                    started: true,
                })
            }
            Err(err)
                if zstash_engine::error::find_engine_ipc_error(&err)
                    .is_some_and(|e| e.code == errors::SYNC_IN_PROGRESS) =>
            {
                Ok(StartSyncResponse {
                    schema_version: SCHEMA_VERSION,
                    started: false,
                })
            }
            Err(err) => Err(err),
        }
    })
}

fn stop_sync_impl(state: &AppState, request: StopSyncRequest) -> IpcResult<StopSyncResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.observe_sync_stop_requested(request.wallet_id);
    }

    map_anyhow(|| {
        state.sync_service.stop_sync(request.wallet_id, None)?;
        Ok(StopSyncResponse {
            schema_version: SCHEMA_VERSION,
            stopped: true,
        })
    })
}

fn get_sync_progress_impl(
    state: &AppState,
    request: GetSyncProgressRequest,
) -> IpcResult<GetSyncProgressResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        Ok(GetSyncProgressResponse {
            schema_version: SCHEMA_VERSION,
            progress: state.sync_service.get_progress(request.wallet_id),
        })
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper to map anyhow errors to IpcResult
fn map_anyhow<T, F>(f: F) -> IpcResult<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    match f() {
        Ok(v) => IpcResult::Ok { ok: v },
        Err(e) => {
            let message = format!("{:#}", e);
            error!(error = %message, "Command failed");
            IpcResult::Err {
                err: zstash_core::ipc::v1::common::IpcError {
                    code: "E9002".to_string(), // INTERNAL_ERROR
                    message,
                    details: None,
                },
            }
        }
    }
}

/// Load accounts for a wallet (helper extracted from wallet.rs)
fn load_accounts_for_wallet(
    mgr: &mut zstash_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<Vec<zstash_core::domain::AccountInfo>> {
    use std::collections::HashMap;
    use zstash_core::domain::{AccountInfo, AccountType};

    let wallet_db_accounts = mgr.list_wallet_db_account_ids(wallet_id)?;
    let meta_accounts =
        zstash_engine::db::account_meta::list_accounts(mgr.app_db().conn(), wallet_id)
            .map_err(|e| anyhow::anyhow!(e))?;

    let meta_by_id: HashMap<u32, AccountInfo> =
        meta_accounts.into_iter().map(|a| (a.id, a)).collect();

    let mut out = Vec::with_capacity(wallet_db_accounts.len());
    for account_id in wallet_db_accounts {
        if let Some(meta) = meta_by_id.get(&account_id) {
            out.push(meta.clone());
            continue;
        }

        warn!(account_id, "Account metadata missing; applying defaults");
        out.push(AccountInfo {
            id: account_id,
            name: format!("Account {}", account_id + 1),
            account_type: if account_id == 0 {
                AccountType::Software
            } else {
                AccountType::HardwareSigner
            },
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_port_is_expected_value() {
        assert_eq!(TEST_BRIDGE_PORT, 19816);
    }
}
