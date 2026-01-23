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

use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::{
    Json, Router,
    extract::{Path, State as AxumState},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info, warn};

use zstash_core::domain::{FiatDisplaySettings, Network, SwapIntent};
use zstash_core::errors;
use zstash_core::ipc::v1::commands::address::{
    GetReceiveAddressRequest, GetReceiveAddressResponse,
};
use zstash_core::ipc::v1::commands::backup::{
    GetBackupChallengeRequest, GetBackupChallengeResponse, RestoreWalletRequest,
    RestoreWalletResponse, VerifyBackupRequest, VerifyBackupResponse,
};
use zstash_core::ipc::v1::commands::balance::{GetBalanceRequest, GetBalanceResponse};
use zstash_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetExchangeRateResponse, GetFiatSettingsRequest,
    GetFiatSettingsResponse, SetFiatSettingsRequest, SetFiatSettingsResponse,
};
use zstash_core::ipc::v1::commands::keystone::{
    BuildSigningRequestRequest, BuildSigningRequestResponse, CreateKeystoneWalletRequest,
    CreateKeystoneWalletResponse, FinalizeSigningRequest, FinalizeSigningResponse,
    ImportUfvkRequest, ImportUfvkResponse,
};
use zstash_core::ipc::v1::commands::logs::{GetLogLocationRequest, GetLogLocationResponse};
use zstash_core::ipc::v1::commands::server::{
    AddServerRequest, AddServerResponse, ListServersRequest, ListServersResponse,
    SetDefaultServerRequest, SetDefaultServerResponse, TestServerRequest, TestServerResponse,
};
use zstash_core::ipc::v1::commands::swap::{
    GetSwapStatusRequest, GetSwapStatusResponse, ListSwapsRequest, ListSwapsResponse,
    RequestSwapQuoteRequest, RequestSwapQuoteResponse, StartSwapRequest, StartSwapResponse,
};
use zstash_core::ipc::v1::commands::sync::{
    GetSyncProgressRequest, GetSyncProgressResponse, StartSyncRequest, StartSyncResponse,
    StopSyncRequest, StopSyncResponse,
};
use zstash_core::ipc::v1::commands::tor::{
    GetTorStateRequest, GetTorStateResponse, SetTorEnabledRequest, SetTorEnabledResponse,
};
use zstash_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use zstash_core::ipc::v1::commands::version::{GetVersionRequest, GetVersionResponse};
use zstash_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse,
    LockWalletRequest, LockWalletResponse, LogoutWalletRequest, LogoutWalletResponse,
    ReauthWalletRequest, ReauthWalletResponse, UnlockWalletRequest, UnlockWalletResponse,
    ViewSeedPhraseRequest, ViewSeedPhraseResponse,
};
use zstash_core::ipc::v1::common::IpcResult;

use crate::state::AppState;

/// Test bridge server port (localhost-only)
pub const TEST_BRIDGE_PORT: u16 = 19816;

/// Timeout for server probe to avoid UI blocking when offline.
const SERVER_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

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
        .allow_origin(AllowOrigin::list([
            "http://localhost:1420".parse().unwrap(),
            "http://127.0.0.1:1420".parse().unwrap(),
        ]))
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
        "zstash_reauth_wallet" => {
            dispatch::<ReauthWalletRequest, ReauthWalletResponse>(&state, body.request, |s, req| {
                reauth_wallet_impl(s, req)
            })
        }
        "zstash_view_seed_phrase" => dispatch::<ViewSeedPhraseRequest, ViewSeedPhraseResponse>(
            &state,
            body.request,
            |s, req| view_seed_phrase_impl(s, req),
        ),
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
        // Backup commands
        "zstash_get_backup_challenge" => dispatch::<
            GetBackupChallengeRequest,
            GetBackupChallengeResponse,
        >(&state, body.request, |s, req| {
            get_backup_challenge_impl(s, req)
        }),
        "zstash_verify_backup" => {
            dispatch::<VerifyBackupRequest, VerifyBackupResponse>(&state, body.request, |s, req| {
                verify_backup_impl(s, req)
            })
        }
        "zstash_restore_wallet" => dispatch::<RestoreWalletRequest, RestoreWalletResponse>(
            &state,
            body.request,
            |s, req| restore_wallet_impl(s, req),
        ),
        // Transactions commands
        "zstash_prepare_send" => {
            dispatch::<PrepareSendRequest, PrepareSendResponse>(&state, body.request, |s, req| {
                prepare_send_impl(s, req)
            })
        }
        "zstash_confirm_send" => {
            dispatch::<ConfirmSendRequest, ConfirmSendResponse>(&state, body.request, |s, req| {
                confirm_send_impl(s, req)
            })
        }
        "zstash_cancel_send" => {
            dispatch::<CancelSendRequest, CancelSendResponse>(&state, body.request, |s, req| {
                cancel_send_impl(s, req)
            })
        }
        "zstash_retry_broadcast" => dispatch::<RetryBroadcastRequest, RetryBroadcastResponse>(
            &state,
            body.request,
            |s, req| retry_broadcast_impl(s, req),
        ),
        "zstash_list_transactions" => {
            dispatch::<ListTransactionsRequest, ListTransactionsResponse>(
                &state,
                body.request,
                |s, req| list_transactions_impl(s, req),
            )
        }
        "zstash_shield_funds" => {
            dispatch::<ShieldFundsRequest, ShieldFundsResponse>(&state, body.request, |s, req| {
                shield_funds_impl(s, req)
            })
        }
        // Keystone commands
        "zstash_import_ufvk" => {
            dispatch::<ImportUfvkRequest, ImportUfvkResponse>(&state, body.request, |s, req| {
                import_ufvk_impl(s, req)
            })
        }
        "zstash_build_signing_request" => dispatch::<
            BuildSigningRequestRequest,
            BuildSigningRequestResponse,
        >(&state, body.request, |s, req| {
            build_signing_request_impl(s, req)
        }),
        "zstash_finalize_signing" => dispatch::<FinalizeSigningRequest, FinalizeSigningResponse>(
            &state,
            body.request,
            |s, req| finalize_signing_impl(s, req),
        ),
        "zstash_create_keystone_wallet" => dispatch::<
            CreateKeystoneWalletRequest,
            CreateKeystoneWalletResponse,
        >(&state, body.request, |s, req| {
            create_keystone_wallet_impl(s, req)
        }),
        // Swap commands
        "zstash_request_swap_quote" => {
            dispatch::<RequestSwapQuoteRequest, RequestSwapQuoteResponse>(
                &state,
                body.request,
                |s, req| request_swap_quote_impl(s, req),
            )
        }
        "zstash_start_swap" => {
            dispatch::<StartSwapRequest, StartSwapResponse>(&state, body.request, |s, req| {
                start_swap_impl(s, req)
            })
        }
        "zstash_get_swap_status" => dispatch::<GetSwapStatusRequest, GetSwapStatusResponse>(
            &state,
            body.request,
            |s, req| get_swap_status_impl(s, req),
        ),
        "zstash_list_swaps" => {
            dispatch::<ListSwapsRequest, ListSwapsResponse>(&state, body.request, |s, req| {
                list_swaps_impl(s, req)
            })
        }
        // Tor commands
        "zstash_set_tor_enabled" => dispatch::<SetTorEnabledRequest, SetTorEnabledResponse>(
            &state,
            body.request,
            |s, req| set_tor_enabled_impl(s, req),
        ),
        "zstash_get_tor_state" => {
            dispatch::<GetTorStateRequest, GetTorStateResponse>(&state, body.request, |s, req| {
                get_tor_state_impl(s, req)
            })
        }
        // Server commands
        "zstash_add_server" => {
            dispatch::<AddServerRequest, AddServerResponse>(&state, body.request, |s, req| {
                add_server_impl(s, req)
            })
        }
        "zstash_set_default_server" => {
            dispatch::<SetDefaultServerRequest, SetDefaultServerResponse>(
                &state,
                body.request,
                |s, req| set_default_server_impl(s, req),
            )
        }
        "zstash_test_server" => {
            dispatch::<TestServerRequest, TestServerResponse>(&state, body.request, |s, req| {
                test_server_impl(s, req)
            })
        }
        "zstash_list_servers" => {
            dispatch::<ListServersRequest, ListServersResponse>(&state, body.request, |s, req| {
                list_servers_impl(s, req)
            })
        }
        // Logs
        "zstash_get_log_location" => dispatch::<GetLogLocationRequest, GetLogLocationResponse>(
            &state,
            body.request,
            |s, req| get_log_location_impl(s, req),
        ),
        // Version
        "zstash_get_version" => {
            dispatch::<GetVersionRequest, GetVersionResponse>(&state, body.request, |s, req| {
                get_version_impl(s, req)
            })
        }
        // Fiat settings/exchange rate
        "zstash_get_fiat_settings" => dispatch::<GetFiatSettingsRequest, GetFiatSettingsResponse>(
            &state,
            body.request,
            |s, req| get_fiat_settings_impl(s, req),
        ),
        "zstash_set_fiat_settings" => dispatch::<SetFiatSettingsRequest, SetFiatSettingsResponse>(
            &state,
            body.request,
            |s, req| set_fiat_settings_impl(s, req),
        ),
        // NOTE: get_exchange_rate_impl is async (fetches rates from external API),
        // so it cannot use the synchronous `dispatch` helper. We handle deserialization
        // and response construction inline instead.
        "zstash_get_exchange_rate" => {
            let req: GetExchangeRateRequest = match serde_json::from_value(body.request) {
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
            let result = get_exchange_rate_impl(&state.app_state, req).await;
            (StatusCode::OK, Json(result)).into_response()
        }
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

    // WARNING: Test bridge divergence from production behavior
    // =========================================================
    // In production, birthday height is fetched from lightwalletd to optimize
    // initial sync. In test-bridge mode, we skip this to avoid nested runtime
    // issues, using Sapling activation height instead. This means test-created
    // wallets will scan from an earlier block height than production wallets.
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
// Additional Command Implementations
// ============================================================================

fn reauth_wallet_impl(
    state: &AppState,
    request: ReauthWalletRequest,
) -> IpcResult<ReauthWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (token, expires_at) =
            mgr.reauth_wallet(request.wallet_id, &request.password, request.purpose)?;
        Ok(ReauthWalletResponse {
            schema_version: SCHEMA_VERSION,
            reauth_token: token,
            expires_at: system_time_to_unix_ms(expires_at)?,
        })
    })
}

fn view_seed_phrase_impl(
    state: &AppState,
    request: ViewSeedPhraseRequest,
) -> IpcResult<ViewSeedPhraseResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let seed_phrase = mgr.view_seed_phrase(request.wallet_id, &request.reauth_token)?;
        Ok(ViewSeedPhraseResponse {
            schema_version: SCHEMA_VERSION,
            seed_phrase,
        })
    })
}

fn get_backup_challenge_impl(
    state: &AppState,
    request: GetBackupChallengeRequest,
) -> IpcResult<GetBackupChallengeResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let challenge = mgr.get_backup_challenge(request.wallet_id)?;
        Ok(GetBackupChallengeResponse {
            schema_version: SCHEMA_VERSION,
            challenge: zstash_core::ipc::v1::commands::backup::BackupChallenge {
                challenge_id: challenge.challenge_id,
                indices: challenge.indices,
                expires_at: challenge.expires_at,
            },
        })
    })
}

fn verify_backup_impl(
    state: &AppState,
    request: VerifyBackupRequest,
) -> IpcResult<VerifyBackupResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};
    use zstash_core::sensitive::SensitiveString;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let word_challenges: std::collections::HashMap<u8, SensitiveString> =
            request.word_challenges.into_iter().collect();
        mgr.verify_backup(request.wallet_id, &request.challenge_id, &word_challenges)?;
        Ok(VerifyBackupResponse {
            schema_version: SCHEMA_VERSION,
            verified: true,
        })
    })
}

fn restore_wallet_impl(
    state: &AppState,
    request: RestoreWalletRequest,
) -> IpcResult<RestoreWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    let RestoreWalletRequest {
        schema_version,
        name,
        network,
        password,
        remember_unlock,
        seed_phrase,
        birthday_date,
    } = request;

    if let Err(err) = ensure_schema_version(schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let restored = mgr.restore_wallet(
            &name,
            network,
            &password,
            remember_unlock,
            seed_phrase,
            birthday_date,
        )?;

        Ok(RestoreWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: restored.wallet,
            birthday_height: restored.birthday_height,
        })
    })
}

fn prepare_send_impl(
    state: &AppState,
    request: PrepareSendRequest,
) -> IpcResult<PrepareSendResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.prepare_send(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
        )
    })
}

fn confirm_send_impl(
    state: &AppState,
    request: ConfirmSendRequest,
) -> IpcResult<ConfirmSendResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.confirm_send(&request.proposal_id, &request.reauth_token, None)
    })
}

fn cancel_send_impl(state: &AppState, request: CancelSendRequest) -> IpcResult<CancelSendResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        Ok(CancelSendResponse {
            schema_version: SCHEMA_VERSION,
            cancelled: mgr.cancel_send(&request.proposal_id),
        })
    })
}

fn retry_broadcast_impl(
    state: &AppState,
    request: RetryBroadcastRequest,
) -> IpcResult<RetryBroadcastResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let txid = mgr.retry_broadcast(&request.txid, &request.reauth_token, None)?;
        Ok(RetryBroadcastResponse {
            schema_version: SCHEMA_VERSION,
            txid,
        })
    })
}

fn list_transactions_impl(
    state: &AppState,
    request: ListTransactionsRequest,
) -> IpcResult<ListTransactionsResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.list_transactions(request.account_id, request.limit, request.offset)
    })
}

fn shield_funds_impl(
    state: &AppState,
    request: ShieldFundsRequest,
) -> IpcResult<ShieldFundsResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.shield_funds(
            request.account_id,
            request.consolidate,
            &request.reauth_token,
            None,
        )
    })
}

fn import_ufvk_impl(state: &AppState, request: ImportUfvkRequest) -> IpcResult<ImportUfvkResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let account = mgr.import_ufvk(
            request.wallet_id,
            &request.ufvk,
            &request.name,
            request.seed_fingerprint.as_deref(),
            request.zip32_account_index,
        )?;
        Ok(ImportUfvkResponse {
            schema_version: SCHEMA_VERSION,
            account,
        })
    })
}

fn build_signing_request_impl(
    state: &AppState,
    request: BuildSigningRequestRequest,
) -> IpcResult<BuildSigningRequestResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.build_signing_request(
            request.account_id,
            &request.recipient,
            &request.amount,
            request.memo.as_deref(),
            request.allow_transparent_recipient,
        )
    })
}

fn finalize_signing_impl(
    state: &AppState,
    request: FinalizeSigningRequest,
) -> IpcResult<FinalizeSigningResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        mgr.finalize_signing(
            &request.signing_request_id,
            &request.signed_payload,
            &request.reauth_token,
            None,
        )
    })
}

fn create_keystone_wallet_impl(
    state: &AppState,
    request: CreateKeystoneWalletRequest,
) -> IpcResult<CreateKeystoneWalletResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (wallet, account) = mgr.create_keystone_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
            &request.ufvk,
            request.birthday_height,
            request.seed_fingerprint.as_deref(),
            request.zip32_account_index,
        )?;

        Ok(CreateKeystoneWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet,
            account,
        })
    })
}

fn request_swap_quote_impl(
    state: &AppState,
    request: RequestSwapQuoteRequest,
) -> IpcResult<RequestSwapQuoteResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zstash_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        let intent = SwapIntent {
            swap_type: request.swap_type,
            input_asset: request.input_asset,
            input_amount: request.input_amount,
            output_asset: request.output_asset,
            destination_address: request.destination_address,
            refund_address: request.refund_address,
        };

        state
            .swap_service
            .request_swap_quote(wallet.id, wallet.network, intent)
    })
}

fn start_swap_impl(state: &AppState, request: StartSwapRequest) -> IpcResult<StartSwapResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zstash_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state.swap_service.start_swap(
            wallet.id,
            wallet.network,
            &request.quote_id,
            request.allow_transparent_interaction,
            request.reauth_token.as_deref(),
            None,
        )
    })
}

fn get_swap_status_impl(
    state: &AppState,
    request: GetSwapStatusRequest,
) -> IpcResult<GetSwapStatusResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zstash_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state
            .swap_service
            .get_swap_status(wallet.id, request.swap_id)
    })
}

fn list_swaps_impl(state: &AppState, request: ListSwapsRequest) -> IpcResult<ListSwapsResponse> {
    use zstash_core::ipc::v1::common::ensure_schema_version;

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let wallet = {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            mgr.active_wallet_info().ok_or_else(|| {
                zstash_engine::error::ipc_err(errors::WALLET_NOT_FOUND, "wallet not loaded")
            })?
        };

        state.swap_service.list_swaps(wallet.id)
    })
}

fn set_tor_enabled_impl(
    state: &AppState,
    request: SetTorEnabledRequest,
) -> IpcResult<SetTorEnabledResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let handle = tokio::runtime::Handle::try_current().ok();
    let _guard = handle.as_ref().map(|h| h.enter());

    map_anyhow(|| {
        let running_wallets = if request.enabled {
            let wallets = state.sync_service.running_wallet_ids();
            for wallet_id in &wallets {
                let _ = state.sync_service.stop_sync(*wallet_id, None);
            }
            wallets
        } else {
            Vec::new()
        };

        let next_state = state
            .tor_manager
            .set_enabled(request.enabled)
            .map_err(|e| {
                zstash_engine::error::ipc_err(errors::TOR_CONNECTION_FAILED, e.to_string())
            })?;

        let updated_at_ms = system_time_to_unix_ms(std::time::SystemTime::now()).map_err(|e| {
            zstash_engine::error::ipc_err(errors::INTERNAL_ERROR, format!("time error: {e}"))
        })?;

        {
            let mgr = state.wallet_manager.lock().expect("mutex poisoned");
            zstash_engine::db::tor_meta::upsert_tor_state(
                mgr.app_db().conn(),
                &next_state,
                updated_at_ms,
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        }

        if request.enabled && !running_wallets.is_empty() {
            for wallet_id in running_wallets {
                let _ = start_sync_impl(
                    state,
                    StartSyncRequest {
                        schema_version: SCHEMA_VERSION,
                        wallet_id,
                    },
                );
            }
        }

        Ok(SetTorEnabledResponse {
            schema_version: SCHEMA_VERSION,
            state: next_state,
        })
    })
}

fn get_tor_state_impl(
    state: &AppState,
    request: GetTorStateRequest,
) -> IpcResult<GetTorStateResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        Ok(GetTorStateResponse {
            schema_version: SCHEMA_VERSION,
            state: state.tor_manager.state(),
        })
    })
}

fn add_server_impl(state: &AppState, request: AddServerRequest) -> IpcResult<AddServerResponse> {
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

fn set_default_server_impl(
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

fn list_servers_impl(
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

fn test_server_impl(state: &AppState, request: TestServerRequest) -> IpcResult<TestServerResponse> {
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

fn get_log_location_impl(
    state: &AppState,
    request: GetLogLocationRequest,
) -> IpcResult<GetLogLocationResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

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

fn get_version_impl(
    _state: &AppState,
    request: GetVersionRequest,
) -> IpcResult<GetVersionResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    IpcResult::ok(GetVersionResponse {
        schema_version: SCHEMA_VERSION,
        version_info: zstash_core::version::VersionInfo::current(),
    })
}

fn get_fiat_settings_impl(
    state: &AppState,
    request: GetFiatSettingsRequest,
) -> IpcResult<GetFiatSettingsResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let settings = zstash_engine::db::fiat_meta::get_fiat_settings(mgr.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(GetFiatSettingsResponse {
            schema_version: SCHEMA_VERSION,
            settings,
        })
    })
}

fn set_fiat_settings_impl(
    state: &AppState,
    request: SetFiatSettingsRequest,
) -> IpcResult<SetFiatSettingsResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        if request.enabled && !request.privacy_acknowledged {
            return Err(zstash_engine::error::ipc_err(
                errors::EXCHANGE_RATE_PRIVACY_ACK_REQUIRED,
                "Privacy acknowledgement required to enable fiat display",
            )
            .into());
        }

        let settings = FiatDisplaySettings {
            enabled: request.enabled,
            currency: request.currency,
            privacy_acknowledged: request.privacy_acknowledged,
        };

        let updated_at_ms = system_time_to_unix_ms(std::time::SystemTime::now()).map_err(|e| {
            zstash_engine::error::ipc_err(errors::INTERNAL_ERROR, format!("time error: {e}"))
        })?;

        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        zstash_engine::db::fiat_meta::upsert_fiat_settings(
            mgr.app_db().conn(),
            &settings,
            updated_at_ms,
        )
        .map_err(|e| anyhow::anyhow!(e))?;

        Ok(SetFiatSettingsResponse {
            schema_version: SCHEMA_VERSION,
            settings,
        })
    })
}

async fn get_exchange_rate_impl(
    state: &AppState,
    request: GetExchangeRateRequest,
) -> IpcResult<GetExchangeRateResponse> {
    use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    let settings = {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        match zstash_engine::db::fiat_meta::get_fiat_settings(mgr.app_db().conn()) {
            Ok(s) => s,
            Err(e) => {
                return IpcResult::err(
                    errors::INTERNAL_ERROR,
                    format!("Failed to get fiat settings: {e}"),
                );
            }
        }
    };

    if !settings.enabled {
        return IpcResult::ok(GetExchangeRateResponse {
            schema_version: SCHEMA_VERSION,
            rate: None,
            is_stale: true,
            fiat_enabled: false,
            refresh_cooldown_secs: 0,
        });
    }

    let cooldown = state.exchange_rate_service.refresh_cooldown_secs() as u32;

    if let Some(rate) = state
        .exchange_rate_service
        .get_cached_rate(settings.currency)
    {
        if !rate.is_stale() && !request.force_refresh {
            return IpcResult::ok(GetExchangeRateResponse {
                schema_version: SCHEMA_VERSION,
                rate: Some(rate),
                is_stale: false,
                fiat_enabled: true,
                refresh_cooldown_secs: cooldown,
            });
        }
    }

    match state
        .exchange_rate_service
        .get_rate(settings.currency, request.force_refresh)
        .await
    {
        Ok(rate) => {
            let is_stale = rate.is_stale();
            IpcResult::ok(GetExchangeRateResponse {
                schema_version: SCHEMA_VERSION,
                rate: Some(rate),
                is_stale,
                fiat_enabled: true,
                refresh_cooldown_secs: state.exchange_rate_service.refresh_cooldown_secs() as u32,
            })
        }
        Err(zstash_network::exchange_rate::ExchangeRateError::RateLimited(secs)) => {
            let cached = state
                .exchange_rate_service
                .get_cached_rate(settings.currency);
            let is_stale = cached.as_ref().is_some_and(|r| r.is_stale());
            IpcResult::ok(GetExchangeRateResponse {
                schema_version: SCHEMA_VERSION,
                rate: cached,
                is_stale,
                fiat_enabled: true,
                refresh_cooldown_secs: secs as u32,
            })
        }
        Err(e) => {
            let cached = state
                .exchange_rate_service
                .get_cached_rate(settings.currency);
            if cached.is_some() {
                return IpcResult::ok(GetExchangeRateResponse {
                    schema_version: SCHEMA_VERSION,
                    rate: cached,
                    is_stale: true,
                    fiat_enabled: true,
                    refresh_cooldown_secs: state.exchange_rate_service.refresh_cooldown_secs()
                        as u32,
                });
            }
            IpcResult::err(
                errors::EXCHANGE_RATE_FETCH_FAILED,
                format!("Failed to fetch exchange rate: {e}"),
            )
        }
    }
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
        Err(err) => {
            error!(error = ?err, "Command failed");
            IpcResult::Err {
                err: to_ipc_error(err),
            }
        }
    }
}

fn to_ipc_error(err: anyhow::Error) -> zstash_core::ipc::v1::common::IpcError {
    if let Some(engine) = zstash_engine::error::find_engine_ipc_error(&err) {
        return zstash_core::ipc::v1::common::IpcError {
            code: engine.code.to_string(),
            message: engine.message.clone(),
            details: engine.details.clone(),
        };
    }

    zstash_core::ipc::v1::common::IpcError {
        code: errors::INTERNAL_ERROR.to_string(),
        message: "internal error".to_string(),
        details: None,
    }
}

fn system_time_to_unix_ms(time: std::time::SystemTime) -> anyhow::Result<i64> {
    let duration = time.duration_since(std::time::UNIX_EPOCH)?;
    Ok(i64::try_from(duration.as_millis())?)
}

fn fallback_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("create tokio runtime"))
}

fn block_on<F: Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => fallback_runtime().block_on(future),
    }
}

fn probe_chain_name_with_timeout(
    client: &zstash_network::grpc_client::GrpcClient,
) -> anyhow::Result<String> {
    let info = block_on(async {
        match tokio::time::timeout(SERVER_PROBE_TIMEOUT, client.probe_server()).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("connection timed out")),
        }
    })?;

    Ok(info.chain_name)
}

fn parse_network(chain_name: &str) -> anyhow::Result<Network> {
    let name = chain_name.trim().to_lowercase();
    match name.as_str() {
        "main" | "mainnet" => Ok(Network::Mainnet),
        "test" | "testnet" => Ok(Network::Testnet),
        other => Err(zstash_engine::error::ipc_err(
            errors::INVALID_REQUEST,
            format!("unsupported chain_name: {other}"),
        )),
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
