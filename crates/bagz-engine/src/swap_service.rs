use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::Context as _;
use rusqlite::Connection;
use tokio::sync::watch;
use uuid::Uuid;

use bagz_core::domain::{
    AccountType, Network, SwapInfo, SwapIntent, SwapMode, SwapQuote, SwapState, SwapType,
};
use bagz_core::errors;
use bagz_core::ipc::v1::commands::swap::{
    GetSwapStatusResponse, ListSwapsResponse, RefreshSwapStatusResponse, RequestSwapQuoteResponse,
    ResumePendingSwapsResponse, StartSwapResponse,
};
use bagz_core::ipc::v1::common::SCHEMA_VERSION;
use bagz_core::ipc::v1::events::SwapChangedEvent;
use bagz_network::near_intents::AppFee;

use crate::db::{account_meta, open_app_db_connection, swap_meta};
use crate::error::ipc_err;
use crate::reauth::SystemClock;
use crate::tokio_runtime::block_on;
use crate::tx_service::TxService;
use crate::wallet_manager::WalletManager;

pub type SwapEventHandler = Arc<dyn Fn(SwapChangedEvent) + Send + Sync>;
const TOKEN_DECIMALS_CACHE_TTL_MS: i64 = 5 * 60 * 1000;

const NEAR_STATUS_TIMEOUT_SECS: u64 = 15;

#[derive(Clone)]
pub struct SwapService {
    app_db_path: PathBuf,
    near: bagz_network::near_intents::NearIntentsClient,
    state: Arc<Mutex<State>>,
    wallet_manager: Arc<Mutex<WalletManager>>,
    tx_service: Arc<Mutex<TxService<SystemClock>>>,
}

#[derive(Debug)]
struct State {
    quotes: HashMap<String, QuoteRecord>,
    jobs: HashMap<Uuid, SwapJob>,
    refresh_inflight: HashSet<Uuid>,
    token_decimals: HashMap<String, u8>,
    token_decimals_updated_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct QuoteRecord {
    wallet_id: Uuid,
    intent: SwapIntent,
    quote: SwapQuote,
}

#[derive(Debug)]
#[allow(dead_code)]
struct SwapJob {
    cancel: watch::Sender<bool>,
}

#[derive(Debug)]
struct SwapJobGuard {
    state: Arc<Mutex<State>>,
    swap_id: Uuid,
}

#[derive(Debug)]
struct RefreshInFlightGuard {
    state: Arc<Mutex<State>>,
    swap_id: Uuid,
}

impl Drop for RefreshInFlightGuard {
    fn drop(&mut self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.refresh_inflight.remove(&self.swap_id);
    }
}

impl Drop for SwapJobGuard {
    fn drop(&mut self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.jobs.remove(&self.swap_id);
    }
}

fn load_owned_swap(conn: &Connection, wallet_id: Uuid, swap_id: Uuid) -> anyhow::Result<SwapInfo> {
    let Some((owner_wallet_id, swap)) =
        swap_meta::get_swap(conn, swap_id).context("failed to load swap")?
    else {
        return Err(ipc_err(errors::SWAP_FAILED, "swap not found"));
    };

    // Intentionally return "swap not found" on wallet mismatch to avoid leaking swap existence.
    if owner_wallet_id != wallet_id {
        return Err(ipc_err(errors::SWAP_FAILED, "swap not found"));
    }

    Ok(swap)
}

fn next_state_from_remote_status(
    wallet_manager: &Arc<Mutex<WalletManager>>,
    wallet_id: Uuid,
    swap: &SwapInfo,
    remote_status: &bagz_network::near_intents::RemoteStatus,
) -> SwapState {
    let mapped = bagz_network::near_intents::map_remote_status_to_local_state(remote_status);
    if mapped == SwapState::Confirming && has_confirmed_zcash_tx(wallet_manager, wallet_id, swap) {
        SwapState::Completed
    } else {
        mapped
    }
}

/// Persist swap state to DB and emit change event.
///
/// Logs errors instead of propagating them, suitable for use in async polling loops.
fn try_persist_and_emit_swap_change(
    app_db_path: &std::path::Path,
    wallet_id: Uuid,
    swap: &SwapInfo,
    wallet_manager: &Arc<Mutex<WalletManager>>,
    on_swap_changed: Option<&SwapEventHandler>,
) {
    match open_app_db_connection(app_db_path) {
        Ok(conn) => {
            if let Err(db_err) = swap_meta::update_swap(&conn, wallet_id, swap) {
                tracing::error!(
                    wallet_id = %wallet_id,
                    swap_id = %swap.id,
                    error = ?db_err,
                    "CRITICAL: failed to persist swap state - swap may be inconsistent after restart"
                );
            }
        }
        Err(db_err) => {
            tracing::error!(
                wallet_id = %wallet_id,
                swap_id = %swap.id,
                error = ?db_err,
                "CRITICAL: failed to open app DB for swap state update"
            );
        }
    }

    if let Some(handler) = on_swap_changed
        && is_active_wallet(wallet_manager, wallet_id)
    {
        handler(SwapChangedEvent {
            schema_version: SCHEMA_VERSION,
            event: "swap.changed".to_string(),
            swap: swap.clone(),
        });
    }
}

impl SwapService {
    fn lock_wallet_then_tx_service(
        &self,
    ) -> anyhow::Result<(
        MutexGuard<'_, WalletManager>,
        MutexGuard<'_, TxService<SystemClock>>,
    )> {
        let mgr = self
            .wallet_manager
            .lock()
            .map_err(|_| ipc_err(errors::WALLET_LOCKED, "wallet locked"))?;
        let tx_svc = self
            .tx_service
            .lock()
            .map_err(|_| ipc_err(errors::WALLET_LOCKED, "tx service locked"))?;
        Ok((mgr, tx_svc))
    }

    pub fn new(
        app_db_path: PathBuf,
        wallet_manager: Arc<Mutex<WalletManager>>,
        tx_service: Arc<Mutex<TxService<SystemClock>>>,
    ) -> anyhow::Result<Self> {
        Self::new_with_near_client_and_tx(
            app_db_path,
            wallet_manager,
            tx_service,
            bagz_network::near_intents::NearIntentsClient::new()?,
        )
    }

    pub fn new_with_near_client(
        app_db_path: PathBuf,
        wallet_manager: Arc<Mutex<WalletManager>>,
        near: bagz_network::near_intents::NearIntentsClient,
    ) -> anyhow::Result<Self> {
        let tx_service = Arc::new(Mutex::new(TxService::new(SystemClock)));
        Self::new_with_near_client_and_tx(app_db_path, wallet_manager, tx_service, near)
    }

    pub fn new_with_near_client_and_tx(
        app_db_path: PathBuf,
        wallet_manager: Arc<Mutex<WalletManager>>,
        tx_service: Arc<Mutex<TxService<SystemClock>>>,
        near: bagz_network::near_intents::NearIntentsClient,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            app_db_path,
            near,
            state: Arc::new(Mutex::new(State {
                quotes: HashMap::new(),
                jobs: HashMap::new(),
                refresh_inflight: HashSet::new(),
                token_decimals: HashMap::new(),
                token_decimals_updated_ms: None,
            })),
            wallet_manager,
            tx_service,
        })
    }

    fn try_acquire_refresh_inflight_guard(&self, swap_id: Uuid) -> Option<RefreshInFlightGuard> {
        let state = Arc::clone(&self.state);
        let mut lock = state.lock().expect("mutex poisoned");
        if !lock.refresh_inflight.insert(swap_id) {
            return None;
        }
        drop(lock);

        Some(RefreshInFlightGuard { state, swap_id })
    }

    fn resolve_asset_decimals(&self, asset_id: &str) -> anyhow::Result<u8> {
        let asset_id = asset_id.trim();
        if asset_id.is_empty() {
            return Err(ipc_err(errors::INVALID_ASSET, "invalid asset"));
        }
        if let Some(decimals) = get_static_decimals_for_asset(asset_id) {
            return Ok(decimals);
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let (cached_decimals, cache_is_fresh) = {
            let state = self.state.lock().expect("mutex poisoned");
            let cached = state.token_decimals.get(asset_id).copied();
            let fresh = state.token_decimals_updated_ms.is_some_and(|updated_ms| {
                now_ms.saturating_sub(updated_ms) <= TOKEN_DECIMALS_CACHE_TTL_MS
            });
            (cached, fresh)
        };

        if cache_is_fresh {
            if let Some(decimals) = cached_decimals {
                return Ok(decimals);
            }
            if let Some(decimals) = get_static_decimals_for_asset(asset_id) {
                tracing::warn!(
                    asset_id,
                    decimals,
                    "asset missing in fresh token cache; using static decimals fallback"
                );
                return Ok(decimals);
            }
            return Err(ipc_err(
                errors::INVALID_ASSET,
                format!("unsupported asset: {asset_id}"),
            ));
        }

        let tokens = match block_on(async { self.near.get_supported_tokens().await }) {
            Ok(tokens) => tokens,
            Err(err) => {
                if let Some(decimals) = cached_decimals {
                    tracing::warn!(
                        asset_id,
                        decimals,
                        error = ?err,
                        "failed to refresh token decimals; using stale cached value"
                    );
                    return Ok(decimals);
                }
                if let Some(decimals) = get_static_decimals_for_asset(asset_id) {
                    tracing::warn!(
                        asset_id,
                        decimals,
                        error = ?err,
                        "failed to refresh token decimals; using static decimals fallback"
                    );
                    return Ok(decimals);
                }
                return Err(match err {
                    bagz_network::near_intents::NearIntentsError::TorNotReady => {
                        ipc_err(errors::TOR_NOT_READY, "Tor is enabled but not ready")
                    }
                    _ => ipc_err(
                        errors::SWAP_FAILED,
                        format!("failed to fetch supported tokens: {err}"),
                    ),
                });
            }
        };

        let mut refreshed_decimals = HashMap::with_capacity(tokens.len());
        for token in tokens {
            refreshed_decimals.insert(token.asset_id, token.decimals);
        }

        let resolved = refreshed_decimals.get(asset_id).copied();
        {
            let mut state = self.state.lock().expect("mutex poisoned");
            state.token_decimals = refreshed_decimals;
            state.token_decimals_updated_ms = Some(now_ms);
        }

        if let Some(decimals) = resolved {
            return Ok(decimals);
        }
        if let Some(decimals) = get_static_decimals_for_asset(asset_id) {
            tracing::warn!(
                asset_id,
                decimals,
                "asset missing from provider token list; using static decimals fallback"
            );
            return Ok(decimals);
        }

        Err(ipc_err(
            errors::INVALID_ASSET,
            format!("unsupported asset: {asset_id}"),
        ))
    }

    pub fn request_swap_quote(
        &self,
        wallet_id: Uuid,
        network: Network,
        intent: SwapIntent,
    ) -> anyhow::Result<RequestSwapQuoteResponse> {
        if network == Network::Testnet {
            return Err(ipc_err(
                errors::SWAP_UNSUPPORTED_NETWORK,
                "swaps are not supported on Testnet",
            ));
        }

        if intent.input_asset.trim().is_empty() || intent.output_asset.trim().is_empty() {
            return Err(ipc_err(errors::INVALID_ASSET, "invalid asset"));
        }

        // Validate amount based on swap mode
        let (amount_smallest, swap_type_str, decimals) = match intent.swap_mode {
            SwapMode::ExactInput => {
                if intent.input_amount.trim().is_empty() {
                    return Err(ipc_err(
                        errors::INVALID_REQUEST,
                        "missing input_amount for ExactInput mode",
                    ));
                }
                let decimals = self.resolve_asset_decimals(&intent.input_asset)?;
                let amount =
                    convert_to_smallest_units(&intent.input_amount, decimals).map_err(|e| {
                        ipc_err(errors::INVALID_REQUEST, format!("invalid amount: {e}"))
                    })?;
                (amount, "EXACT_INPUT", decimals)
            }
            SwapMode::ExactOutput => {
                let output_amount = intent
                    .output_amount
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        ipc_err(
                            errors::INVALID_REQUEST,
                            "missing output_amount for ExactOutput/CrossPay mode",
                        )
                    })?;
                let decimals = self.resolve_asset_decimals(&intent.output_asset)?;
                let amount = convert_to_smallest_units(output_amount, decimals).map_err(|e| {
                    ipc_err(errors::INVALID_REQUEST, format!("invalid amount: {e}"))
                })?;
                (amount, "EXACT_OUTPUT", decimals)
            }
        };

        // For ToZec: recipient is the destination Zcash address, refund goes back to origin chain
        // For FromZec: recipient is the destination chain address, refund goes back to Zcash
        let recipient = intent
            .destination_address
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "destination_address required"))?
            .to_string();

        // Refund address is required by the new API
        let refund_to = intent
            .refund_address
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "refund_address required"))?
            .to_string();

        // Calculate deadline (2 hours from now, like Zashi)
        let deadline_dt = chrono::Utc::now() + chrono::Duration::hours(2);
        let deadline = deadline_dt.to_rfc3339();
        let deadline_ms_fallback = deadline_dt.timestamp_millis();

        // Development phase: disable app fees until quote reliability is stable.
        // Set APP_FEE_BPS > 0 to re-enable affiliate fee collection.
        const APP_FEE_BPS: u32 = 0;
        const AFFILIATE_RECIPIENT: &str = "bagz.near";

        let app_fees = if APP_FEE_BPS == 0 {
            None
        } else {
            Some(vec![AppFee {
                recipient: AFFILIATE_RECIPIENT.to_string(),
                fee: APP_FEE_BPS,
            }])
        };

        tracing::debug!(
            swap_mode = ?intent.swap_mode,
            swap_type = swap_type_str,
            amount = %amount_smallest,
            decimals,
            "requesting swap quote"
        );

        // Use dry=false to get deposit address directly (like Zashi)
        let req = bagz_network::near_intents::QuoteRequest {
            origin_asset: intent.input_asset.clone(),
            destination_asset: intent.output_asset.clone(),
            amount: amount_smallest,
            swap_type: swap_type_str.to_string(),
            slippage_tolerance: 100, // 1%
            quote_waiting_time_ms: Some(3000),
            referral: Some("bagz".to_string()),
            app_fees,
            deposit_type: "ORIGIN_CHAIN".to_string(),
            refund_to,
            refund_type: "ORIGIN_CHAIN".to_string(),
            recipient,
            recipient_type: "DESTINATION_CHAIN".to_string(),
            deadline,
            dry: false, // Get deposit address directly
        };

        let map_quote_error = |e| match e {
            bagz_network::near_intents::NearIntentsError::TorNotReady => {
                ipc_err(errors::TOR_NOT_READY, "Tor is enabled but not ready")
            }
            _ => ipc_err(errors::SWAP_FAILED, format!("failed to fetch quote: {e}")),
        };

        let app_fee_bps = if APP_FEE_BPS == 0 {
            None
        } else {
            Some(APP_FEE_BPS)
        };
        let quote_res =
            block_on(async { self.near.get_quote(req).await }).map_err(map_quote_error)?;

        let quote = SwapQuote {
            input_asset: intent.input_asset.clone(),
            input_amount: quote_res.amount_in.clone(),
            input_amount_formatted: quote_res.amount_in_formatted.clone(),
            output_asset: intent.output_asset.clone(),
            output_amount: quote_res.amount_out.clone(),
            output_amount_formatted: quote_res.amount_out_formatted.clone(),
            min_output_amount: quote_res.min_amount_out.clone(),
            deadline: quote_res
                .deadline_ms
                .filter(|ms| *ms > 0)
                .unwrap_or(deadline_ms_fallback),
            time_estimate_secs: quote_res.time_estimate_secs,
            deposit_address: quote_res.deposit_address.clone(),
            deposit_memo: quote_res.deposit_memo.clone(),
            correlation_id: quote_res.correlation_id.clone(),
            app_fee_bps,
        };

        // Use correlation_id as the quote_id for tracking
        let quote_id = quote_res.correlation_id;

        {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let mut state = self.state.lock().expect("mutex poisoned");
            state
                .quotes
                .retain(|_, record| record.quote.deadline == 0 || now_ms < record.quote.deadline);
            state.quotes.insert(
                quote_id.clone(),
                QuoteRecord {
                    wallet_id,
                    intent,
                    quote: quote.clone(),
                },
            );
        }

        Ok(RequestSwapQuoteResponse {
            schema_version: SCHEMA_VERSION,
            quote_id,
            quote,
        })
    }

    pub fn start_swap(
        &self,
        wallet_id: Uuid,
        network: Network,
        quote_id: &str,
        allow_transparent_interaction: bool,
        reauth_token: Option<&str>,
        on_swap_changed: Option<SwapEventHandler>,
    ) -> anyhow::Result<StartSwapResponse> {
        if network == Network::Testnet {
            return Err(ipc_err(
                errors::SWAP_UNSUPPORTED_NETWORK,
                "swaps are not supported on Testnet",
            ));
        }

        if !allow_transparent_interaction {
            // For v1: fail-closed when transparent interaction is required. ToZec does not
            // require it, but we preserve the contract behavior for future FromZec flows.
        }

        let record = {
            let mut state = self.state.lock().expect("mutex poisoned");
            let Some(record) = state.quotes.get(quote_id).cloned() else {
                return Err(ipc_err(errors::QUOTE_EXPIRED, "quote not found"));
            };
            if record.wallet_id != wallet_id {
                return Err(ipc_err(errors::QUOTE_EXPIRED, "quote not found"));
            }

            let now_ms = chrono::Utc::now().timestamp_millis();
            if record.quote.deadline > 0 && now_ms >= record.quote.deadline {
                state.quotes.remove(quote_id);
                return Err(ipc_err(errors::QUOTE_EXPIRED, "quote expired"));
            }
            record
        };

        if record.intent.swap_type == SwapType::FromZec {
            return self.start_swap_from_zec(
                wallet_id,
                network,
                quote_id,
                allow_transparent_interaction,
                reauth_token,
                on_swap_changed,
                record,
            );
        }

        // With the new API, deposit_address is already in the quote (from dry=false)
        let deposit_address = record
            .quote
            .deposit_address
            .clone()
            .ok_or_else(|| ipc_err(errors::SWAP_FAILED, "quote missing deposit address"))?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let swap = SwapInfo {
            id: Uuid::new_v4(),
            remote_id: Some(record.quote.correlation_id.clone()),
            swap_type: record.intent.swap_type,
            input_asset: record.intent.input_asset,
            input_amount: record.quote.input_amount_formatted.clone(),
            output_asset: record.intent.output_asset,
            output_amount: Some(record.quote.output_amount_formatted.clone()),
            deposit_address: Some(deposit_address),
            deposit_memo: record.quote.deposit_memo.clone(),
            destination_address: record.intent.destination_address,
            refund_address: record.intent.refund_address,
            state: SwapState::AwaitingDeposit,
            deadline: Some(record.quote.deadline),
            last_error: None,
            created_at: now_ms,
            updated_at: now_ms,
        };

        let conn = open_app_db_connection(&self.app_db_path)?;
        swap_meta::insert_swap(&conn, wallet_id, &swap).context("failed to insert swap")?;

        if let Some(handler) = on_swap_changed.as_ref() {
            handler(SwapChangedEvent {
                schema_version: SCHEMA_VERSION,
                event: "swap.changed".to_string(),
                swap: swap.clone(),
            });
        }

        self.start_polling(wallet_id, swap.clone(), on_swap_changed);

        Ok(StartSwapResponse {
            schema_version: SCHEMA_VERSION,
            swap,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn start_swap_from_zec(
        &self,
        wallet_id: Uuid,
        _network: Network,
        _quote_id: &str,
        allow_transparent_interaction: bool,
        reauth_token: Option<&str>,
        on_swap_changed: Option<SwapEventHandler>,
        record: QuoteRecord,
    ) -> anyhow::Result<StartSwapResponse> {
        if !allow_transparent_interaction {
            return Err(ipc_err(
                errors::PRIVACY_ACK_REQUIRED,
                "swap-from-ZEC requires transparent interaction acknowledgement",
            ));
        }

        let reauth_token = reauth_token
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "reauth_token required"))?;

        // With the new API, deposit_address is already in the quote (from dry=false)
        let deposit_address = record
            .quote
            .deposit_address
            .clone()
            .ok_or_else(|| ipc_err(errors::SWAP_FAILED, "quote missing deposit address"))?;

        let conn = open_app_db_connection(&self.app_db_path)?;
        let accounts =
            account_meta::list_accounts(&conn, wallet_id).context("failed to list accounts")?;
        let account_id = accounts
            .iter()
            .find(|a| a.account_type == AccountType::Software)
            .map(|a| a.id)
            .ok_or_else(|| {
                ipc_err(
                    errors::WATCH_ONLY_CANNOT_SPEND,
                    "no spend-capable account available",
                )
            })?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let raw_input_amount = record.quote.input_amount.clone();
        let mut swap = SwapInfo {
            id: Uuid::new_v4(),
            remote_id: Some(record.quote.correlation_id.clone()),
            swap_type: record.intent.swap_type,
            input_asset: record.intent.input_asset,
            input_amount: record.quote.input_amount_formatted.clone(),
            output_asset: record.intent.output_asset,
            output_amount: Some(record.quote.output_amount_formatted.clone()),
            deposit_address: Some(deposit_address.clone()),
            deposit_memo: record.quote.deposit_memo.clone(),
            destination_address: record.intent.destination_address,
            refund_address: record.intent.refund_address,
            state: SwapState::Draft,
            deadline: Some(record.quote.deadline),
            last_error: None,
            created_at: now_ms,
            updated_at: now_ms,
        };

        swap_meta::insert_swap(&conn, wallet_id, &swap).context("failed to insert swap")?;

        if let Some(handler) = on_swap_changed.as_ref() {
            handler(SwapChangedEvent {
                schema_version: SCHEMA_VERSION,
                event: "swap.changed".to_string(),
                swap: swap.clone(),
            });
        }

        // Send ZEC to the deposit address
        let send_result: anyhow::Result<_> = (|| {
            let task = {
                let (mut mgr, mut tx_svc) = self.lock_wallet_then_tx_service()?;

                let proposal = mgr.prepare_send(
                    account_id,
                    &deposit_address,
                    // Wallet send APIs require zatoshis (integer), not formatted units.
                    &raw_input_amount,
                    swap.deposit_memo.as_deref(),
                    allow_transparent_interaction,
                    &mut tx_svc,
                )?;

                mgr.prepare_confirm_send_task(&proposal.proposal_id, reauth_token, &mut tx_svc)?
            };

            WalletManager::execute_prepared_confirm_send_task(task, None)
        })();

        let txid = match send_result {
            Ok(res) => res.txid,
            Err(err) => {
                swap.state = SwapState::Failed;
                swap.updated_at = chrono::Utc::now().timestamp_millis();
                swap.last_error = Some(err.to_string());
                if let Err(db_err) = swap_meta::update_swap(&conn, wallet_id, &swap) {
                    tracing::error!(
                        wallet_id = %wallet_id,
                        swap_id = %swap.id,
                        error = ?db_err,
                        "CRITICAL: failed to persist swap state - swap may be inconsistent after restart"
                    );
                }
                if let Some(handler) = on_swap_changed.as_ref() {
                    handler(SwapChangedEvent {
                        schema_version: SCHEMA_VERSION,
                        event: "swap.changed".to_string(),
                        swap: swap.clone(),
                    });
                }
                return Err(err);
            }
        };

        // Notify the API about the deposit (optional but helps speed up detection)
        let submit_req = bagz_network::near_intents::DepositSubmitRequest {
            tx_hash: txid,
            deposit_address,
        };

        // Best-effort notification - don't fail the swap if this fails
        if let Err(e) = block_on(async { self.near.submit_deposit(submit_req).await }) {
            tracing::warn!(
                wallet_id = %wallet_id,
                swap_id = %swap.id,
                error = ?e,
                "failed to notify API about deposit (non-fatal)"
            );
        }

        swap.state = SwapState::Pending;
        swap.updated_at = chrono::Utc::now().timestamp_millis();

        swap_meta::update_swap(&conn, wallet_id, &swap).context("failed to update swap")?;
        if let Some(handler) = on_swap_changed.as_ref() {
            handler(SwapChangedEvent {
                schema_version: SCHEMA_VERSION,
                event: "swap.changed".to_string(),
                swap: swap.clone(),
            });
        }

        self.start_polling(wallet_id, swap.clone(), on_swap_changed);

        Ok(StartSwapResponse {
            schema_version: SCHEMA_VERSION,
            swap,
        })
    }

    pub fn get_swap_status(
        &self,
        wallet_id: Uuid,
        swap_id: Uuid,
    ) -> anyhow::Result<GetSwapStatusResponse> {
        let conn = open_app_db_connection(&self.app_db_path)?;
        let swap = load_owned_swap(&conn, wallet_id, swap_id)?;

        Ok(GetSwapStatusResponse {
            schema_version: SCHEMA_VERSION,
            swap,
        })
    }

    pub fn list_swaps(&self, wallet_id: Uuid) -> anyhow::Result<ListSwapsResponse> {
        let conn = open_app_db_connection(&self.app_db_path)?;
        let swaps =
            swap_meta::list_swaps_for_wallet(&conn, wallet_id).context("failed to list swaps")?;
        Ok(ListSwapsResponse {
            schema_version: SCHEMA_VERSION,
            swaps,
        })
    }

    /// Actively refresh swap status from the remote API.
    ///
    /// Unlike `get_swap_status` which only reads from the local DB, this method
    /// queries the Near Intents API for the latest status and updates the DB.
    pub fn refresh_swap_status(
        &self,
        wallet_id: Uuid,
        swap_id: Uuid,
        on_swap_changed: Option<SwapEventHandler>,
    ) -> anyhow::Result<RefreshSwapStatusResponse> {
        let conn = open_app_db_connection(&self.app_db_path)?;
        let mut swap = load_owned_swap(&conn, wallet_id, swap_id)?;

        // Only refresh if swap is in a non-terminal state
        if matches!(
            swap.state,
            SwapState::Completed | SwapState::Refunded | SwapState::Failed
        ) {
            return Ok(RefreshSwapStatusResponse {
                schema_version: SCHEMA_VERSION,
                swap,
            });
        }

        let Some(deposit_address) = swap.deposit_address.clone() else {
            tracing::debug!(
                wallet_id = %wallet_id,
                swap_id = %swap_id,
                state = ?swap.state,
                "refresh_swap_status skipped: missing deposit_address"
            );
            return Ok(RefreshSwapStatusResponse {
                schema_version: SCHEMA_VERSION,
                swap,
            });
        };

        let Some(_refresh_guard) = self.try_acquire_refresh_inflight_guard(swap_id) else {
            tracing::debug!(
                wallet_id = %wallet_id,
                swap_id = %swap_id,
                "refresh_swap_status skipped: refresh already in flight"
            );
            return Ok(RefreshSwapStatusResponse {
                schema_version: SCHEMA_VERSION,
                swap,
            });
        };

        // Query remote API for latest status
        let status_res = block_on(async {
            match tokio::time::timeout(
                Duration::from_secs(NEAR_STATUS_TIMEOUT_SECS),
                self.near
                    .get_status(bagz_network::near_intents::StatusRequest {
                        deposit_address,
                        deposit_memo: swap.deposit_memo.clone(),
                    }),
            )
            .await
            {
                Ok(res) => res,
                Err(_) => Err(bagz_network::near_intents::NearIntentsError::Transport(
                    format!("timeout after {NEAR_STATUS_TIMEOUT_SECS}s"),
                )),
            }
        });

        match status_res {
            Ok(status) => {
                let next_state = next_state_from_remote_status(
                    &self.wallet_manager,
                    wallet_id,
                    &swap,
                    &status.status,
                );
                let next_last_error = status.message.clone();

                if next_state != swap.state || next_last_error != swap.last_error {
                    swap.state = next_state;
                    swap.updated_at = chrono::Utc::now().timestamp_millis();
                    swap.last_error = next_last_error;

                    swap_meta::update_swap(&conn, wallet_id, &swap)
                        .context("failed to update swap")?;

                    if let Some(handler) = on_swap_changed.as_ref()
                        && is_active_wallet(&self.wallet_manager, wallet_id)
                    {
                        handler(SwapChangedEvent {
                            schema_version: SCHEMA_VERSION,
                            event: "swap.changed".to_string(),
                            swap: swap.clone(),
                        });
                    }
                }
            }
            Err(e) => {
                swap.last_error = Some(e.to_string());
                swap.updated_at = chrono::Utc::now().timestamp_millis();
                swap_meta::update_swap(&conn, wallet_id, &swap).context("failed to update swap")?;

                if let Some(handler) = on_swap_changed.as_ref()
                    && is_active_wallet(&self.wallet_manager, wallet_id)
                {
                    handler(SwapChangedEvent {
                        schema_version: SCHEMA_VERSION,
                        event: "swap.changed".to_string(),
                        swap: swap.clone(),
                    });
                }
            }
        }

        Ok(RefreshSwapStatusResponse {
            schema_version: SCHEMA_VERSION,
            swap,
        })
    }

    /// Resume polling for all pending (non-terminal) swaps that have a deposit address.
    ///
    /// This should be called on app startup or wallet load to continue
    /// tracking swaps that were in progress when the app was closed.
    pub fn resume_pending_swaps(
        &self,
        wallet_id: Uuid,
        on_swap_changed: Option<SwapEventHandler>,
    ) -> anyhow::Result<ResumePendingSwapsResponse> {
        let conn = open_app_db_connection(&self.app_db_path)?;
        let swaps = swap_meta::list_pollable_swaps_for_wallet(&conn, wallet_id)
            .context("failed to list pollable swaps")?;

        let mut resumed_count = 0;
        for swap in swaps {
            if self.start_polling(wallet_id, swap, on_swap_changed.clone()) {
                resumed_count += 1;
            }
        }

        Ok(ResumePendingSwapsResponse {
            schema_version: SCHEMA_VERSION,
            resumed_count,
        })
    }

    fn start_polling(
        &self,
        wallet_id: Uuid,
        initial_swap: SwapInfo,
        on_swap_changed: Option<SwapEventHandler>,
    ) -> bool {
        let swap_id = initial_swap.id;
        let state = Arc::clone(&self.state);
        let app_db_path = self.app_db_path.clone();
        let near = self.near.clone();
        let wallet_manager = Arc::clone(&self.wallet_manager);

        let (cancel_tx, mut cancel_rx) = watch::channel(false);

        {
            let mut state_guard = state.lock().expect("mutex poisoned");
            if state_guard.jobs.contains_key(&swap_id) {
                return false;
            }
            // Insert the job marker before spawning the task to avoid TOCTOU races where the task
            // could complete and attempt to remove itself before it's recorded.
            state_guard
                .jobs
                .insert(swap_id, SwapJob { cancel: cancel_tx });
        }

        let state_for_task = Arc::clone(&state);
        let spawn_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            crate::tokio_runtime::spawn(async move {
                // Runtime task completion always removes `jobs[swap_id]` via Drop.
                // The outer `catch_unwind` path below only handles panics that happen
                // before the async task is successfully spawned.
                let _job_guard = SwapJobGuard {
                    state: state_for_task,
                    swap_id,
                };

                let mut backoff = Duration::from_secs(5);
                let mut swap = initial_swap;

                loop {
                    // Check before sleeping so we don't wait out a full backoff interval after a
                    // wallet switch.
                    if !is_active_wallet(&wallet_manager, wallet_id) {
                        break;
                    }

                    tokio::select! {
                        _ = cancel_rx.changed() => {
                            break;
                        }
                        _ = tokio::time::sleep(backoff) => {}
                    }

                    if !is_active_wallet(&wallet_manager, wallet_id) {
                        break;
                    }

                    let Some(deposit_address) = swap.deposit_address.clone() else {
                        break;
                    };

                    let status_res = match tokio::time::timeout(
                        Duration::from_secs(NEAR_STATUS_TIMEOUT_SECS),
                        near.get_status(bagz_network::near_intents::StatusRequest {
                            deposit_address,
                            deposit_memo: swap.deposit_memo.clone(),
                        }),
                    )
                    .await
                    {
                        Ok(res) => res,
                        Err(_) => Err(bagz_network::near_intents::NearIntentsError::Transport(
                            format!("timeout after {NEAR_STATUS_TIMEOUT_SECS}s"),
                        )),
                    };

                    match status_res {
                        Ok(status) => {
                            backoff = Duration::from_secs(5);

                            let next_state = next_state_from_remote_status(
                                &wallet_manager,
                                wallet_id,
                                &swap,
                                &status.status,
                            );
                            let next_last_error = status.message.clone();

                            if next_state != swap.state || next_last_error != swap.last_error {
                                swap.state = next_state;
                                swap.updated_at = chrono::Utc::now().timestamp_millis();
                                swap.last_error = next_last_error;

                                try_persist_and_emit_swap_change(
                                    &app_db_path,
                                    wallet_id,
                                    &swap,
                                    &wallet_manager,
                                    on_swap_changed.as_ref(),
                                );
                            }

                            if matches!(
                                swap.state,
                                SwapState::Completed | SwapState::Refunded | SwapState::Failed
                            ) {
                                break;
                            }
                        }
                        Err(bagz_network::near_intents::NearIntentsError::RateLimited {
                            retry_after,
                        }) => {
                            backoff = retry_after
                                .unwrap_or_else(|| backoff.saturating_mul(2))
                                .min(Duration::from_secs(60));
                        }
                        Err(err) => {
                            backoff = backoff.saturating_mul(2).min(Duration::from_secs(60));
                            swap.last_error = Some(err.to_string());
                            swap.updated_at = chrono::Utc::now().timestamp_millis();
                            try_persist_and_emit_swap_change(
                                &app_db_path,
                                wallet_id,
                                &swap,
                                &wallet_manager,
                                on_swap_changed.as_ref(),
                            );
                        }
                    }
                }
            });
        }));

        if spawn_result.is_err() {
            let mut state_guard = match state.lock() {
                Ok(state) => state,
                Err(poisoned) => poisoned.into_inner(),
            };
            state_guard.jobs.remove(&swap_id);
            return false;
        }

        true
    }
}

fn has_confirmed_zcash_tx(
    wallet_manager: &Arc<Mutex<WalletManager>>,
    wallet_id: Uuid,
    swap: &SwapInfo,
) -> bool {
    let ctx = {
        let Ok(mgr) = wallet_manager.lock() else {
            return false;
        };

        let Some(active_wallet) = mgr.active_wallet_info() else {
            return false;
        };
        if active_wallet.id != wallet_id {
            return false;
        }

        match mgr.get_tx_operation_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::debug!(error = ?e, "swap detection check failed");
                return false;
            }
        }
    };

    let conn = match crate::wallet_manager::open_wallet_db_for_tx(&ctx) {
        Ok(conn) => conn,
        Err(e) => {
            tracing::debug!(error = ?e, "swap detection check failed");
            return false;
        }
    };

    let expected_amount_zat = match swap.swap_type {
        SwapType::ToZec => swap.output_amount.as_deref().and_then(parse_zatoshis),
        SwapType::FromZec => parse_zatoshis(&swap.input_amount),
    };
    let min_block_time_s = swap
        .created_at
        .saturating_sub(15 * 60 * 1000)
        .saturating_div(1000);

    let mut stmt = match conn.prepare(
        "SELECT
            mined_height,
            fee_paid,
            total_spent,
            total_received,
            is_shielding,
            sent_note_count,
            received_note_count,
            block_time
         FROM v_transactions
         WHERE mined_height IS NOT NULL
         ORDER BY COALESCE(block_time, 0) DESC, txid DESC
         LIMIT 200",
    ) {
        Ok(stmt) => stmt,
        Err(e) => {
            tracing::debug!(error = ?e, "swap detection check failed");
            return false;
        }
    };

    let mut rows = match stmt.query([]) {
        Ok(rows) => rows,
        Err(e) => {
            tracing::debug!(error = ?e, "swap detection check failed");
            return false;
        }
    };

    while let Ok(Some(row)) = rows.next() {
        let is_shielding: bool = match row.get(4) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = ?e, "skipping swap check due to error");
                continue;
            }
        };
        if is_shielding {
            continue;
        }

        let sent_note_count: i64 = match row.get(5) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = ?e, "skipping swap check due to error");
                continue;
            }
        };
        let received_note_count: i64 = match row.get(6) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = ?e, "skipping swap check due to error");
                continue;
            }
        };

        let block_time: Option<i64> = row.get(7).ok();
        if let Some(bt) = block_time
            && bt < min_block_time_s
        {
            continue;
        }

        match swap.swap_type {
            SwapType::ToZec => {
                if sent_note_count > 0 || received_note_count <= 0 {
                    continue;
                }

                let total_received: i64 = match row.get(3) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(error = ?e, "skipping swap check due to error");
                        continue;
                    }
                };
                let received_u64 = u64::try_from(total_received.max(0)).unwrap_or(0);
                if expected_amount_zat.is_none_or(|expected| received_u64 == expected) {
                    return true;
                }
            }
            SwapType::FromZec => {
                if sent_note_count <= 0 {
                    continue;
                }

                let total_spent: i64 = match row.get(2) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(error = ?e, "skipping swap check due to error");
                        continue;
                    }
                };
                let total_received: i64 = match row.get(3) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(error = ?e, "skipping swap check due to error");
                        continue;
                    }
                };
                let fee_paid: Option<i64> = row.get(1).ok();
                let fee_u64 = fee_paid
                    .and_then(|f| u64::try_from(f.max(0)).ok())
                    .unwrap_or(0);

                let spent_u64 = u64::try_from(total_spent.max(0)).unwrap_or(0);
                let received_u64 = u64::try_from(total_received.max(0)).unwrap_or(0);
                let sent_u64 = spent_u64
                    .saturating_sub(received_u64)
                    .saturating_sub(fee_u64);

                if expected_amount_zat.is_none_or(|expected| sent_u64 == expected) {
                    return true;
                }
            }
        }
    }

    false
}

fn is_active_wallet(wallet_manager: &Arc<Mutex<WalletManager>>, wallet_id: Uuid) -> bool {
    let Ok(mgr) = wallet_manager.lock() else {
        return false;
    };
    mgr.is_active_wallet_unlocked(wallet_id)
}

fn parse_zatoshis(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Some((whole, frac)) = value.split_once('.') {
        let whole: u64 = whole.parse().ok()?;
        if frac.len() > 8 {
            return None;
        }
        let mut frac_buf = frac.as_bytes().to_vec();
        while frac_buf.len() < 8 {
            frac_buf.push(b'0');
        }
        let frac_str = std::str::from_utf8(&frac_buf).ok()?;
        let frac_val: u64 = frac_str.parse().ok()?;
        whole.checked_mul(100_000_000)?.checked_add(frac_val)
    } else {
        value.parse::<u64>().ok()
    }
}

#[allow(dead_code)] // May be used for future FromZec refund address derivation
fn derive_ephemeral_transparent_address(
    wallet_db_conn: &mut rusqlite::Connection,
    network: Network,
    account_id: u32,
) -> anyhow::Result<String> {
    #[allow(deprecated)]
    use zcash_client_backend::data_api::{WalletRead as _, WalletWrite as _};
    #[allow(deprecated)]
    use zcash_client_backend::keys::UnifiedAddressRequest;
    use zcash_protocol::consensus::Parameters as _;

    let params = zcash_consensus_network(network);
    let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
        wallet_db_conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    let account_uuid = crate::address_service::find_account_uuid(&mut wdb, account_id)
        .context("account not found")?;

    // Force derivation of a new address index that permits a transparent receiver, then return the
    // newest transparent receiver as an "ephemeral" address for one-off flows.
    let _ = wdb
        .get_next_available_address(account_uuid, UnifiedAddressRequest::ALLOW_ALL)
        .context("failed to derive address with transparent receiver")?;

    let receivers = wdb
        .get_transparent_receivers(account_uuid, false, false)
        .context("failed to list transparent receivers")?;

    let Some((addr, _meta)) = receivers
        .into_iter()
        .max_by_key(|(_addr, meta)| meta.address_index().map(|i| i.index()).unwrap_or(0))
    else {
        return Err(ipc_err(
            errors::INTERNAL_ERROR,
            "no transparent receiver available",
        ));
    };

    Ok(addr.to_zcash_address(params.network_type()).encode())
}

#[allow(dead_code)] // Used by derive_ephemeral_transparent_address
fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}

/// Get static decimals for a known asset ID.
///
/// Asset IDs use the new 1Click API format (e.g., `nep141:zec.omft.near`).
fn get_static_decimals_for_asset(asset_id: &str) -> Option<u8> {
    match asset_id {
        // ZEC (8 decimals)
        "nep141:zec.omft.near" => Some(8),
        // ETH and ETH variants (18 decimals)
        "nep141:eth.omft.near" | "nep141:base.omft.near" => Some(18),
        // SOL (9 decimals)
        "nep141:sol.omft.near" => Some(9),
        // NEAR (24 decimals)
        "nep141:wrap.near" => Some(24),
        // USDC/USDT variants (6 decimals)
        s if s.contains("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") => Some(6), // ETH USDC
        s if s.contains("0xdac17f958d2ee523a2206206994597c13d831ec7") => Some(6), // ETH USDT
        s if s.contains("0x833589fcd6edb6e08f4c7c32d4f71b54bda02913") => Some(6), // Base USDC
        s if s.contains("5ce3bf3a31af18be40ba30f721101b4341690186") => Some(6),   // SOL USDC
        s if s.contains("c800a4bd850783ccb82c2b2c7e84175443606352") => Some(6),   // SOL USDT
        s if s.contains("17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1") => {
            Some(6) // NEAR USDC
        }
        _ => None,
    }
}

/// Convert a human-readable amount to smallest units.
///
/// For example, "1.5" ETH with 18 decimals becomes "1500000000000000000".
fn convert_to_smallest_units(amount: &str, decimals: u8) -> anyhow::Result<String> {
    let amount = amount.trim();
    if amount.is_empty() {
        anyhow::bail!("empty amount");
    }

    let parts: Vec<&str> = amount.split('.').collect();
    if parts.len() > 2 {
        anyhow::bail!("invalid amount format");
    }

    let whole_str = parts[0];
    if !whole_str.chars().all(|c| c.is_ascii_digit()) {
        anyhow::bail!("invalid whole part");
    }
    let whole: u128 = whole_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid whole part"))?;

    let frac_str = if parts.len() > 1 { parts[1] } else { "" };
    if !frac_str.is_empty() && !frac_str.chars().all(|c| c.is_ascii_digit()) {
        anyhow::bail!("invalid fractional part");
    }

    // Multiply whole part by 10^decimals
    let mut result = whole;
    for _ in 0..decimals {
        result = result
            .checked_mul(10)
            .ok_or_else(|| anyhow::anyhow!("amount overflow"))?;
    }

    // Add fractional part (truncate if too many decimals)
    if !frac_str.is_empty() {
        let frac_len = frac_str.len().min(decimals as usize);
        let frac_val: u128 = frac_str[..frac_len]
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid fractional part"))?;
        let frac_multiplier = 10u128.pow((decimals as u32).saturating_sub(frac_len as u32));
        result = result
            .checked_add(frac_val * frac_multiplier)
            .ok_or_else(|| anyhow::anyhow!("amount overflow"))?;
    }

    if result == 0 {
        anyhow::bail!("amount must be greater than zero");
    }

    Ok(result.to_string())
}
