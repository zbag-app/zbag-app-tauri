//! Background job service for async long-running operations.
//!
//! This service manages transaction jobs (send, shield) that run in the background,
//! allowing the UI to remain responsive while CPU-intensive proving and network
//! broadcast operations complete.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use zcash_client_backend::data_api::Account as _;

use zstash_core::domain::{JobId, JobProgress, JobState, JobType, Network};
use zstash_core::ipc::v1::common::SCHEMA_VERSION;
use zstash_core::ipc::v1::events::{JobProgressEvent, TransactionChangedEvent};

use crate::encryption::Dek;

/// Event handler for job progress updates.
pub type JobEventHandler = Arc<dyn Fn(JobProgressEvent) + Send + Sync>;

/// Event handler for transaction state changes.
pub type TxEventHandler = Arc<dyn Fn(TransactionChangedEvent) + Send + Sync>;

/// Manages background jobs for long-running operations.
#[derive(Debug, Clone)]
pub struct JobService {
    state: Arc<Mutex<JobServiceState>>,
}

#[derive(Debug)]
struct JobServiceState {
    /// Active jobs keyed by job_id.
    jobs: HashMap<JobId, JobEntry>,
    /// Current progress for each job.
    progress: HashMap<JobId, JobProgress>,
}

#[derive(Debug)]
struct JobEntry {
    /// Cancellation signal sender.
    cancel_tx: watch::Sender<bool>,
    /// Task handle.
    #[allow(dead_code)]
    handle: JoinHandle<()>,
    /// Wallet ID this job belongs to.
    wallet_id: Uuid,
}

/// Context needed to execute a send job.
pub struct SendJobContext {
    pub wallet_id: Uuid,
    pub network: Network,
    pub wallet_dir: PathBuf,
    pub wallet_dek: Dek,
    pub wallet_db_path: PathBuf,
    pub grpc_url: String,
    pub proposal_id: String,
    pub account_id: u32,
    pub spending_key: zcash_client_backend::keys::UnifiedSpendingKey,
    pub tor_manager: Option<Arc<zstash_tor::TorManager>>,
}

/// Context needed to execute a shield job.
pub struct ShieldJobContext {
    pub wallet_id: Uuid,
    pub network: Network,
    pub wallet_dir: PathBuf,
    pub wallet_dek: Dek,
    pub wallet_db_path: PathBuf,
    pub grpc_url: String,
    pub account_id: u32,
    pub consolidate: bool,
    pub spending_key: zcash_client_backend::keys::UnifiedSpendingKey,
    pub tor_manager: Option<Arc<zstash_tor::TorManager>>,
}

impl JobService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(JobServiceState {
                jobs: HashMap::new(),
                progress: HashMap::new(),
            })),
        }
    }

    /// Get the current progress of a job.
    pub fn get_progress(&self, job_id: &str) -> Option<JobProgress> {
        self.state
            .lock()
            .expect("mutex poisoned")
            .progress
            .get(job_id)
            .cloned()
    }

    /// List all active jobs for a wallet.
    pub fn list_jobs(&self, wallet_id: Uuid) -> Vec<JobProgress> {
        let state = self.state.lock().expect("mutex poisoned");
        state
            .jobs
            .iter()
            .filter(|(_, entry)| entry.wallet_id == wallet_id)
            .filter_map(|(job_id, _)| state.progress.get(job_id).cloned())
            .collect()
    }

    /// Cancel a job if it's in a cancellable state.
    /// Returns true if the job was cancelled or already terminal; false otherwise.
    pub fn cancel_job(&self, job_id: &str) -> bool {
        let mut state = self.state.lock().expect("mutex poisoned");

        if let Some(progress) = state.progress.get(job_id) {
            if matches!(
                progress.state,
                JobState::Completed | JobState::Failed | JobState::Cancelled
            ) {
                return true;
            }
            if !progress.can_cancel {
                return false;
            }
        }

        if let Some(entry) = state.jobs.remove(job_id) {
            let _ = entry.cancel_tx.send(true);
            entry.handle.abort();

            // Update progress to cancelled
            if let Some(progress) = state.progress.get_mut(job_id) {
                *progress = JobProgress::cancelled(job_id.to_string(), progress.job_type);
            }

            true
        } else {
            false
        }
    }

    /// Clear completed/failed/cancelled jobs for a wallet.
    pub fn clear_finished_jobs(&self, wallet_id: Uuid) {
        let mut state = self.state.lock().expect("mutex poisoned");

        // Find job IDs to remove
        let to_remove: Vec<JobId> = state
            .progress
            .iter()
            .filter(|(job_id, progress)| {
                matches!(
                    progress.state,
                    JobState::Completed | JobState::Failed | JobState::Cancelled
                ) && state
                    .jobs
                    .get(*job_id)
                    .is_none_or(|e| e.wallet_id == wallet_id)
            })
            .map(|(job_id, _)| job_id.clone())
            .collect();

        for job_id in to_remove {
            state.jobs.remove(&job_id);
            state.progress.remove(&job_id);
        }
    }

    /// Start a send job in the background.
    #[instrument(skip_all, fields(wallet_id = %ctx.wallet_id, proposal_id = %ctx.proposal_id))]
    pub fn start_send_job(
        &self,
        ctx: SendJobContext,
        proposal: zcash_client_backend::proposal::Proposal<
            zcash_client_backend::fees::StandardFeeRule,
            zcash_client_sqlite::ReceivedNoteId,
        >,
        on_progress: Option<JobEventHandler>,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<JobId> {
        let job_id = Uuid::new_v4().to_string();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        info!(job_id = %job_id, "starting send job");

        // Initialize progress
        {
            let mut state = self.state.lock().expect("mutex poisoned");
            state.progress.insert(
                job_id.clone(),
                JobProgress::queued(job_id.clone(), JobType::Send),
            );
        }

        // Emit initial progress
        self.emit_progress(&job_id, on_progress.as_ref());

        let state = Arc::clone(&self.state);
        let job_id_clone = job_id.clone();
        let on_progress_task = on_progress.clone();
        let ctx_wallet_id = ctx.wallet_id;

        let handle = crate::tokio_runtime::spawn(async move {
            run_send_job(
                job_id_clone,
                ctx,
                proposal,
                cancel_rx,
                state,
                on_progress_task,
                on_tx_changed,
            )
            .await;
        });

        // Store job entry
        {
            let mut state = self.state.lock().expect("mutex poisoned");
            state.jobs.insert(
                job_id.clone(),
                JobEntry {
                    cancel_tx,
                    handle,
                    wallet_id: ctx_wallet_id,
                },
            );
        }

        Ok(job_id)
    }

    /// Start a shield job in the background.
    #[instrument(skip_all, fields(wallet_id = %ctx.wallet_id, account_id = %ctx.account_id))]
    pub fn start_shield_job(
        &self,
        ctx: ShieldJobContext,
        on_progress: Option<JobEventHandler>,
        on_tx_changed: Option<TxEventHandler>,
    ) -> anyhow::Result<JobId> {
        let job_id = Uuid::new_v4().to_string();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        info!(job_id = %job_id, "starting shield job");

        // Initialize progress
        {
            let mut state = self.state.lock().expect("mutex poisoned");
            state.progress.insert(
                job_id.clone(),
                JobProgress::queued(job_id.clone(), JobType::Shield),
            );
        }

        // Emit initial progress
        self.emit_progress(&job_id, on_progress.as_ref());

        let state = Arc::clone(&self.state);
        let job_id_clone = job_id.clone();
        let on_progress_task = on_progress.clone();
        let ctx_wallet_id = ctx.wallet_id;

        let handle = crate::tokio_runtime::spawn(async move {
            run_shield_job(
                job_id_clone,
                ctx,
                cancel_rx,
                state,
                on_progress_task,
                on_tx_changed,
            )
            .await;
        });

        // Store job entry
        {
            let mut state = self.state.lock().expect("mutex poisoned");
            state.jobs.insert(
                job_id.clone(),
                JobEntry {
                    cancel_tx,
                    handle,
                    wallet_id: ctx_wallet_id,
                },
            );
        }

        Ok(job_id)
    }

    fn emit_progress(&self, job_id: &str, handler: Option<&JobEventHandler>) {
        let Some(handler) = handler else { return };
        let progress = self.get_progress(job_id);
        if let Some(progress) = progress {
            handler(JobProgressEvent {
                schema_version: SCHEMA_VERSION,
                event: "job.progress".to_string(),
                progress,
            });
        }
    }
}

impl Default for JobService {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a send job asynchronously.
async fn run_send_job(
    job_id: JobId,
    ctx: SendJobContext,
    proposal: zcash_client_backend::proposal::Proposal<
        zcash_client_backend::fees::StandardFeeRule,
        zcash_client_sqlite::ReceivedNoteId,
    >,
    cancel_rx: watch::Receiver<bool>,
    state: Arc<Mutex<JobServiceState>>,
    on_progress: Option<JobEventHandler>,
    on_tx_changed: Option<TxEventHandler>,
) {
    let job_type = JobType::Send;

    let emit_progress = |progress: JobProgress| {
        let is_terminal = matches!(
            progress.state,
            JobState::Completed | JobState::Failed | JobState::Cancelled
        );
        {
            let mut s = state.lock().expect("mutex poisoned");
            s.progress.insert(job_id.clone(), progress.clone());
            if is_terminal {
                s.jobs.remove(&job_id);
            }
        }
        if let Some(handler) = on_progress.as_ref() {
            handler(JobProgressEvent {
                schema_version: SCHEMA_VERSION,
                event: "job.progress".to_string(),
                progress,
            });
        }
    };

    // Check cancellation
    if *cancel_rx.borrow() {
        emit_progress(JobProgress::cancelled(job_id.clone(), job_type));
        return;
    }

    // Phase: Proving
    emit_progress(JobProgress::proving(job_id.clone(), job_type, Some(10)));

    // Open wallet DB for this job
    let wallet_db_conn = match open_wallet_db_for_job(&ctx.wallet_db_path, &ctx.wallet_dek) {
        Ok(conn) => conn,
        Err(e) => {
            error!(job_id = %job_id, error = ?e, "failed to open wallet db for send job");
            emit_progress(JobProgress::failed(
                job_id.clone(),
                job_type,
                format!("failed to open wallet: {e}"),
                None,
            ));
            return;
        }
    };

    // Capture values needed both inside and outside spawn_blocking
    let ctx_account_id = ctx.account_id;
    let ctx_wallet_id = ctx.wallet_id;
    let ctx_wallet_dir = ctx.wallet_dir.clone();
    let ctx_wallet_dek_bytes = ctx.wallet_dek.0;
    let ctx_grpc_url = ctx.grpc_url.clone();
    let ctx_tor_manager = ctx.tor_manager.clone();
    let ctx_network = ctx.network;

    // Run the CPU-intensive proving in a blocking task
    let result = tokio::task::spawn_blocking({
        let job_id = job_id.clone();
        let cancel_rx = cancel_rx.clone();
        let state = Arc::clone(&state);
        let on_progress = on_progress.clone();

        move || {
            // Check cancellation before proving
            if *cancel_rx.borrow() {
                return Err("cancelled".to_string());
            }

            let params = zcash_consensus_network(ctx.network);
            let mut conn = wallet_db_conn;

            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let spending_keys =
                zcash_client_backend::data_api::wallet::SpendingKeys::from_unified_spending_key(
                    ctx.spending_key.clone(),
                );
            let prover = zcash_proofs::prover::LocalTxProver::bundled();

            // Update progress during proving (simplified - real progress would need callbacks from prover)
            {
                let mut s = state.lock().expect("mutex poisoned");
                s.progress.insert(
                    job_id.clone(),
                    JobProgress::proving(job_id.clone(), job_type, Some(50)),
                );
            }
            if let Some(handler) = on_progress.as_ref() {
                handler(JobProgressEvent {
                    schema_version: SCHEMA_VERSION,
                    event: "job.progress".to_string(),
                    progress: JobProgress::proving(job_id.clone(), job_type, Some(50)),
                });
            }

            // Create the transaction (this does proving)
            let txids = zcash_client_backend::data_api::wallet::create_proposed_transactions::<
                _,
                _,
                std::convert::Infallible,
                _,
                std::convert::Infallible,
                _,
            >(
                &mut wdb,
                &params,
                &prover,
                &prover,
                &spending_keys,
                zcash_client_backend::wallet::OvkPolicy::Sender,
                &proposal,
            )
            .map_err(|e| format!("failed to build tx: {e}"))?;

            // Check cancellation after proving but before broadcast
            if *cancel_rx.borrow() {
                return Err("cancelled".to_string());
            }

            Ok((txids, conn))
        }
    })
    .await;

    let (txids, mut conn) = match result {
        Ok(Ok((txids, conn))) => (txids, conn),
        Ok(Err(e)) => {
            if e == "cancelled" {
                emit_progress(JobProgress::cancelled(job_id.clone(), job_type));
            } else {
                emit_progress(JobProgress::failed(job_id.clone(), job_type, e, None));
            }
            return;
        }
        Err(e) => {
            emit_progress(JobProgress::failed(
                job_id.clone(),
                job_type,
                format!("task panicked: {e}"),
                None,
            ));
            return;
        }
    };

    let primary_txid = txids[0].to_string();

    // Phase: Broadcasting
    emit_progress(JobProgress::broadcasting(
        job_id.clone(),
        job_type,
        primary_txid.clone(),
    ));

    // Broadcast the transaction
    let params = zcash_consensus_network(ctx_network);
    let wdb = zcash_client_sqlite::WalletDb::from_connection(
        &mut conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    #[allow(deprecated)]
    use zcash_client_backend::data_api::WalletRead as _;

    let mut broadcast_error: Option<String> = None;

    for txid in txids.iter() {
        let tx = match wdb.get_transaction(*txid) {
            Ok(Some(tx)) => tx,
            Ok(None) => {
                broadcast_error = Some("tx bytes unavailable".to_string());
                break;
            }
            Err(e) => {
                broadcast_error = Some(format!("failed to load tx: {e}"));
                break;
            }
        };

        let mut tx_bytes = Vec::new();
        if let Err(e) = tx.write(&mut tx_bytes) {
            broadcast_error = Some(format!("failed to serialize tx: {e}"));
            break;
        }

        // Broadcast
        match broadcast_transaction(&ctx_grpc_url, &tx_bytes, ctx_tor_manager.as_ref()).await {
            Ok(()) => {
                debug!(job_id = %job_id, txid = %txid, "broadcast successful");
            }
            Err(e) => {
                // Queue for retry but don't fail the job entirely
                debug!(job_id = %job_id, txid = %txid, error = ?e, "broadcast failed, queuing for retry");
                let dek = Dek(ctx_wallet_dek_bytes);
                if let Err(queue_err) = queue_broadcast_for_retry(
                    &ctx_wallet_id,
                    &ctx_wallet_dir,
                    &dek,
                    &txid.to_string(),
                    &tx_bytes,
                    Some(format!("{e:#}")),
                ) {
                    error!(job_id = %job_id, error = ?queue_err, "failed to queue broadcast for retry");
                }
                broadcast_error = Some(format!("{e:#}"));
            }
        }
    }

    // Emit transaction changed event
    if let Some(handler) = on_tx_changed.as_ref() {
        use zstash_core::domain::{TransactionInfo, TransactionStatus, TransactionType};
        let now_ms = chrono::Utc::now().timestamp_millis();
        let status = if broadcast_error.is_some() {
            TransactionStatus::Failed
        } else {
            TransactionStatus::Pending
        };

        handler(TransactionChangedEvent {
            schema_version: SCHEMA_VERSION,
            event: "tx.changed".to_string(),
            transaction: TransactionInfo {
                txid: primary_txid.clone(),
                account_id: ctx_account_id,
                tx_type: TransactionType::Send,
                value: "0".to_string(), // Will be filled by list_transactions
                fee: "0".to_string(),
                memo_present: false,
                memo: None,
                status,
                last_error: broadcast_error.clone(),
                can_retry_broadcast: broadcast_error.is_some(),
                mined_height: None,
                created_at: now_ms,
                confirmed_at: None,
            },
        });
    }

    // Complete (or fail) based on broadcast outcome
    if let Some(error) = broadcast_error {
        emit_progress(JobProgress::failed(
            job_id.clone(),
            job_type,
            error,
            Some(primary_txid),
        ));
    } else {
        emit_progress(JobProgress::completed(
            job_id.clone(),
            job_type,
            primary_txid,
        ));
    }

    info!(job_id = %job_id, "send job completed");
}

/// Execute a shield job asynchronously.
async fn run_shield_job(
    job_id: JobId,
    ctx: ShieldJobContext,
    cancel_rx: watch::Receiver<bool>,
    state: Arc<Mutex<JobServiceState>>,
    on_progress: Option<JobEventHandler>,
    on_tx_changed: Option<TxEventHandler>,
) {
    let job_type = JobType::Shield;

    let emit_progress = |progress: JobProgress| {
        let is_terminal = matches!(
            progress.state,
            JobState::Completed | JobState::Failed | JobState::Cancelled
        );
        {
            let mut s = state.lock().expect("mutex poisoned");
            s.progress.insert(job_id.clone(), progress.clone());
            if is_terminal {
                s.jobs.remove(&job_id);
            }
        }
        if let Some(handler) = on_progress.as_ref() {
            handler(JobProgressEvent {
                schema_version: SCHEMA_VERSION,
                event: "job.progress".to_string(),
                progress,
            });
        }
    };

    // Check cancellation
    if *cancel_rx.borrow() {
        emit_progress(JobProgress::cancelled(job_id.clone(), job_type));
        return;
    }

    // Phase: Proving
    emit_progress(JobProgress::proving(job_id.clone(), job_type, Some(10)));

    // Capture values needed after spawn_blocking
    let ctx_account_id = ctx.account_id;

    // Open wallet DB for this job
    let wallet_db_conn = match open_wallet_db_for_job(&ctx.wallet_db_path, &ctx.wallet_dek) {
        Ok(conn) => conn,
        Err(e) => {
            error!(job_id = %job_id, error = ?e, "failed to open wallet db for shield job");
            emit_progress(JobProgress::failed(
                job_id.clone(),
                job_type,
                format!("failed to open wallet: {e}"),
                None,
            ));
            return;
        }
    };

    // Run the shielding in a blocking task
    let runtime_handle = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => {
            emit_progress(JobProgress::failed(
                job_id.clone(),
                job_type,
                "tokio runtime unavailable".to_string(),
                None,
            ));
            return;
        }
    };

    let result = tokio::task::spawn_blocking({
        let job_id = job_id.clone();
        let cancel_rx = cancel_rx.clone();
        let state = Arc::clone(&state);
        let on_progress = on_progress.clone();
        let runtime_handle = runtime_handle.clone();

        move || {
            shield_funds_blocking(
                job_id,
                ctx,
                wallet_db_conn,
                cancel_rx,
                state,
                on_progress,
                runtime_handle,
            )
        }
    })
    .await;

    let (primary_txid, _conn, broadcast_error) = match result {
        Ok(Ok((txid, conn, broadcast_error))) => (txid, conn, broadcast_error),
        Ok(Err(e)) => {
            if e == "cancelled" {
                emit_progress(JobProgress::cancelled(job_id.clone(), job_type));
            } else {
                emit_progress(JobProgress::failed(job_id.clone(), job_type, e, None));
            }
            return;
        }
        Err(e) => {
            emit_progress(JobProgress::failed(
                job_id.clone(),
                job_type,
                format!("task panicked: {e}"),
                None,
            ));
            return;
        }
    };

    // Emit transaction changed event
    if let Some(handler) = on_tx_changed.as_ref() {
        use zstash_core::domain::{TransactionInfo, TransactionStatus, TransactionType};
        let now_ms = chrono::Utc::now().timestamp_millis();
        let status = if broadcast_error.is_some() {
            TransactionStatus::Failed
        } else {
            TransactionStatus::Pending
        };

        handler(TransactionChangedEvent {
            schema_version: SCHEMA_VERSION,
            event: "tx.changed".to_string(),
            transaction: TransactionInfo {
                txid: primary_txid.clone(),
                account_id: ctx_account_id,
                tx_type: TransactionType::Shield,
                value: "0".to_string(),
                fee: "0".to_string(),
                memo_present: false,
                memo: None,
                status,
                last_error: broadcast_error.clone(),
                can_retry_broadcast: broadcast_error.is_some(),
                mined_height: None,
                created_at: now_ms,
                confirmed_at: None,
            },
        });
    }

    // Complete (or fail) based on broadcast outcome
    if let Some(error) = broadcast_error {
        emit_progress(JobProgress::failed(
            job_id.clone(),
            job_type,
            error,
            Some(primary_txid),
        ));
    } else {
        emit_progress(JobProgress::completed(
            job_id.clone(),
            job_type,
            primary_txid,
        ));
    }

    info!(job_id = %job_id, "shield job completed");
}

/// Blocking implementation of shield_funds for the job service.
fn shield_funds_blocking(
    job_id: JobId,
    ctx: ShieldJobContext,
    mut conn: rusqlite::Connection,
    cancel_rx: watch::Receiver<bool>,
    state: Arc<Mutex<JobServiceState>>,
    on_progress: Option<JobEventHandler>,
    runtime_handle: tokio::runtime::Handle,
) -> Result<(String, rusqlite::Connection, Option<String>), String> {
    use std::collections::{BTreeMap, BTreeSet};
    use zcash_client_backend::data_api::{InputSource as _, WalletRead as _};
    use zcash_client_backend::fees::ChangeStrategy as _;
    use zcash_primitives::transaction::fees::transparent as transparent_fees;

    let job_type = JobType::Shield;
    const MAX_SHIELDING_INPUTS_PER_TX: usize = 200;

    // Check cancellation
    if *cancel_rx.borrow() {
        return Err("cancelled".to_string());
    }

    let params = zcash_consensus_network(ctx.network);
    let account_uuid = {
        let wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        // Find account UUID
        let account_ids = wdb
            .get_account_ids()
            .map_err(|e| format!("failed to list accounts: {e}"))?;

        let mut found_uuid = None;
        for uuid in account_ids {
            if let Ok(Some(account)) = wdb.get_account(uuid) {
                if let Some(key_source) = account.source().key_source()
                    && crate::account_key_source::parse_account_id_from_key_source(key_source)
                        == Some(ctx.account_id)
                {
                    found_uuid = Some(uuid);
                    break;
                }
                if let Some(derivation) = account.source().key_derivation() {
                    let idx: u32 = derivation.account_index().into();
                    if idx == ctx.account_id {
                        found_uuid = Some(uuid);
                        break;
                    }
                }
            }
        }

        found_uuid.ok_or_else(|| "account not found".to_string())?
    };

    let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
        &mut conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    let receivers = wdb
        .get_transparent_receivers(account_uuid, false, false)
        .map_err(|e| format!("failed to list transparent receivers: {e}"))?;
    let from_addrs: Vec<_> = receivers.into_keys().collect();

    let chain_tip_height = wdb
        .chain_height()
        .map_err(|e| format!("failed to read chain height: {e}"))?
        .ok_or_else(|| "must scan blocks first".to_string())?;
    let target_height: zcash_client_backend::data_api::wallet::TargetHeight =
        (chain_tip_height + 1).into();
    let confirmations_policy =
        zcash_client_backend::data_api::wallet::ConfirmationsPolicy::default();

    let mut transparent_inputs = Vec::new();
    for addr in from_addrs.iter() {
        let outputs = wdb
            .get_spendable_transparent_outputs(addr, target_height, confirmations_policy)
            .map_err(|e| format!("failed to list transparent outputs: {e}"))?;
        transparent_inputs.extend(outputs.into_iter().map(|u| u.into_wallet_output()));
    }

    if transparent_inputs.is_empty() {
        return Err("no transparent funds to shield".to_string());
    }

    // Update progress
    {
        let mut s = state.lock().expect("mutex poisoned");
        s.progress.insert(
            job_id.clone(),
            JobProgress::proving(job_id.clone(), job_type, Some(30)),
        );
    }
    if let Some(handler) = on_progress.as_ref() {
        handler(JobProgressEvent {
            schema_version: SCHEMA_VERSION,
            event: "job.progress".to_string(),
            progress: JobProgress::proving(job_id.clone(), job_type, Some(30)),
        });
    }

    let fee_rule = zcash_client_backend::fees::StandardFeeRule::Zip317;
    let change_strategy = zcash_client_backend::fees::standard::SingleOutputChangeStrategy::new(
        fee_rule,
        None,
        zcash_protocol::ShieldedProtocol::Orchard,
        zcash_client_backend::fees::DustOutputPolicy::default(),
    );

    let spending_keys =
        zcash_client_backend::data_api::wallet::SpendingKeys::from_unified_spending_key(
            ctx.spending_key.clone(),
        );
    let prover = zcash_proofs::prover::LocalTxProver::bundled();

    let batches: Vec<Vec<_>> = transparent_inputs
        .chunks(MAX_SHIELDING_INPUTS_PER_TX)
        .map(|chunk| chunk.to_vec())
        .collect();

    let mut primary_txid: Option<String> = None;
    let mut broadcast_error: Option<String> = None;

    for batch in batches {
        // Check cancellation
        if *cancel_rx.borrow() {
            return Err("cancelled".to_string());
        }

        if batch.is_empty() {
            continue;
        }

        let mut input_selection = batch;
        change_strategy
            .fetch_wallet_meta(&wdb, account_uuid, target_height, &[])
            .map_err(|e| format!("failed to load wallet metadata: {e}"))?;

        #[derive(Debug)]
        struct Zip317P2pkhTransparentInput<'a> {
            utxo: &'a zcash_client_backend::wallet::WalletTransparentOutput,
        }

        impl transparent_fees::InputView for Zip317P2pkhTransparentInput<'_> {
            fn outpoint(&self) -> &zcash_transparent::bundle::OutPoint {
                self.utxo.outpoint()
            }

            fn coin(&self) -> &zcash_transparent::bundle::TxOut {
                self.utxo.txout()
            }

            fn serialized_size(&self) -> transparent_fees::InputSize {
                match self.utxo.recipient_address() {
                    zcash_transparent::address::TransparentAddress::PublicKeyHash(_) => {
                        transparent_fees::InputSize::Known(149)
                    }
                    _ => transparent_fees::InputSize::Unknown(self.utxo.outpoint().clone()),
                }
            }
        }

        let balance = loop {
            let input_views: Vec<_> = input_selection
                .iter()
                .map(|utxo| Zip317P2pkhTransparentInput { utxo })
                .collect();

            match change_strategy.compute_balance::<_, std::convert::Infallible>(
                &params,
                target_height,
                &input_views,
                &[] as &[zcash_transparent::bundle::TxOut],
                &zcash_client_backend::fees::sapling::EmptyBundleView,
                &zcash_client_backend::fees::orchard::EmptyBundleView,
                None,
                &(),
            ) {
                Ok(balance) => break Some(balance),
                Err(zcash_client_backend::fees::ChangeError::DustInputs {
                    transparent, ..
                }) => {
                    let exclusions: BTreeSet<zcash_transparent::bundle::OutPoint> =
                        transparent.into_iter().collect();
                    input_selection.retain(|i| !exclusions.contains(i.outpoint()));
                    if input_selection.is_empty() {
                        break None;
                    }
                }
                Err(zcash_client_backend::fees::ChangeError::InsufficientFunds { .. }) => {
                    return Err(
                        "insufficient transparent balance to cover shielding fee".to_string()
                    );
                }
                Err(other) => {
                    return Err(format!("failed to compute shielding balance: {other}"));
                }
            }
        };

        if input_selection.is_empty() {
            continue;
        }
        let Some(balance) = balance else {
            continue;
        };

        let proposal = zcash_client_backend::proposal::Proposal::<
            zcash_client_backend::fees::StandardFeeRule,
            std::convert::Infallible,
        >::single_step(
            zcash_client_backend::zip321::TransactionRequest::empty(),
            BTreeMap::new(),
            input_selection,
            None,
            balance,
            fee_rule,
            target_height,
            true,
        )
        .map_err(|e| format!("invalid shielding proposal: {e}"))?;

        // Update progress
        {
            let mut s = state.lock().expect("mutex poisoned");
            s.progress.insert(
                job_id.clone(),
                JobProgress::proving(job_id.clone(), job_type, Some(60)),
            );
        }
        if let Some(handler) = on_progress.as_ref() {
            handler(JobProgressEvent {
                schema_version: SCHEMA_VERSION,
                event: "job.progress".to_string(),
                progress: JobProgress::proving(job_id.clone(), job_type, Some(60)),
            });
        }

        let txids = zcash_client_backend::data_api::wallet::create_proposed_transactions::<
            _,
            _,
            std::convert::Infallible,
            _,
            std::convert::Infallible,
            std::convert::Infallible,
        >(
            &mut wdb,
            &params,
            &prover,
            &prover,
            &spending_keys,
            zcash_client_backend::wallet::OvkPolicy::Sender,
            &proposal,
        )
        .map_err(|e| format!("failed to build shielding tx: {e}"))?;

        for txid in txids.iter() {
            let txid_str = txid.to_string();
            if primary_txid.is_none() {
                primary_txid = Some(txid_str.clone());
            }

            // Get tx bytes and broadcast
            #[allow(deprecated)]
            use zcash_client_backend::data_api::WalletRead as _;

            let tx = wdb
                .get_transaction(*txid)
                .map_err(|e| format!("failed to load tx: {e}"))?
                .ok_or_else(|| "tx bytes unavailable".to_string())?;

            let mut tx_bytes = Vec::new();
            tx.write(&mut tx_bytes)
                .map_err(|e| format!("failed to serialize tx: {e}"))?;

            // Update progress to broadcasting
            {
                let mut s = state.lock().expect("mutex poisoned");
                s.progress.insert(
                    job_id.clone(),
                    JobProgress::broadcasting(job_id.clone(), job_type, txid_str.clone()),
                );
            }
            if let Some(handler) = on_progress.as_ref() {
                handler(JobProgressEvent {
                    schema_version: SCHEMA_VERSION,
                    event: "job.progress".to_string(),
                    progress: JobProgress::broadcasting(job_id.clone(), job_type, txid_str.clone()),
                });
            }

            // Broadcast synchronously (we're already in a blocking context)
            let grpc_url = ctx.grpc_url.clone();
            let tor = ctx.tor_manager.clone();
            let tx_bytes_clone = tx_bytes.clone();

            let result = runtime_handle.block_on(async {
                broadcast_transaction(&grpc_url, &tx_bytes_clone, tor.as_ref()).await
            });

            if let Err(e) = result {
                let error = format!("{e:#}");
                debug!(job_id = %job_id, txid = %txid_str, error = ?e, "broadcast failed, queuing for retry");
                if let Err(queue_err) = queue_broadcast_for_retry(
                    &ctx.wallet_id,
                    &ctx.wallet_dir,
                    &ctx.wallet_dek,
                    &txid_str,
                    &tx_bytes,
                    Some(error.clone()),
                ) {
                    error!(job_id = %job_id, error = ?queue_err, "failed to queue broadcast for retry");
                }
                if broadcast_error.is_none() {
                    broadcast_error = Some(error);
                }
            }
        }
    }

    let Some(primary_txid) = primary_txid else {
        return Err("no transparent funds to shield".to_string());
    };

    Ok((primary_txid, conn, broadcast_error))
}

fn open_wallet_db_for_job(
    wallet_db_path: &std::path::Path,
    dek: &Dek,
) -> anyhow::Result<rusqlite::Connection> {
    use rusqlite::OpenFlags;
    use zeroize::Zeroize;

    let conn =
        rusqlite::Connection::open_with_flags(wallet_db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .with_context(|| format!("failed to open wallet db: {}", wallet_db_path.display()))?;

    let mut dek_hex = dek.0.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");
    conn.execute_batch(&pragma)
        .context("failed to apply wallet db encryption key")?;

    dek_hex.zeroize();
    pragma.zeroize();

    rusqlite::vtab::array::load_module(&conn).context("failed to load sqlite array module")?;

    // Force an early read to detect an incorrect key
    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
        .context("wallet db is not readable (incorrect key or corrupted db)")?;

    Ok(conn)
}

async fn broadcast_transaction(
    grpc_url: &str,
    tx_bytes: &[u8],
    tor_manager: Option<&Arc<zstash_tor::TorManager>>,
) -> anyhow::Result<()> {
    let client = match tor_manager {
        Some(tor) => zstash_network::grpc_client::GrpcClient::new_with_tor(
            grpc_url.to_string(),
            Arc::clone(tor),
        ),
        None => zstash_network::grpc_client::GrpcClient::new(grpc_url.to_string()),
    };

    client.send_transaction(tx_bytes.to_vec()).await
}

fn queue_broadcast_for_retry(
    wallet_id: &Uuid,
    wallet_dir: &std::path::Path,
    wallet_dek: &Dek,
    txid: &str,
    tx_bytes: &[u8],
    last_error: Option<String>,
) -> anyhow::Result<()> {
    use chacha20poly1305::aead::{Aead, Payload};
    use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
    use rand::RngCore as _;

    let queue_dir = wallet_dir.join("queued_broadcasts");
    std::fs::create_dir_all(&queue_dir)?;

    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let nonce_ref: &XNonce = XNonce::from_slice(&nonce);
    let cipher = XChaCha20Poly1305::new_from_slice(&wallet_dek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let aad = format!("wallet_id={wallet_id};txid={txid};aead_scheme=xchacha20poly1305;v=1");
    let ciphertext = cipher
        .encrypt(
            nonce_ref,
            Payload {
                msg: tx_bytes,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to encrypt queued tx: {e}"))?;

    let bin_path = queue_dir.join(format!("{txid}.bin"));
    let meta_path = queue_dir.join(format!("{txid}.json"));

    let mut out = Vec::with_capacity(24 + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    std::fs::write(&bin_path, out)?;

    #[derive(serde::Serialize)]
    struct QueuedBroadcastMeta {
        created_at_ms: i64,
        last_error: Option<String>,
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    let meta = QueuedBroadcastMeta {
        created_at_ms: now_ms,
        last_error,
    };
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)?;

    Ok(())
}

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}

use anyhow::Context as _;
