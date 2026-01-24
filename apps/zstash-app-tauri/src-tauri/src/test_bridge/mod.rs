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
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info, warn};

use zstash_core::ipc::v1::commands::address::{
    GetReceiveAddressRequest, GetReceiveAddressResponse,
};
use zstash_core::ipc::v1::commands::backup::{
    GetBackupChallengeRequest, GetBackupChallengeResponse, RestoreWalletRequest,
    RestoreWalletResponse, VerifyBackupRequest, VerifyBackupResponse,
};
use zstash_core::ipc::v1::commands::balance::{GetBalanceRequest, GetBalanceResponse};
use zstash_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetFiatSettingsRequest, GetFiatSettingsResponse,
    SetFiatSettingsRequest, SetFiatSettingsResponse,
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

mod handlers;
mod helpers;

pub use handlers::*;
pub use helpers::*;

/// Test bridge server port (localhost-only).
///
/// Port 19816 was chosen arbitrarily to avoid conflicts with common
/// development ports (1420, 3000, 8080, etc.). Not an IANA registered port.
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
        .allow_origin(AllowOrigin::list([
            "http://localhost:1420".parse().unwrap(),
            "http://127.0.0.1:1420".parse().unwrap(),
        ]))
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/invoke/{command}", post(invoke_command))
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
            get_wallet_status_impl,
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
            view_seed_phrase_impl,
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
            restore_wallet_impl,
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
            retry_broadcast_impl,
        ),
        "zstash_list_transactions" => {
            dispatch::<ListTransactionsRequest, ListTransactionsResponse>(
                &state,
                body.request,
                list_transactions_impl,
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
            finalize_signing_impl,
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
                request_swap_quote_impl,
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
            get_swap_status_impl,
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
            set_tor_enabled_impl,
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
                set_default_server_impl,
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
            get_log_location_impl,
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
            get_fiat_settings_impl,
        ),
        "zstash_set_fiat_settings" => dispatch::<SetFiatSettingsRequest, SetFiatSettingsResponse>(
            &state,
            body.request,
            set_fiat_settings_impl,
        ),
        // NOTE: get_exchange_rate_impl is async (fetches rates from external API),
        // so it cannot use the synchronous `dispatch` helper.
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
            get_sync_progress_impl,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_port_is_expected_value() {
        assert_eq!(TEST_BRIDGE_PORT, 19816);
    }
}
