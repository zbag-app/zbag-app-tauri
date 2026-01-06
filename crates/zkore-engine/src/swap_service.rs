use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;
use rusqlite::OpenFlags;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use uuid::Uuid;

use zkore_core::domain::{
    AccountType, Network, SwapInfo, SwapIntent, SwapQuote, SwapState, SwapType,
};
use zkore_core::errors;
use zkore_core::ipc::v1::commands::swap::{
    GetSwapStatusResponse, ListSwapsResponse, RequestSwapQuoteResponse, StartSwapResponse,
};
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::events::SwapChangedEvent;

use crate::db::{account_meta, swap_meta};
use crate::error::ipc_err;
use crate::wallet_manager::WalletManager;

pub type SwapEventHandler = Arc<dyn Fn(SwapChangedEvent) + Send + Sync>;

#[derive(Clone)]
pub struct SwapService {
    app_db_path: PathBuf,
    near: zkore_network::near_intents::NearIntentsClient,
    state: Arc<Mutex<State>>,
    wallet_manager: Arc<Mutex<WalletManager>>,
}

#[derive(Debug)]
struct State {
    quotes: HashMap<String, QuoteRecord>,
    jobs: HashMap<Uuid, SwapJob>,
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
    handle: JoinHandle<()>,
}

impl SwapService {
    pub fn new(
        app_db_path: PathBuf,
        wallet_manager: Arc<Mutex<WalletManager>>,
    ) -> anyhow::Result<Self> {
        Self::new_with_near_client(
            app_db_path,
            wallet_manager,
            zkore_network::near_intents::NearIntentsClient::new()?,
        )
    }

    pub fn new_with_near_client(
        app_db_path: PathBuf,
        wallet_manager: Arc<Mutex<WalletManager>>,
        near: zkore_network::near_intents::NearIntentsClient,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            app_db_path,
            near,
            state: Arc::new(Mutex::new(State {
                quotes: HashMap::new(),
                jobs: HashMap::new(),
            })),
            wallet_manager,
        })
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
        if intent.input_amount.trim().is_empty() {
            return Err(ipc_err(errors::INVALID_REQUEST, "missing input_amount"));
        }

        let req = zkore_network::near_intents::QuoteRequest {
            input_asset: intent.input_asset.clone(),
            input_amount: intent.input_amount.clone(),
            output_asset: intent.output_asset.clone(),
        };

        let quote_res =
            block_on(async { self.near.get_quote(req).await }).map_err(|e| match e {
                zkore_network::near_intents::NearIntentsError::TorNotReady => {
                    ipc_err(errors::TOR_NOT_READY, "Tor is enabled but not ready")
                }
                _ => ipc_err(errors::SWAP_FAILED, format!("failed to fetch quote: {e}")),
            })?;

        let quote = SwapQuote {
            input_asset: intent.input_asset.clone(),
            input_amount: intent.input_amount.clone(),
            output_asset: intent.output_asset.clone(),
            output_amount: quote_res.output_amount,
            fee_amount: quote_res.fee_amount,
            fee_asset: quote_res.fee_asset,
            deadline: quote_res.deadline_ms,
            rate: quote_res.rate,
        };

        let quote_id = quote_res.quote_id;

        self.state.lock().expect("mutex poisoned").quotes.insert(
            quote_id.clone(),
            QuoteRecord {
                wallet_id,
                intent,
                quote: quote.clone(),
            },
        );

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
            let state = self.state.lock().expect("mutex poisoned");
            let Some(record) = state.quotes.get(quote_id).cloned() else {
                return Err(ipc_err(errors::QUOTE_EXPIRED, "quote not found"));
            };
            if record.wallet_id != wallet_id {
                return Err(ipc_err(errors::QUOTE_EXPIRED, "quote not found"));
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

        let deposit_req = zkore_network::near_intents::DepositSubmitRequest {
            quote_id: quote_id.to_string(),
            destination_address: record.intent.destination_address.clone(),
            refund_address: record.intent.refund_address.clone(),
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut swap = SwapInfo {
            id: Uuid::new_v4(),
            remote_id: None,
            swap_type: record.intent.swap_type,
            input_asset: record.intent.input_asset,
            input_amount: record.intent.input_amount,
            output_asset: record.intent.output_asset,
            output_amount: Some(record.quote.output_amount),
            deposit_address: None,
            deposit_memo: None,
            destination_address: record.intent.destination_address,
            refund_address: record.intent.refund_address,
            state: SwapState::Draft,
            deadline: Some(record.quote.deadline),
            last_error: None,
            created_at: now_ms,
            updated_at: now_ms,
        };

        let conn = open_app_db(&self.app_db_path)?;
        swap_meta::insert_swap(&conn, wallet_id, &swap).context("failed to insert swap")?;

        if let Some(handler) = on_swap_changed.as_ref() {
            handler(SwapChangedEvent {
                schema_version: SCHEMA_VERSION,
                event: "swap.changed".to_string(),
                swap: swap.clone(),
            });
        }

        let deposit_res = match block_on(async { self.near.submit_deposit(deposit_req).await }) {
            Ok(res) => res,
            Err(e) => {
                swap.state = SwapState::Failed;
                swap.updated_at = chrono::Utc::now().timestamp_millis();
                swap.last_error = Some(e.to_string());
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
                return Err(match e {
                    zkore_network::near_intents::NearIntentsError::TorNotReady => {
                        ipc_err(errors::TOR_NOT_READY, "Tor is enabled but not ready")
                    }
                    _ => ipc_err(errors::SWAP_FAILED, format!("failed to start swap: {e}")),
                });
            }
        };

        swap.remote_id = deposit_res.remote_id;
        swap.deposit_address = Some(deposit_res.deposit_address);
        swap.deposit_memo = deposit_res.deposit_memo;
        if let Some(output_amount) = deposit_res.output_amount {
            swap.output_amount = Some(output_amount);
        }
        if let Some(deadline) = deposit_res.deadline_ms {
            swap.deadline = Some(deadline);
        }
        swap.state = SwapState::AwaitingDeposit;
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

    #[allow(clippy::too_many_arguments)]
    fn start_swap_from_zec(
        &self,
        wallet_id: Uuid,
        network: Network,
        quote_id: &str,
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

        let destination_address = record
            .intent
            .destination_address
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ipc_err(errors::INVALID_REQUEST, "destination_address required"))?
            .to_string();

        let conn = open_app_db(&self.app_db_path)?;
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

        let refund_address = {
            let Ok(mut mgr) = self.wallet_manager.lock() else {
                return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
            };
            let (wallet, wallet_db_conn) = mgr.require_active_unlocked_wallet_db()?;
            if wallet.id != wallet_id {
                return Err(ipc_err(errors::WALLET_NOT_FOUND, "wallet not found"));
            }
            derive_ephemeral_transparent_address(wallet_db_conn, network, account_id)?
        };

        let deposit_req = zkore_network::near_intents::DepositSubmitRequest {
            quote_id: quote_id.to_string(),
            destination_address: Some(destination_address),
            refund_address: Some(refund_address.clone()),
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut swap = SwapInfo {
            id: Uuid::new_v4(),
            remote_id: None,
            swap_type: record.intent.swap_type,
            input_asset: record.intent.input_asset,
            input_amount: record.intent.input_amount,
            output_asset: record.intent.output_asset,
            output_amount: Some(record.quote.output_amount),
            deposit_address: None,
            deposit_memo: None,
            destination_address: record.intent.destination_address,
            refund_address: Some(refund_address),
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

        let deposit_res = match block_on(async { self.near.submit_deposit(deposit_req).await }) {
            Ok(res) => res,
            Err(e) => {
                swap.state = SwapState::Failed;
                swap.updated_at = chrono::Utc::now().timestamp_millis();
                swap.last_error = Some(e.to_string());
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
                return Err(match e {
                    zkore_network::near_intents::NearIntentsError::TorNotReady => {
                        ipc_err(errors::TOR_NOT_READY, "Tor is enabled but not ready")
                    }
                    _ => ipc_err(errors::SWAP_FAILED, format!("failed to start swap: {e}")),
                });
            }
        };

        swap.remote_id = deposit_res.remote_id;
        swap.deposit_address = Some(deposit_res.deposit_address);
        swap.deposit_memo = deposit_res.deposit_memo;
        if let Some(output_amount) = deposit_res.output_amount {
            swap.output_amount = Some(output_amount);
        }
        if let Some(deadline) = deposit_res.deadline_ms {
            swap.deadline = Some(deadline);
        }
        swap.state = SwapState::AwaitingDeposit;
        swap.updated_at = chrono::Utc::now().timestamp_millis();

        swap_meta::update_swap(&conn, wallet_id, &swap).context("failed to update swap")?;
        if let Some(handler) = on_swap_changed.as_ref() {
            handler(SwapChangedEvent {
                schema_version: SCHEMA_VERSION,
                event: "swap.changed".to_string(),
                swap: swap.clone(),
            });
        }

        let send_result = {
            let Ok(mut mgr) = self.wallet_manager.lock() else {
                return Err(ipc_err(errors::WALLET_LOCKED, "wallet locked"));
            };

            let proposal = mgr.prepare_send(
                account_id,
                swap.deposit_address.as_deref().unwrap_or_default(),
                &swap.input_amount,
                swap.deposit_memo.as_deref(),
                allow_transparent_interaction,
            )?;

            mgr.confirm_send(&proposal.proposal_id, reauth_token, None)?;
            Ok::<(), anyhow::Error>(())
        };

        if let Err(err) = send_result {
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
        let conn = open_app_db(&self.app_db_path)?;
        let Some((owner_wallet_id, swap)) =
            swap_meta::get_swap(&conn, swap_id).context("failed to load swap")?
        else {
            return Err(ipc_err(errors::SWAP_FAILED, "swap not found"));
        };

        if owner_wallet_id != wallet_id {
            return Err(ipc_err(errors::SWAP_FAILED, "swap not found"));
        }

        Ok(GetSwapStatusResponse {
            schema_version: SCHEMA_VERSION,
            swap,
        })
    }

    pub fn list_swaps(&self, wallet_id: Uuid) -> anyhow::Result<ListSwapsResponse> {
        let conn = open_app_db(&self.app_db_path)?;
        let swaps =
            swap_meta::list_swaps_for_wallet(&conn, wallet_id).context("failed to list swaps")?;
        Ok(ListSwapsResponse {
            schema_version: SCHEMA_VERSION,
            swaps,
        })
    }

    fn start_polling(
        &self,
        wallet_id: Uuid,
        initial_swap: SwapInfo,
        on_swap_changed: Option<SwapEventHandler>,
    ) {
        let swap_id = initial_swap.id;

        {
            let state = self.state.lock().expect("mutex poisoned");
            if state.jobs.contains_key(&swap_id) {
                return;
            }
        }

        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        let state = Arc::clone(&self.state);
        let app_db_path = self.app_db_path.clone();
        let near = self.near.clone();
        let wallet_manager = Arc::clone(&self.wallet_manager);

        let handle = crate::tokio_runtime::spawn(async move {
            let mut backoff = Duration::from_secs(5);
            let mut swap = initial_swap;

            loop {
                tokio::select! {
                    _ = cancel_rx.changed() => {
                        break;
                    }
                    _ = tokio::time::sleep(backoff) => {}
                }

                let Some(deposit_address) = swap.deposit_address.clone() else {
                    break;
                };

                let status_res = near
                    .get_status(zkore_network::near_intents::StatusRequest {
                        deposit_address,
                        deposit_memo: swap.deposit_memo.clone(),
                    })
                    .await;

                match status_res {
                    Ok(status) => {
                        backoff = Duration::from_secs(5);

                        let mapped = zkore_network::near_intents::map_remote_status_to_local_state(
                            &status.status,
                        );
                        let mut next_state = mapped;

                        if mapped == SwapState::Confirming {
                            let confirmed =
                                has_confirmed_zcash_tx(&wallet_manager, wallet_id, &swap);
                            if confirmed {
                                next_state = SwapState::Completed;
                            }
                        }

                        if next_state != swap.state {
                            swap.state = next_state;
                            swap.updated_at = chrono::Utc::now().timestamp_millis();
                            swap.last_error = status.message.clone().or(swap.last_error);

                            match open_app_db(&app_db_path) {
                                Ok(conn) => {
                                    if let Err(db_err) =
                                        swap_meta::update_swap(&conn, wallet_id, &swap)
                                    {
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

                            if let Some(handler) = on_swap_changed.as_ref() {
                                handler(SwapChangedEvent {
                                    schema_version: SCHEMA_VERSION,
                                    event: "swap.changed".to_string(),
                                    swap: swap.clone(),
                                });
                            }
                        }

                        if matches!(
                            swap.state,
                            SwapState::Completed | SwapState::Refunded | SwapState::Failed
                        ) {
                            break;
                        }
                    }
                    Err(zkore_network::near_intents::NearIntentsError::RateLimited {
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
                        match open_app_db(&app_db_path) {
                            Ok(conn) => {
                                if let Err(db_err) = swap_meta::update_swap(&conn, wallet_id, &swap)
                                {
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
                        if let Some(handler) = on_swap_changed.as_ref() {
                            handler(SwapChangedEvent {
                                schema_version: SCHEMA_VERSION,
                                event: "swap.changed".to_string(),
                                swap: swap.clone(),
                            });
                        }
                    }
                }
            }

            let mut state = state.lock().expect("mutex poisoned");
            state.jobs.remove(&swap_id);
        });

        let finished = handle.is_finished();
        {
            let mut state = self.state.lock().expect("mutex poisoned");
            state.jobs.insert(
                swap_id,
                SwapJob {
                    cancel: cancel_tx,
                    handle,
                },
            );
            if finished {
                state.jobs.remove(&swap_id);
            }
        }
    }
}

fn has_confirmed_zcash_tx(
    wallet_manager: &Arc<Mutex<WalletManager>>,
    wallet_id: Uuid,
    swap: &SwapInfo,
) -> bool {
    let Ok(mut mgr) = wallet_manager.lock() else {
        return false;
    };

    let Some(active_wallet) = mgr.active_wallet_info() else {
        return false;
    };
    if active_wallet.id != wallet_id {
        return false;
    }

    let (wallet, conn) = match mgr.require_active_unlocked_wallet_db() {
        Ok(ctx) => ctx,
        Err(_) => return false,
    };
    if wallet.id != wallet_id {
        return false;
    }

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
        Err(_) => return false,
    };

    let mut rows = match stmt.query([]) {
        Ok(rows) => rows,
        Err(_) => return false,
    };

    while let Ok(Some(row)) = rows.next() {
        let is_shielding: bool = match row.get(4) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if is_shielding {
            continue;
        }

        let sent_note_count: i64 = match row.get(5) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let received_note_count: i64 = match row.get(6) {
            Ok(v) => v,
            Err(_) => continue,
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
                    Err(_) => continue,
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
                    Err(_) => continue,
                };
                let total_received: i64 = match row.get(3) {
                    Ok(v) => v,
                    Err(_) => continue,
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

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}

fn open_app_db(path: &Path) -> anyhow::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .with_context(|| format!("failed to open app metadata db: {}", path.display()))?;

    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .context("failed to enable foreign_keys")?;

    Ok(conn)
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => tokio::runtime::Runtime::new()
            .expect("create tokio runtime")
            .block_on(future),
    }
}
