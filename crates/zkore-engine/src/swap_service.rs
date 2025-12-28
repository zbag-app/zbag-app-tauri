use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;
use rusqlite::OpenFlags;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use uuid::Uuid;

use zkore_core::domain::{Network, SwapInfo, SwapIntent, SwapQuote, SwapState};
use zkore_core::errors;
use zkore_core::ipc::v1::commands::swap::{
    GetSwapStatusResponse, ListSwapsResponse, RequestSwapQuoteResponse, StartSwapResponse,
};
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::events::SwapChangedEvent;

use crate::db::swap_meta;
use crate::error::ipc_err;

pub type SwapEventHandler = Arc<dyn Fn(SwapChangedEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct SwapService {
    app_db_path: PathBuf,
    near: zkore_network::near_intents::NearIntentsClient,
    state: Arc<Mutex<State>>,
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
struct SwapJob {
    cancel: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

impl SwapService {
    pub fn new(app_db_path: PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            app_db_path,
            near: zkore_network::near_intents::NearIntentsClient::new()?,
            state: Arc::new(Mutex::new(State {
                quotes: HashMap::new(),
                jobs: HashMap::new(),
            })),
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

        let quote_res = block_on(async { self.near.get_quote(req).await }).map_err(|e| {
            ipc_err(errors::SWAP_FAILED, format!("failed to fetch quote: {e}"))
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

        self.state
            .lock()
            .expect("mutex poisoned")
            .quotes
            .insert(
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
        _reauth_token: Option<&str>,
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
                let _ = swap_meta::update_swap(&conn, wallet_id, &swap);
                if let Some(handler) = on_swap_changed.as_ref() {
                    handler(SwapChangedEvent {
                        schema_version: SCHEMA_VERSION,
                        event: "swap.changed".to_string(),
                        swap: swap.clone(),
                    });
                }
                return Err(ipc_err(errors::SWAP_FAILED, format!("failed to start swap: {e}")));
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

        let handle = tokio::spawn(async move {
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

                        let mapped = zkore_network::near_intents::map_remote_status_to_local_state(&status.status);
                        let mut next_state = mapped;
                        if swap.state == SwapState::Confirming && mapped == SwapState::Confirming {
                            next_state = SwapState::Completed;
                        }

                        if next_state != swap.state {
                            swap.state = next_state;
                            swap.updated_at = chrono::Utc::now().timestamp_millis();
                            swap.last_error = status.message.clone().or(swap.last_error);

                            if let Ok(conn) = open_app_db(&app_db_path) {
                                let _ = swap_meta::update_swap(&conn, wallet_id, &swap);
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
                    Err(zkore_network::near_intents::NearIntentsError::RateLimited { retry_after }) => {
                        backoff = retry_after.unwrap_or_else(|| backoff.saturating_mul(2)).min(Duration::from_secs(60));
                    }
                    Err(err) => {
                        backoff = backoff.saturating_mul(2).min(Duration::from_secs(60));
                        swap.last_error = Some(err.to_string());
                        swap.updated_at = chrono::Utc::now().timestamp_millis();
                        if let Ok(conn) = open_app_db(&app_db_path) {
                            let _ = swap_meta::update_swap(&conn, wallet_id, &swap);
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
