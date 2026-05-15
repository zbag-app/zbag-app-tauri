//! HTTP Test Bridge for E2E Testing
//!
//! This module provides an HTTP server that exposes Tauri commands via REST endpoints,
//! enabling Playwright and Claude Code (via Chrome MCP) to test bagz against the real
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
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::{
    Json, Router,
    extract::{Path, State as AxumState},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tracing::{error, info, warn};

use bagz_core::ipc::v1::commands::address::{
    GetReceiveAddressRequest, GetReceiveAddressResponse,
};
use bagz_core::ipc::v1::commands::backup::{
    GetBackupChallengeRequest, GetBackupChallengeResponse, RestoreWalletRequest,
    RestoreWalletResponse, VerifyBackupRequest, VerifyBackupResponse,
};
use bagz_core::ipc::v1::commands::balance::{GetBalanceRequest, GetBalanceResponse};
use bagz_core::ipc::v1::commands::exchange_rate::{
    GetExchangeRateRequest, GetFiatSettingsRequest, GetFiatSettingsResponse,
    SetFiatSettingsRequest, SetFiatSettingsResponse,
};
use bagz_core::ipc::v1::commands::keystone::{
    BuildSigningRequestRequest, BuildSigningRequestResponse, CreateKeystoneWalletRequest,
    CreateKeystoneWalletResponse, FinalizeSigningRequest, FinalizeSigningResponse,
    ImportUfvkRequest, ImportUfvkResponse,
};
use bagz_core::ipc::v1::commands::logs::{GetLogLocationRequest, GetLogLocationResponse};
use bagz_core::ipc::v1::commands::server::{
    AddServerRequest, AddServerResponse, ListServersRequest, ListServersResponse,
    SetDefaultServerRequest, SetDefaultServerResponse, TestServerRequest, TestServerResponse,
};
use bagz_core::ipc::v1::commands::swap::{
    GetSwapStatusRequest, GetSwapStatusResponse, ListSwapsRequest, ListSwapsResponse,
    RequestSwapQuoteRequest, RequestSwapQuoteResponse, StartSwapRequest, StartSwapResponse,
};
use bagz_core::ipc::v1::commands::sync::{
    GetSyncProgressRequest, GetSyncProgressResponse, StartSyncRequest, StartSyncResponse,
    StopSyncRequest, StopSyncResponse,
};
use bagz_core::ipc::v1::commands::tor::{
    GetTorStateRequest, GetTorStateResponse, SetTorEnabledRequest, SetTorEnabledResponse,
};
use bagz_core::ipc::v1::commands::transaction::{
    CancelSendRequest, CancelSendResponse, ConfirmSendRequest, ConfirmSendResponse,
    ListTransactionsRequest, ListTransactionsResponse, PrepareSendRequest, PrepareSendResponse,
    RetryBroadcastRequest, RetryBroadcastResponse, ShieldFundsRequest, ShieldFundsResponse,
};
use bagz_core::ipc::v1::commands::version::{GetVersionRequest, GetVersionResponse};
use bagz_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse,
    LockWalletRequest, LockWalletResponse, LogoutWalletRequest, LogoutWalletResponse,
    ReauthWalletRequest, ReauthWalletResponse, UnlockWalletRequest, UnlockWalletResponse,
    ViewSeedPhraseRequest, ViewSeedPhraseResponse,
};
use bagz_core::ipc::v1::common::IpcResult;

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

/// Maximum concurrent requests the test bridge will process.
/// Prevents runaway tests from overwhelming the server.
const MAX_CONCURRENT_REQUESTS: usize = 50;

/// Maximum request body size (1MB).
const MAX_REQUEST_BODY_SIZE: usize = 1024 * 1024;

const DEFAULT_ALLOWED_ORIGINS: [&str; 2] = ["http://localhost:1420", "http://127.0.0.1:1420"];
const SENSITIVE_CONFIRM_HEADER: &str = "X-Test-Bridge-Confirm";
const SENSITIVE_CONFIRM_VALUE: &str = "true";
const SENSITIVE_MIN_INTERVAL: Duration = Duration::from_secs(2);
const SENSITIVE_COMMANDS: [&str; 3] = [
    "bagz_view_seed_phrase",
    "bagz_restore_wallet",
    "bagz_confirm_send",
];
const RATE_LIMITED_SENSITIVE_COMMANDS: [&str; 1] = ["bagz_view_seed_phrase"];

/// Shared state for the test bridge server
pub struct TestBridgeState {
    pub app_state: Arc<AppState>,
    sensitive_last_call: Mutex<Option<Instant>>,
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
    let bridge_state = Arc::new(TestBridgeState {
        app_state,
        sensitive_last_call: Mutex::new(None),
    });

    // CORS layer to allow requests from Vite dev server (configurable).
    let cors = cors_layer();

    // Rate limiting to protect against runaway tests
    let limits = ServiceBuilder::new()
        .concurrency_limit(MAX_CONCURRENT_REQUESTS)
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_SIZE));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/invoke/{command}", post(invoke_command))
        .layer(cors)
        .layer(limits)
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

fn cors_layer() -> CorsLayer {
    let origins = allowed_origins();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods(Any)
        .allow_headers(Any)
}

fn allowed_origins() -> Vec<HeaderValue> {
    static ORIGINS: OnceLock<Vec<HeaderValue>> = OnceLock::new();
    ORIGINS.get_or_init(parse_allowed_origins).clone()
}

fn parse_allowed_origins() -> Vec<HeaderValue> {
    let mut parsed = Vec::new();

    if let Ok(raw) = std::env::var("BAGZ_TEST_BRIDGE_ALLOWED_ORIGINS") {
        for origin in raw.split(',') {
            let trimmed = origin.trim();
            if trimmed.is_empty() {
                continue;
            }
            match trimmed.parse::<HeaderValue>() {
                Ok(value) => parsed.push(value),
                Err(err) => {
                    warn!(origin = trimmed, error = %err, "invalid test-bridge origin");
                }
            }
        }
    }

    if parsed.is_empty() {
        for origin in DEFAULT_ALLOWED_ORIGINS {
            parsed.push(origin.parse().expect("default origin must parse"));
        }
    }

    parsed
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": "1",
        "test_bridge": true
    }))
}

fn confirm_sensitive_access(
    state: &TestBridgeState,
    headers: &HeaderMap,
    rate_limit: bool,
) -> Result<(), axum::response::Response> {
    let confirmed = headers
        .get(SENSITIVE_CONFIRM_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case(SENSITIVE_CONFIRM_VALUE))
        .unwrap_or(false);

    if !confirmed {
        warn!(
            header = SENSITIVE_CONFIRM_HEADER,
            "Test bridge: missing confirmation header for sensitive endpoint"
        );
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": format!(
                    "Missing confirmation header: {}: {}",
                    SENSITIVE_CONFIRM_HEADER,
                    SENSITIVE_CONFIRM_VALUE
                )
            })),
        )
            .into_response());
    }

    if rate_limit {
        let mut last_call = state.sensitive_last_call.lock().expect("mutex poisoned");
        let now = Instant::now();
        if let Some(previous) = *last_call {
            if now.duration_since(previous) < SENSITIVE_MIN_INTERVAL {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "error": "Sensitive endpoint rate limit exceeded"
                    })),
                )
                    .into_response());
            }
        }
        *last_call = Some(now);
    }

    Ok(())
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
    headers: HeaderMap,
    Json(body): Json<InvokeBody>,
) -> impl IntoResponse {
    info!(command = %command, "Test bridge: invoking command");

    if SENSITIVE_COMMANDS.contains(&command.as_str()) {
        let rate_limit = RATE_LIMITED_SENSITIVE_COMMANDS.contains(&command.as_str());
        if let Err(resp) = confirm_sensitive_access(state.as_ref(), &headers, rate_limit) {
            return resp;
        }
    }

    match command.as_str() {
        // Wallet commands
        "bagz_list_wallets" => {
            dispatch::<ListWalletsRequest, ListWalletsResponse>(&state, body.request, |s, req| {
                list_wallets_impl(s, req)
            })
        }
        "bagz_create_wallet" => {
            dispatch::<CreateWalletRequest, CreateWalletResponse>(&state, body.request, |s, req| {
                create_wallet_impl(s, req)
            })
        }
        "bagz_load_wallet" => {
            dispatch::<LoadWalletRequest, LoadWalletResponse>(&state, body.request, |s, req| {
                load_wallet_impl(s, req)
            })
        }
        "bagz_get_wallet_status" => dispatch::<GetWalletStatusRequest, GetWalletStatusResponse>(
            &state,
            body.request,
            get_wallet_status_impl,
        ),
        "bagz_unlock_wallet" => {
            dispatch::<UnlockWalletRequest, UnlockWalletResponse>(&state, body.request, |s, req| {
                unlock_wallet_impl(s, req)
            })
        }
        "bagz_lock_wallet" => {
            dispatch::<LockWalletRequest, LockWalletResponse>(&state, body.request, |s, req| {
                lock_wallet_impl(s, req)
            })
        }
        "bagz_reauth_wallet" => {
            dispatch::<ReauthWalletRequest, ReauthWalletResponse>(&state, body.request, |s, req| {
                reauth_wallet_impl(s, req)
            })
        }
        "bagz_view_seed_phrase" => dispatch::<ViewSeedPhraseRequest, ViewSeedPhraseResponse>(
            &state,
            body.request,
            view_seed_phrase_impl,
        ),
        "bagz_logout_wallet" => {
            dispatch::<LogoutWalletRequest, LogoutWalletResponse>(&state, body.request, |s, req| {
                logout_wallet_impl(s, req)
            })
        }
        // Balance commands
        "bagz_get_balance" => {
            dispatch::<GetBalanceRequest, GetBalanceResponse>(&state, body.request, |s, req| {
                get_balance_impl(s, req)
            })
        }
        // Address commands
        "bagz_get_receive_address" => dispatch::<
            GetReceiveAddressRequest,
            GetReceiveAddressResponse,
        >(&state, body.request, |s, req| {
            get_receive_address_impl(s, req)
        }),
        // Backup commands
        "bagz_get_backup_challenge" => dispatch::<
            GetBackupChallengeRequest,
            GetBackupChallengeResponse,
        >(&state, body.request, |s, req| {
            get_backup_challenge_impl(s, req)
        }),
        "bagz_verify_backup" => {
            dispatch::<VerifyBackupRequest, VerifyBackupResponse>(&state, body.request, |s, req| {
                verify_backup_impl(s, req)
            })
        }
        "bagz_restore_wallet" => dispatch::<RestoreWalletRequest, RestoreWalletResponse>(
            &state,
            body.request,
            restore_wallet_impl,
        ),
        // Transactions commands
        "bagz_prepare_send" => {
            dispatch::<PrepareSendRequest, PrepareSendResponse>(&state, body.request, |s, req| {
                prepare_send_impl(s, req)
            })
        }
        "bagz_confirm_send" => {
            dispatch::<ConfirmSendRequest, ConfirmSendResponse>(&state, body.request, |s, req| {
                confirm_send_impl(s, req)
            })
        }
        "bagz_cancel_send" => {
            dispatch::<CancelSendRequest, CancelSendResponse>(&state, body.request, |s, req| {
                cancel_send_impl(s, req)
            })
        }
        "bagz_retry_broadcast" => dispatch::<RetryBroadcastRequest, RetryBroadcastResponse>(
            &state,
            body.request,
            retry_broadcast_impl,
        ),
        "bagz_list_transactions" => {
            dispatch::<ListTransactionsRequest, ListTransactionsResponse>(
                &state,
                body.request,
                list_transactions_impl,
            )
        }
        "bagz_shield_funds" => {
            dispatch::<ShieldFundsRequest, ShieldFundsResponse>(&state, body.request, |s, req| {
                shield_funds_impl(s, req)
            })
        }
        // Keystone commands
        "bagz_import_ufvk" => {
            dispatch::<ImportUfvkRequest, ImportUfvkResponse>(&state, body.request, |s, req| {
                import_ufvk_impl(s, req)
            })
        }
        "bagz_build_signing_request" => dispatch::<
            BuildSigningRequestRequest,
            BuildSigningRequestResponse,
        >(&state, body.request, |s, req| {
            build_signing_request_impl(s, req)
        }),
        "bagz_finalize_signing" => dispatch::<FinalizeSigningRequest, FinalizeSigningResponse>(
            &state,
            body.request,
            finalize_signing_impl,
        ),
        "bagz_create_keystone_wallet" => dispatch::<
            CreateKeystoneWalletRequest,
            CreateKeystoneWalletResponse,
        >(&state, body.request, |s, req| {
            create_keystone_wallet_impl(s, req)
        }),
        // Swap commands
        "bagz_request_swap_quote" => {
            dispatch::<RequestSwapQuoteRequest, RequestSwapQuoteResponse>(
                &state,
                body.request,
                request_swap_quote_impl,
            )
        }
        "bagz_start_swap" => {
            dispatch::<StartSwapRequest, StartSwapResponse>(&state, body.request, |s, req| {
                start_swap_impl(s, req)
            })
        }
        "bagz_get_swap_status" => dispatch::<GetSwapStatusRequest, GetSwapStatusResponse>(
            &state,
            body.request,
            get_swap_status_impl,
        ),
        "bagz_list_swaps" => {
            dispatch::<ListSwapsRequest, ListSwapsResponse>(&state, body.request, |s, req| {
                list_swaps_impl(s, req)
            })
        }
        // Tor commands
        "bagz_set_tor_enabled" => dispatch::<SetTorEnabledRequest, SetTorEnabledResponse>(
            &state,
            body.request,
            set_tor_enabled_impl,
        ),
        "bagz_get_tor_state" => {
            dispatch::<GetTorStateRequest, GetTorStateResponse>(&state, body.request, |s, req| {
                get_tor_state_impl(s, req)
            })
        }
        // Server commands
        "bagz_add_server" => {
            dispatch::<AddServerRequest, AddServerResponse>(&state, body.request, |s, req| {
                add_server_impl(s, req)
            })
        }
        "bagz_set_default_server" => {
            dispatch::<SetDefaultServerRequest, SetDefaultServerResponse>(
                &state,
                body.request,
                set_default_server_impl,
            )
        }
        "bagz_test_server" => {
            dispatch::<TestServerRequest, TestServerResponse>(&state, body.request, |s, req| {
                test_server_impl(s, req)
            })
        }
        "bagz_list_servers" => {
            dispatch::<ListServersRequest, ListServersResponse>(&state, body.request, |s, req| {
                list_servers_impl(s, req)
            })
        }
        // Logs
        "bagz_get_log_location" => dispatch::<GetLogLocationRequest, GetLogLocationResponse>(
            &state,
            body.request,
            get_log_location_impl,
        ),
        // Version
        "bagz_get_version" => {
            dispatch::<GetVersionRequest, GetVersionResponse>(&state, body.request, |s, req| {
                get_version_impl(s, req)
            })
        }
        // Fiat settings/exchange rate
        "bagz_get_fiat_settings" => dispatch::<GetFiatSettingsRequest, GetFiatSettingsResponse>(
            &state,
            body.request,
            get_fiat_settings_impl,
        ),
        "bagz_set_fiat_settings" => dispatch::<SetFiatSettingsRequest, SetFiatSettingsResponse>(
            &state,
            body.request,
            set_fiat_settings_impl,
        ),
        // NOTE: get_exchange_rate_impl is async (fetches rates from external API),
        // so it cannot use the synchronous `dispatch` helper.
        "bagz_get_exchange_rate" => {
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
        "bagz_start_sync" => {
            dispatch::<StartSyncRequest, StartSyncResponse>(&state, body.request, |s, req| {
                start_sync_impl(s, req)
            })
        }
        "bagz_stop_sync" => {
            dispatch::<StopSyncRequest, StopSyncResponse>(&state, body.request, |s, req| {
                stop_sync_impl(s, req)
            })
        }
        "bagz_get_sync_progress" => dispatch::<GetSyncProgressRequest, GetSyncProgressResponse>(
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
