use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use rusqlite::Connection;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use uuid::Uuid;

use std::io::Write as _;

use prost::Message as _;
use zcash_client_backend::data_api::chain::{
    ChainState, error::Error as ChainError, scan_cached_blocks,
};
use zcash_client_backend::data_api::scanning::ScanPriority;
use zcash_client_backend::data_api::wallet::ConfirmationsPolicy;
use zcash_client_backend::data_api::{WalletRead as _, WalletWrite as _};
use zcash_client_backend::decrypt_transaction;
use zcash_client_backend::proto::compact_formats::CompactBlock;
use zcash_client_sqlite::FsBlockDb;
use zcash_client_sqlite::chain::BlockMeta;
use zcash_client_sqlite::chain::init::init_blockmeta_db;
use zcash_primitives::block::BlockHash;
use zcash_primitives::transaction::{Transaction, TxId};
use zcash_protocol::consensus::{BlockHeight, BranchId};

use zbag_core::domain::{Balance, Network, SyncPhase, SyncProgress};
use zbag_core::errors;
use zbag_core::ipc::v1::common::SCHEMA_VERSION;
use zbag_core::ipc::v1::events::{BalanceChangedEvent, SyncProgressEvent};

use crate::db::{AppDb, OpenSqlcipherOptions, open_sqlcipher_db};
use crate::encryption::Dek;
use crate::error::ipc_err;
use crate::server_resolver;
use zbag_core::permissions::{create_dir_all_secure, secure_open_options};

/// Default batch size for downloading blocks.
///
/// Reduced from 1000 to 250 to improve mid-sync balance refresh cadence in the
/// UI (more frequent progress/balance opportunities while catching up).
/// Tradeoff: smaller batches increase round trips and can reduce peak sync
/// throughput on fast links or sparse wallets.
const BATCH_SIZE: u32 = 250;

/// Smaller batch size for sandblasting periods where blocks are much larger.
/// Matches Zashi's SYNC_BATCH_SMALL_SIZE.
const BATCH_SIZE_SANDBLASTING: u32 = 100;

/// Known Zcash mainnet sandblasting period (blocks 1.71M to 2.05M).
/// During this range, we use smaller batches due to larger block sizes.
const SANDBLASTING_RANGE: std::ops::RangeInclusive<u32> = 1_710_000..=2_050_000;

/// Number of batches to buffer ahead for download/scan pipelining.
const LOOKAHEAD_BATCHES: usize = 4;

/// Time-based throttle for `balance.changed` emission opportunities.
const BALANCE_EMIT_MIN_INTERVAL: Duration = Duration::from_millis(1500);

/// Scan-progress-based throttle for `balance.changed` emission opportunities.
const BALANCE_EMIT_BLOCK_THRESHOLD: u32 = 250;

/// Poll interval once the wallet is caught up to tip.
pub(crate) const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(20);

/// Maximum backoff after repeated sync failures.
const MAX_POLL_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Adaptive download batch sizing limits and tuning.
const ADAPTIVE_BATCH_MIN_SIZE: u32 = 125;
const ADAPTIVE_BATCH_MAX_SIZE: u32 = 2_000;
const ADAPTIVE_BATCH_FAST_SCAN_SECS: f64 = 0.80;
const ADAPTIVE_BATCH_SLOW_SCAN_SECS: f64 = 2.00;
const ADAPTIVE_BATCH_GROWTH_NUMERATOR: u32 = 3;
const ADAPTIVE_BATCH_GROWTH_DENOMINATOR: u32 = 2;
const ADAPTIVE_BATCH_SHRINK_NUMERATOR: u32 = 3;
const ADAPTIVE_BATCH_SHRINK_DENOMINATOR: u32 = 4;

/// A downloaded batch of blocks ready for scanning.
struct DownloadedBatch {
    range_start: BlockHeight,
    range_end: BlockHeight,
    blocks: Vec<CompactBlock>,
}

#[derive(Debug, Default, Clone, Copy)]
struct ScanBatchActivity {
    scanned_blocks: u32,
    spent_sapling: u32,
    spent_orchard: u32,
    received_sapling: u32,
    received_orchard: u32,
}

impl ScanBatchActivity {
    fn touched_notes(self) -> u32 {
        self.spent_sapling
            .saturating_add(self.spent_orchard)
            .saturating_add(self.received_sapling)
            .saturating_add(self.received_orchard)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdaptiveBatchDecision {
    Grew,
    Shrunk,
}

#[derive(Debug, Clone, Copy)]
struct AdaptiveBatchAdjustment {
    previous: u32,
    current: u32,
    decision: AdaptiveBatchDecision,
}

#[derive(Debug)]
struct AdaptiveBatchController {
    min_size: u32,
    max_size: u32,
    current_size: u32,
}

impl AdaptiveBatchController {
    fn new(range_start: BlockHeight) -> Self {
        let base = effective_batch_size(range_start);
        if base == BATCH_SIZE_SANDBLASTING {
            return Self {
                min_size: base,
                max_size: base,
                current_size: base,
            };
        }

        Self {
            min_size: ADAPTIVE_BATCH_MIN_SIZE.min(BATCH_SIZE),
            max_size: ADAPTIVE_BATCH_MAX_SIZE.max(BATCH_SIZE),
            current_size: base,
        }
    }

    fn next_download_size(&self, height: BlockHeight) -> u32 {
        let base = effective_batch_size(height);
        if base == BATCH_SIZE_SANDBLASTING {
            BATCH_SIZE_SANDBLASTING
        } else {
            self.current_size.clamp(self.min_size, self.max_size)
        }
    }

    fn record_scan_result(
        &mut self,
        batch_start: BlockHeight,
        scan_duration: Duration,
        activity: ScanBatchActivity,
    ) -> Option<AdaptiveBatchAdjustment> {
        if activity.scanned_blocks == 0 {
            return None;
        }

        if effective_batch_size(batch_start) == BATCH_SIZE_SANDBLASTING {
            return None;
        }

        let elapsed_secs = scan_duration.as_secs_f64();
        if elapsed_secs <= 0.0 {
            return None;
        }

        let previous = self.current_size;
        let touched_notes = activity.touched_notes();

        if elapsed_secs >= ADAPTIVE_BATCH_SLOW_SCAN_SECS {
            self.current_size = self
                .current_size
                .saturating_mul(ADAPTIVE_BATCH_SHRINK_NUMERATOR)
                / ADAPTIVE_BATCH_SHRINK_DENOMINATOR;
        } else if elapsed_secs <= ADAPTIVE_BATCH_FAST_SCAN_SECS && touched_notes == 0 {
            self.current_size = self
                .current_size
                .saturating_mul(ADAPTIVE_BATCH_GROWTH_NUMERATOR)
                / ADAPTIVE_BATCH_GROWTH_DENOMINATOR;
        }

        self.current_size = self.current_size.clamp(self.min_size, self.max_size);

        if self.current_size == previous {
            return None;
        }

        let decision = if self.current_size > previous {
            AdaptiveBatchDecision::Grew
        } else {
            AdaptiveBatchDecision::Shrunk
        };

        Some(AdaptiveBatchAdjustment {
            previous,
            current: self.current_size,
            decision,
        })
    }
}

/// Result type for the download task.
enum DownloadResult {
    /// A batch of blocks was downloaded successfully.
    Batch(DownloadedBatch),
    /// Download completed for a range.
    RangeComplete,
    /// An error occurred during download.
    Error(anyhow::Error),
    /// Download was cancelled.
    Cancelled,
}

type SyncEventHandler = Arc<dyn Fn(SyncProgressEvent) + Send + Sync>;
type BalanceEventHandler = Arc<dyn Fn(BalanceChangedEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct SyncService {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    jobs: HashMap<Uuid, SyncJob>,
    blocking_scans: HashMap<Uuid, usize>,
    progress: HashMap<Uuid, SyncProgress>,
    balances: HashMap<(Uuid, u32), Balance>,
    started_at: HashMap<Uuid, Instant>,
    progress_estimates: HashMap<Uuid, SyncProgressEstimate>,
}

#[derive(Debug, Default, Clone)]
struct SyncProgressEstimate {
    start_height: Option<u32>,
    start_instant: Option<Instant>,
    target_height: Option<u32>,
    last_frontier_height: Option<u32>,
    last_update_at: Option<Instant>,
    ewma_blocks_per_sec: Option<f64>,
    last_percent: Option<u8>,
    last_eta_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BalanceEmitTrigger {
    SuccessfulScanBatch { scanned_blocks: u32 },
    ForcedMilestone,
}

#[derive(Debug, Default, Clone)]
struct BalanceEmitThrottle {
    last_emit_at: Option<Instant>,
    scanned_blocks_since_emit: u32,
    saw_successful_scan_batch: bool,
}

impl BalanceEmitThrottle {
    fn should_emit_and_update(&mut self, trigger: BalanceEmitTrigger, now: Instant) -> bool {
        match trigger {
            BalanceEmitTrigger::ForcedMilestone => true,
            BalanceEmitTrigger::SuccessfulScanBatch { scanned_blocks } => {
                self.should_emit_after_successful_scan_batch(scanned_blocks, now)
            }
        }
    }

    fn should_emit_after_successful_scan_batch(
        &mut self,
        scanned_blocks: u32,
        now: Instant,
    ) -> bool {
        self.scanned_blocks_since_emit = self
            .scanned_blocks_since_emit
            .saturating_add(scanned_blocks);

        // Force an emit opportunity on the first successfully scanned batch in this run.
        if !self.saw_successful_scan_batch {
            self.saw_successful_scan_batch = true;
            return true;
        }

        let time_gate = self
            .last_emit_at
            .map(|last| now.saturating_duration_since(last) >= BALANCE_EMIT_MIN_INTERVAL)
            .unwrap_or(true);
        let block_gate = self.scanned_blocks_since_emit >= BALANCE_EMIT_BLOCK_THRESHOLD;

        time_gate || block_gate
    }

    fn record_emit(&mut self, now: Instant) {
        self.last_emit_at = Some(now);
        self.scanned_blocks_since_emit = 0;
    }

    fn start_new_auto_sync_iteration(&mut self) {
        self.scanned_blocks_since_emit = 0;
        self.saw_successful_scan_batch = false;
    }
}

#[derive(Debug)]
struct SyncJob {
    cancel: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

#[derive(Debug)]
struct BlockingScanInFlightGuard {
    state: Arc<Mutex<State>>,
    wallet_id: Uuid,
}

impl BlockingScanInFlightGuard {
    fn acquire(state: Arc<Mutex<State>>, wallet_id: Uuid) -> Self {
        let mut locked = state.lock().expect("mutex poisoned");
        *locked.blocking_scans.entry(wallet_id).or_insert(0) += 1;
        drop(locked);

        Self { state, wallet_id }
    }
}

impl Drop for BlockingScanInFlightGuard {
    fn drop(&mut self) {
        let mut state = self.state.lock().expect("mutex poisoned");
        let Some(count) = state.blocking_scans.get_mut(&self.wallet_id) else {
            return;
        };

        *count = count.saturating_sub(1);
        if *count == 0 {
            state.blocking_scans.remove(&self.wallet_id);
        }
    }
}

impl SyncService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                jobs: HashMap::new(),
                blocking_scans: HashMap::new(),
                progress: HashMap::new(),
                balances: HashMap::new(),
                started_at: HashMap::new(),
                progress_estimates: HashMap::new(),
            })),
        }
    }

    pub fn get_progress(&self, wallet_id: Uuid) -> SyncProgress {
        let mut state = self.state.lock().expect("mutex poisoned");
        let progress = state
            .progress
            .get(&wallet_id)
            .cloned()
            .unwrap_or_else(default_progress);

        // Keep ETA/progress estimates fresh even if the sync engine hasn't emitted
        // a new progress event yet (e.g., during brief stalls). This makes ETA
        // reflect elapsed time and avoids overly optimistic estimates.
        let progress = with_eta(&mut state, wallet_id, progress);
        state.progress.insert(wallet_id, progress.clone());
        progress
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_sync(
        &self,
        app_db: &AppDb,
        wallet_id: Uuid,
        network: Network,
        wallet_db_path: PathBuf,
        wallet_dek: Dek,
        account_ids: Vec<u32>,
        tor_manager: Option<std::sync::Arc<zbag_tor::TorManager>>,
        on_progress: Option<SyncEventHandler>,
        on_balance_changed: Option<BalanceEventHandler>,
    ) -> anyhow::Result<()> {
        let tor_state = tor_manager.as_ref().map(|tor| tor.state());
        tracing::info!(
            wallet_id = %wallet_id,
            network = ?network,
            tor_state = ?tor_state,
            "sync start requested"
        );

        {
            let mut state = self.state.lock().expect("mutex poisoned");
            if state.jobs.contains_key(&wallet_id) || state.blocking_scans.contains_key(&wallet_id)
            {
                return Err(ipc_err(
                    errors::SYNC_IN_PROGRESS,
                    "sync already in progress",
                ));
            }

            state.started_at.insert(wallet_id, Instant::now());
            state.progress.insert(
                wallet_id,
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
        }

        self.emit_progress(wallet_id, on_progress.as_ref());

        let grpc_url = server_resolver::resolve_grpc_url(app_db, network)
            .context("failed to resolve active lightwalletd endpoint")?;
        tracing::debug!(
            wallet_id = %wallet_id,
            network = ?network,
            grpc_url = %grpc_url,
            "sync resolved gRPC endpoint"
        );

        let (cancel_tx, cancel_rx) = watch::channel(false);
        let state = Arc::clone(&self.state);
        let on_progress_task = on_progress.clone();
        let on_balance_task = on_balance_changed.clone();

        let handle = crate::tokio_runtime::spawn(async move {
            tracing::debug!(wallet_id = %wallet_id, grpc_url = %grpc_url, "sync task started");

            // Wrap wallet_dek in Arc for sharing across concurrent enhancement tasks.
            // The Dek is only read (never mutated) during sync operations.
            let wallet_dek = Arc::new(wallet_dek);

            let client = match tor_manager {
                Some(ref tor) => {
                    zbag_network::grpc_client::GrpcClient::new_with_tor(grpc_url, Arc::clone(tor))
                }
                None => zbag_network::grpc_client::GrpcClient::new(grpc_url),
            };
            let mut balance_db = if on_balance_task.as_ref().is_some() {
                // Copy DEK bytes for the balance connection (original stays with sync)
                match open_wallet_db_async(wallet_db_path.clone(), wallet_dek.clone_key_material())
                    .await
                {
                    Ok(db) => Some(db),
                    Err(err) => {
                        tracing::debug!(
                            wallet_id = %wallet_id,
                            error = ?err,
                            "failed to open wallet db for balance updates"
                        );
                        None
                    }
                }
            } else {
                None
            };
            let mut balance_emit_throttle = BalanceEmitThrottle::default();
            let mut balance_emitter = BalanceEmitter {
                state: &state,
                wallet_id,
                on_balance_task: on_balance_task.as_ref(),
                balance_db: &mut balance_db,
                wallet_db_path: &wallet_db_path,
                wallet_dek: wallet_dek.as_ref(),
                network,
                account_ids: &account_ids,
            };

            // Wait for Tor to be ready if enabled but not connected
            if let Some(ref tor) = tor_manager {
                loop {
                    if *cancel_rx.borrow() {
                        tracing::debug!(wallet_id = %wallet_id, "sync cancelled while waiting for Tor");
                        balance_emitter
                            .maybe_emit(
                                &mut balance_emit_throttle,
                                BalanceEmitTrigger::ForcedMilestone,
                            )
                            .await;
                        let mut state = state.lock().expect("mutex poisoned");
                        state.jobs.remove(&wallet_id);
                        return;
                    }

                    let tor_state = tor.state();

                    // If Tor is disabled, proceed with direct connection
                    if !tor_state.enabled {
                        break;
                    }

                    // If Tor is ready, proceed
                    if tor_state.status == zbag_core::domain::TorStatus::On {
                        tracing::info!(wallet_id = %wallet_id, "Tor connected, starting sync");
                        break;
                    }

                    // If Tor is in error state, log and continue (will fail in main loop)
                    if tor_state.status == zbag_core::domain::TorStatus::Error {
                        tracing::warn!(wallet_id = %wallet_id, error = ?tor_state.last_error, "Tor in error state");
                        break;
                    }

                    // Tor is connecting - wait silently
                    tracing::debug!(wallet_id = %wallet_id, status = ?tor_state.status, "waiting for Tor");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }

            let emit = |progress: SyncProgress| {
                if let Some(handler) = on_progress_task.as_ref() {
                    handler(SyncProgressEvent {
                        schema_version: SCHEMA_VERSION,
                        event: "sync.progress".to_string(),
                        progress: progress.clone(),
                    });
                }
            };

            let update = |progress: SyncProgress| {
                let mut state = state.lock().expect("mutex poisoned");
                let progress = with_eta(&mut state, wallet_id, progress);
                state.progress.insert(wallet_id, progress.clone());
                drop(state);
                emit(progress);
            };

            // === Phase: Preparing ===
            update(SyncProgress {
                phase: SyncPhase::Preparing,
                scan_frontier_height: 0,
                wallet_tip_height: 0,
                progress_percent: 0,
                eta_seconds: None,
                retry_in_seconds: None,
                error_message: None,
            });

            // Check cancellation
            if *cancel_rx.borrow() {
                balance_emitter
                    .maybe_emit(
                        &mut balance_emit_throttle,
                        BalanceEmitTrigger::ForcedMilestone,
                    )
                    .await;
                update(default_progress());
                // Clear job and return early
                let mut state = state.lock().expect("mutex poisoned");
                state.jobs.remove(&wallet_id);
                state.started_at.remove(&wallet_id);
                tracing::debug!(wallet_id = %wallet_id, "sync cancelled during prepare");
                return;
            }

            // Chain tip is fetched in the catch-up loop below.

            // Initialize block cache directory (on blocking thread)
            let cache_dir = wallet_db_path
                .parent()
                .unwrap_or(&wallet_db_path)
                .join("block_cache");
            if let Err(err) = create_cache_dir_async(cache_dir.clone()).await {
                tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to create block cache dir");
                balance_emitter
                    .maybe_emit(
                        &mut balance_emit_throttle,
                        BalanceEmitTrigger::ForcedMilestone,
                    )
                    .await;
                update(default_progress());
                let mut state = state.lock().expect("mutex poisoned");
                state.jobs.remove(&wallet_id);
                state.started_at.remove(&wallet_id);
                return;
            }

            // Initialize FsBlockDb (on blocking thread)
            let mut fsblock_db = match init_fsblock_db_async(cache_dir.clone(), wallet_id).await {
                Ok(db) => db,
                Err(err) => {
                    tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to init FsBlockDb");
                    balance_emitter
                        .maybe_emit(
                            &mut balance_emit_throttle,
                            BalanceEmitTrigger::ForcedMilestone,
                        )
                        .await;
                    update(default_progress());
                    let mut state = state.lock().expect("mutex poisoned");
                    state.jobs.remove(&wallet_id);
                    state.started_at.remove(&wallet_id);
                    return;
                }
            };

            // Open wallet DB for sync operations (on blocking thread)
            let mut sync_wallet_conn = match open_wallet_db_async(
                wallet_db_path.clone(),
                wallet_dek.clone_key_material(),
            )
            .await
            {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to open wallet db for sync");
                    balance_emitter
                        .maybe_emit(
                            &mut balance_emit_throttle,
                            BalanceEmitTrigger::ForcedMilestone,
                        )
                        .await;
                    update(default_progress());
                    let mut state = state.lock().expect("mutex poisoned");
                    state.jobs.remove(&wallet_id);
                    state.started_at.remove(&wallet_id);
                    return;
                }
            };

            // Backfill account birthday tree sizes from lightwalletd (required for accurate
            // output-based progress ratios in WalletSummary).
            let (conn, backfill_result) =
                match backfill_birthday_tree_sizes(sync_wallet_conn, &client, wallet_id).await {
                    Ok(result) => result,
                    Err(err) => {
                        tracing::error!(
                            wallet_id = %wallet_id,
                            error = ?err,
                            "backfill worker failed"
                        );
                        update(SyncProgress {
                            phase: SyncPhase::Error,
                            scan_frontier_height: 0,
                            wallet_tip_height: 0,
                            progress_percent: 0,
                            eta_seconds: None,
                            retry_in_seconds: None,
                            error_message: Some("Sync worker failed".to_string()),
                        });
                        balance_emitter
                            .maybe_emit(
                                &mut balance_emit_throttle,
                                BalanceEmitTrigger::ForcedMilestone,
                            )
                            .await;
                        let mut state = state.lock().expect("mutex poisoned");
                        state.jobs.remove(&wallet_id);
                        state.started_at.remove(&wallet_id);
                        return;
                    }
                };
            sync_wallet_conn = conn;
            if let Err(err) = backfill_result {
                tracing::warn!(
                    wallet_id = %wallet_id,
                    error = ?err,
                    "failed to backfill account birthday tree sizes; progress percent may be inaccurate"
                );
            }

            let params = zcash_consensus_network(network);

            let mut poll_backoff = POLL_INTERVAL;
            // When entering Offline/Error backoff, emit `retry_in_seconds` before sleeping so the UI
            // can show a countdown based on the full backoff duration.

            // === Persistent sync loop ===
            // Note: We pass sync_wallet_conn and fsblock_db in/out of blocking operations
            // to keep SQLite work off the async runtime.
            'auto_sync: loop {
                balance_emit_throttle.start_new_auto_sync_iteration();

                // Check cancellation at start of each iteration
                if *cancel_rx.borrow() {
                    tracing::debug!(wallet_id = %wallet_id, "sync cancelled");
                    balance_emitter
                        .maybe_emit(
                            &mut balance_emit_throttle,
                            BalanceEmitTrigger::ForcedMilestone,
                        )
                        .await;
                    update(default_progress());
                    break 'auto_sync;
                }

                // Refresh chain tip (retry with backoff on transient errors).
                let chain_tip = match client.get_latest_block().await {
                    Ok((height, _hash)) => {
                        poll_backoff = POLL_INTERVAL;
                        height
                    }
                    Err(err) => {
                        tracing::warn!(wallet_id = %wallet_id, error = ?err, "failed to get chain tip");
                        // Get progress on blocking thread
                        let (conn, progress_percent, fully_scanned, chain_height) =
                            match get_progress_blocking(sync_wallet_conn, params).await {
                                Ok(result) => result,
                                Err(join_err) => {
                                    tracing::error!(
                                        wallet_id = %wallet_id,
                                        error = ?join_err,
                                        "sync progress worker failed"
                                    );
                                    update(SyncProgress {
                                        phase: SyncPhase::Error,
                                        scan_frontier_height: 0,
                                        wallet_tip_height: 0,
                                        progress_percent: 0,
                                        eta_seconds: None,
                                        retry_in_seconds: None,
                                        error_message: Some("Sync worker failed".to_string()),
                                    });
                                    break 'auto_sync;
                                }
                            };
                        sync_wallet_conn = conn;
                        let wallet_tip_height = chain_height.map(u32::from).unwrap_or(0);
                        let retry_in_seconds = poll_backoff.as_secs();
                        update(SyncProgress {
                            phase: SyncPhase::Offline,
                            scan_frontier_height: fully_scanned,
                            wallet_tip_height,
                            progress_percent,
                            eta_seconds: None,
                            retry_in_seconds: Some(retry_in_seconds),
                            error_message: None,
                        });

                        tokio::time::sleep(poll_backoff).await;
                        poll_backoff = poll_backoff
                            .checked_mul(2)
                            .unwrap_or(MAX_POLL_BACKOFF)
                            .min(MAX_POLL_BACKOFF);
                        continue 'auto_sync;
                    }
                };

                tracing::debug!(
                    wallet_id = %wallet_id,
                    chain_tip = %u32::from(chain_tip),
                    "got chain tip"
                );

                // Update chain tip in wallet on blocking thread (retry with backoff on transient errors).
                let (conn, update_result) =
                    match update_chain_tip_blocking(sync_wallet_conn, params, chain_tip).await {
                        Ok(result) => result,
                        Err(join_err) => {
                            tracing::error!(
                                wallet_id = %wallet_id,
                                error = ?join_err,
                                "update_chain_tip worker failed"
                            );
                            update(SyncProgress {
                                phase: SyncPhase::Error,
                                scan_frontier_height: 0,
                                wallet_tip_height: u32::from(chain_tip),
                                progress_percent: 0,
                                eta_seconds: None,
                                retry_in_seconds: None,
                                error_message: Some("Sync worker failed".to_string()),
                            });
                            break 'auto_sync;
                        }
                    };
                sync_wallet_conn = conn;
                if let Err(err) = update_result {
                    tracing::warn!(
                        wallet_id = %wallet_id,
                        error = %err,
                        "failed to update chain tip"
                    );
                    let (conn, progress_percent, fully_scanned, _) =
                        match get_progress_blocking(sync_wallet_conn, params).await {
                            Ok(result) => result,
                            Err(join_err) => {
                                tracing::error!(
                                    wallet_id = %wallet_id,
                                    error = ?join_err,
                                    "sync progress worker failed"
                                );
                                update(SyncProgress {
                                    phase: SyncPhase::Error,
                                    scan_frontier_height: 0,
                                    wallet_tip_height: u32::from(chain_tip),
                                    progress_percent: 0,
                                    eta_seconds: None,
                                    retry_in_seconds: None,
                                    error_message: Some("Sync worker failed".to_string()),
                                });
                                break 'auto_sync;
                            }
                        };
                    sync_wallet_conn = conn;
                    let retry_in_seconds = poll_backoff.as_secs();
                    update(SyncProgress {
                        phase: SyncPhase::Error,
                        scan_frontier_height: fully_scanned,
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent,
                        eta_seconds: None,
                        retry_in_seconds: Some(retry_in_seconds),
                        error_message: Some("Failed to update wallet chain tip".to_string()),
                    });

                    tokio::time::sleep(poll_backoff).await;
                    poll_backoff = poll_backoff
                        .checked_mul(2)
                        .unwrap_or(MAX_POLL_BACKOFF)
                        .min(MAX_POLL_BACKOFF);
                    continue 'auto_sync;
                }

                // === Main sync loop (one pass to tip) ===
                let mut sync_complete = false;
                let mut sync_error = false;
                let mut sync_error_message: Option<String> = None;
                'sync_loop: loop {
                    // Check cancellation at start of each iteration
                    if *cancel_rx.borrow() {
                        tracing::debug!(wallet_id = %wallet_id, "sync cancelled");
                        balance_emitter
                            .maybe_emit(
                                &mut balance_emit_throttle,
                                BalanceEmitTrigger::ForcedMilestone,
                            )
                            .await;
                        update(default_progress());
                        break 'auto_sync;
                    }

                    // Get suggested scan ranges on blocking thread
                    let (conn, ranges_result) =
                        match suggest_scan_ranges_blocking(sync_wallet_conn, params).await {
                            Ok(result) => result,
                            Err(join_err) => {
                                tracing::error!(
                                    wallet_id = %wallet_id,
                                    error = ?join_err,
                                    "suggest_scan_ranges worker failed"
                                );
                                update(SyncProgress {
                                    phase: SyncPhase::Error,
                                    scan_frontier_height: 0,
                                    wallet_tip_height: u32::from(chain_tip),
                                    progress_percent: 0,
                                    eta_seconds: None,
                                    retry_in_seconds: None,
                                    error_message: Some("Sync worker failed".to_string()),
                                });
                                break 'auto_sync;
                            }
                        };
                    sync_wallet_conn = conn;
                    let ranges = match ranges_result {
                        Ok(ranges) => ranges,
                        Err(err) => {
                            tracing::error!(
                                wallet_id = %wallet_id,
                                error = %err,
                                "failed to get scan ranges"
                            );
                            sync_error_message =
                                Some("Failed to determine scan ranges".to_string());
                            sync_error = true;
                            break 'sync_loop;
                        }
                    };

                    if ranges.is_empty() {
                        tracing::debug!(
                            wallet_id = %wallet_id,
                            "no more ranges to scan, sync complete"
                        );
                        sync_complete = true;
                        break 'sync_loop;
                    }

                    for range in ranges {
                        // Check cancellation before each range
                        if *cancel_rx.borrow() {
                            tracing::debug!(
                                wallet_id = %wallet_id,
                                "sync cancelled during range processing"
                            );
                            balance_emitter
                                .maybe_emit(
                                    &mut balance_emit_throttle,
                                    BalanceEmitTrigger::ForcedMilestone,
                                )
                                .await;
                            update(default_progress());
                            break 'auto_sync;
                        }

                        let range_start = range.block_range().start;
                        let range_end = range.block_range().end;
                        let priority = range.priority();

                        tracing::debug!(
                            wallet_id = %wallet_id,
                            range_start = %u32::from(range_start),
                            range_end = %u32::from(range_end),
                            priority = ?priority,
                            "processing scan range"
                        );

                        // Skip if this is a low priority range that shouldn't block sync
                        if priority == ScanPriority::Ignored {
                            continue;
                        }

                        // Get wallet tip on blocking thread
                        let (conn, _, _, chain_height) =
                            match get_progress_blocking(sync_wallet_conn, params).await {
                                Ok(result) => result,
                                Err(join_err) => {
                                    tracing::error!(
                                        wallet_id = %wallet_id,
                                        error = ?join_err,
                                        "sync progress worker failed"
                                    );
                                    update(SyncProgress {
                                        phase: SyncPhase::Error,
                                        scan_frontier_height: 0,
                                        wallet_tip_height: u32::from(chain_tip),
                                        progress_percent: 0,
                                        eta_seconds: None,
                                        retry_in_seconds: None,
                                        error_message: Some("Sync worker failed".to_string()),
                                    });
                                    break 'auto_sync;
                                }
                            };
                        sync_wallet_conn = conn;
                        let wallet_tip = chain_height.unwrap_or_else(|| {
                            tracing::debug!("chain height unavailable for progress calculation");
                            range_start
                        });

                        // === Pipelined download and scan ===
                        // Create channel for downloaded batches
                        let (batch_tx, mut batch_rx) =
                            mpsc::channel::<DownloadResult>(LOOKAHEAD_BATCHES);
                        let batch_controller =
                            Arc::new(Mutex::new(AdaptiveBatchController::new(range_start)));

                        // Clone what the download task needs
                        let download_client = client.clone();
                        let download_cancel_rx = cancel_rx.clone();
                        let download_wallet_id = wallet_id;
                        let download_batch_controller = Arc::clone(&batch_controller);

                        // Spawn download task
                        let download_handle = crate::tokio_runtime::spawn(async move {
                            let mut current = range_start;
                            while current < range_end {
                                // Check cancellation
                                if *download_cancel_rx.borrow() {
                                    tracing::debug!(
                                        wallet_id = %download_wallet_id,
                                        "download task cancelled"
                                    );
                                    let _ = batch_tx.send(DownloadResult::Cancelled).await;
                                    return;
                                }

                                let batch_size = download_batch_controller
                                    .lock()
                                    .expect("mutex poisoned")
                                    .next_download_size(current);
                                let batch_end = std::cmp::min(current + batch_size, range_end);

                                // Download compact blocks with retry
                                match download_blocks_with_retry(
                                    &download_client,
                                    current,
                                    batch_end,
                                    5,
                                )
                                .await
                                {
                                    Ok(blocks) => {
                                        tracing::debug!(
                                            wallet_id = %download_wallet_id,
                                            blocks_downloaded = blocks.len(),
                                            range = format!("{}..{}", u32::from(current), u32::from(batch_end)),
                                            "downloaded blocks"
                                        );

                                        let batch = DownloadedBatch {
                                            range_start: current,
                                            range_end: batch_end,
                                            blocks,
                                        };

                                        // Send batch through channel (will block if channel is full, providing backpressure)
                                        if batch_tx
                                            .send(DownloadResult::Batch(batch))
                                            .await
                                            .is_err()
                                        {
                                            // Receiver dropped, scan loop exited early
                                            tracing::debug!(
                                                wallet_id = %download_wallet_id,
                                                "batch receiver dropped, stopping download"
                                            );
                                            return;
                                        }
                                    }
                                    Err(err) => {
                                        tracing::warn!(
                                            wallet_id = %download_wallet_id,
                                            start = %u32::from(current),
                                            end = %u32::from(batch_end),
                                            error = ?err,
                                            "failed to download blocks after retries"
                                        );
                                        let _ = batch_tx.send(DownloadResult::Error(err)).await;
                                        return;
                                    }
                                }

                                current = batch_end;
                            }

                            // Signal download complete for this range
                            let _ = batch_tx.send(DownloadResult::RangeComplete).await;
                        });

                        // Update phase to downloading/scanning (pipelined)
                        // Get progress on blocking thread
                        let (conn, progress_percent, fully_scanned, _) =
                            match get_progress_blocking(sync_wallet_conn, params).await {
                                Ok(result) => result,
                                Err(join_err) => {
                                    tracing::error!(
                                        wallet_id = %wallet_id,
                                        error = ?join_err,
                                        "sync progress worker failed"
                                    );
                                    update(SyncProgress {
                                        phase: SyncPhase::Error,
                                        scan_frontier_height: 0,
                                        wallet_tip_height: u32::from(chain_tip),
                                        progress_percent: 0,
                                        eta_seconds: None,
                                        retry_in_seconds: None,
                                        error_message: Some("Sync worker failed".to_string()),
                                    });
                                    break 'auto_sync;
                                }
                            };
                        sync_wallet_conn = conn;
                        let initial_frontier =
                            fully_scanned.max(u32::from(range_start.saturating_sub(1)));
                        update(SyncProgress {
                            phase: SyncPhase::Downloading,
                            scan_frontier_height: initial_frontier,
                            wallet_tip_height: u32::from(wallet_tip),
                            progress_percent,
                            eta_seconds: None,
                            retry_in_seconds: None,
                            error_message: None,
                        });

                        // === Fetch tree state ONCE at the start of the range ===
                        // This is a major optimization: instead of fetching tree state for every
                        // 100-block batch (34,700 RPC calls for initial sync), we fetch it once
                        // per range. The scanner maintains internal state between batches.
                        let range_prior_height = range_start.saturating_sub(1);
                        let range_chain_state =
                            match fetch_chain_state(&client, range_prior_height, wallet_id).await {
                                Ok(state) => state,
                                Err(err) => {
                                    tracing::error!(
                                        wallet_id = %wallet_id,
                                        height = %u32::from(range_prior_height),
                                        error = ?err,
                                        "tree state fetch failed, aborting sync"
                                    );
                                    sync_error_message =
                                        Some("Failed to fetch chain state".to_string());
                                    sync_error = true;
                                    break 'sync_loop;
                                }
                            };

                        // === Main scan loop - receives batches and scans them ===
                        let blocks_dir = cache_dir.join("blocks");
                        let mut range_error = false;
                        let mut range_cancelled = false;
                        let mut is_first_batch_in_range = true;

                        while let Some(result) = batch_rx.recv().await {
                            match result {
                                DownloadResult::Batch(mut batch) => {
                                    // Check cancellation
                                    if *cancel_rx.borrow() {
                                        tracing::debug!(
                                            wallet_id = %wallet_id,
                                            "sync cancelled during scan"
                                        );
                                        range_cancelled = true;
                                        break;
                                    }

                                    // Write blocks to cache on blocking thread
                                    let (db, block_metas) = match write_blocks_to_cache_async(
                                        blocks_dir.clone(),
                                        std::mem::take(&mut batch.blocks),
                                        fsblock_db,
                                        wallet_id,
                                    )
                                    .await
                                    {
                                        Ok(result) => result,
                                        Err(err) => {
                                            tracing::error!(
                                                wallet_id = %wallet_id,
                                                error = ?err,
                                                "write_blocks_to_cache worker failed"
                                            );
                                            update(SyncProgress {
                                                phase: SyncPhase::Error,
                                                scan_frontier_height: 0,
                                                wallet_tip_height: u32::from(chain_tip),
                                                progress_percent: 0,
                                                eta_seconds: None,
                                                retry_in_seconds: None,
                                                error_message: Some(
                                                    "Sync worker failed".to_string(),
                                                ),
                                            });
                                            break 'auto_sync;
                                        }
                                    };
                                    fsblock_db = db;

                                    // Scan the batch immediately after caching
                                    // Fetch tree state for each batch to avoid CheckpointConflict
                                    // with existing wallet data during incremental syncs.
                                    let prior_height = batch.range_start.saturating_sub(1);
                                    let chain_state = if is_first_batch_in_range {
                                        is_first_batch_in_range = false;
                                        range_chain_state.clone()
                                    } else {
                                        match fetch_chain_state(&client, prior_height, wallet_id)
                                            .await
                                        {
                                            Ok(state) => state,
                                            Err(err) => {
                                                tracing::error!(
                                                    wallet_id = %wallet_id,
                                                    height = %u32::from(prior_height),
                                                    error = ?err,
                                                    "tree state fetch failed for batch, aborting range"
                                                );
                                                sync_error_message =
                                                    Some("Failed to fetch chain state".to_string());
                                                range_error = true;
                                                break;
                                            }
                                        }
                                    };
                                    // Scan blocks on blocking thread
                                    let scan_started_at = Instant::now();
                                    let scan_result = match scan_batch_blocking(
                                        sync_wallet_conn,
                                        fsblock_db,
                                        params,
                                        batch.range_start,
                                        chain_state,
                                        block_metas,
                                        blocks_dir.clone(),
                                        batch.range_end,
                                        Arc::clone(&state),
                                        wallet_id,
                                    )
                                    .await
                                    {
                                        Ok(result) => result,
                                        Err(err) => {
                                            tracing::error!(
                                                wallet_id = %wallet_id,
                                                error = ?err,
                                                "scan_batch worker failed"
                                            );
                                            update(SyncProgress {
                                                phase: SyncPhase::Error,
                                                scan_frontier_height: 0,
                                                wallet_tip_height: u32::from(chain_tip),
                                                progress_percent: 0,
                                                eta_seconds: None,
                                                retry_in_seconds: None,
                                                error_message: Some(
                                                    "Sync worker failed".to_string(),
                                                ),
                                            });
                                            break 'auto_sync;
                                        }
                                    };
                                    let scan_elapsed = scan_started_at.elapsed();

                                    let ScanBatchResult {
                                        conn: updated_conn,
                                        fsblock_db: updated_fsblock_db,
                                        scan_outcome,
                                        scan_activity,
                                        progress_percent,
                                        fully_scanned,
                                    } = scan_result;

                                    // Restore ownership
                                    sync_wallet_conn = updated_conn;
                                    fsblock_db = updated_fsblock_db;

                                    match scan_outcome {
                                        ScanOutcome::Success => {
                                            if let Some(adjustment) = batch_controller
                                                .lock()
                                                .expect("mutex poisoned")
                                                .record_scan_result(
                                                    batch.range_start,
                                                    scan_elapsed,
                                                    scan_activity,
                                                )
                                            {
                                                let decision = match adjustment.decision {
                                                    AdaptiveBatchDecision::Grew => "grew",
                                                    AdaptiveBatchDecision::Shrunk => "shrunk",
                                                };
                                                tracing::debug!(
                                                    wallet_id = %wallet_id,
                                                    range_start = %u32::from(batch.range_start),
                                                    previous_batch_size = adjustment.previous,
                                                    new_batch_size = adjustment.current,
                                                    decision,
                                                    scan_elapsed_ms = scan_elapsed.as_millis(),
                                                    touched_notes = scan_activity.touched_notes(),
                                                    "adaptive sync batch size updated"
                                                );
                                            }

                                            // Update progress after scan
                                            update(SyncProgress {
                                                phase: SyncPhase::Scanning,
                                                scan_frontier_height: fully_scanned,
                                                wallet_tip_height: u32::from(wallet_tip),
                                                progress_percent,
                                                eta_seconds: None,
                                                retry_in_seconds: None,
                                                error_message: None,
                                            });

                                            balance_emitter
                                                .maybe_emit(
                                                    &mut balance_emit_throttle,
                                                    BalanceEmitTrigger::SuccessfulScanBatch {
                                                        scanned_blocks: scan_activity
                                                            .scanned_blocks,
                                                    },
                                                )
                                                .await;
                                        }
                                        ScanOutcome::ReorgDetected { rewind_height } => {
                                            tracing::debug!(
                                                wallet_id = %wallet_id,
                                                rewind_height = %u32::from(rewind_height),
                                                "reorg recovery complete, refetching scan ranges"
                                            );
                                            // Break to re-fetch scan ranges with the rewound state.
                                            // Don't set range_error - this is a recoverable situation.
                                            break;
                                        }
                                        ScanOutcome::Error(err) => {
                                            sync_error_message =
                                                Some(format!("Failed to scan blocks: {err}"));
                                            range_error = true;
                                            break;
                                        }
                                    }
                                }
                                DownloadResult::RangeComplete => {
                                    tracing::debug!(
                                        wallet_id = %wallet_id,
                                        "range download complete"
                                    );
                                    break;
                                }
                                DownloadResult::Error(_err) => {
                                    sync_error_message =
                                        Some("Failed to download blocks".to_string());
                                    range_error = true;
                                    break;
                                }
                                DownloadResult::Cancelled => {
                                    range_cancelled = true;
                                    break;
                                }
                            }
                        }

                        // Wait for download task to complete
                        if let Err(e) = download_handle.await
                            && e.is_panic()
                        {
                            tracing::error!(error = ?e, "download task panicked");
                        }

                        if range_cancelled {
                            balance_emitter
                                .maybe_emit(
                                    &mut balance_emit_throttle,
                                    BalanceEmitTrigger::ForcedMilestone,
                                )
                                .await;
                            update(default_progress());
                            break 'auto_sync;
                        }

                        if range_error {
                            sync_error = true;
                            break 'sync_loop;
                        }

                        // Final progress update for the range (on blocking thread)
                        let (conn, progress_percent, fully_scanned, _) =
                            match get_progress_blocking(sync_wallet_conn, params).await {
                                Ok(result) => result,
                                Err(join_err) => {
                                    tracing::error!(
                                        wallet_id = %wallet_id,
                                        error = ?join_err,
                                        "sync progress worker failed"
                                    );
                                    update(SyncProgress {
                                        phase: SyncPhase::Error,
                                        scan_frontier_height: 0,
                                        wallet_tip_height: u32::from(chain_tip),
                                        progress_percent: 0,
                                        eta_seconds: None,
                                        retry_in_seconds: None,
                                        error_message: Some("Sync worker failed".to_string()),
                                    });
                                    break 'auto_sync;
                                }
                            };
                        sync_wallet_conn = conn;
                        update(SyncProgress {
                            phase: SyncPhase::Scanning,
                            scan_frontier_height: fully_scanned,
                            wallet_tip_height: u32::from(wallet_tip),
                            progress_percent,
                            eta_seconds: None,
                            retry_in_seconds: None,
                            error_message: None,
                        });
                    }
                }

                if sync_error {
                    let (conn, progress_percent, fully_scanned, _) =
                        match get_progress_blocking(sync_wallet_conn, params).await {
                            Ok(result) => result,
                            Err(join_err) => {
                                tracing::error!(
                                    wallet_id = %wallet_id,
                                    error = ?join_err,
                                    "sync progress worker failed during error handling"
                                );
                                update(SyncProgress {
                                    phase: SyncPhase::Error,
                                    scan_frontier_height: 0,
                                    wallet_tip_height: u32::from(chain_tip),
                                    progress_percent: 0,
                                    eta_seconds: None,
                                    retry_in_seconds: None,
                                    error_message: Some("Sync worker failed".to_string()),
                                });
                                break 'auto_sync;
                            }
                        };
                    sync_wallet_conn = conn;
                    let retry_in_seconds = poll_backoff.as_secs();
                    update(SyncProgress {
                        phase: SyncPhase::Error,
                        scan_frontier_height: fully_scanned,
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent,
                        eta_seconds: None,
                        retry_in_seconds: Some(retry_in_seconds),
                        error_message: sync_error_message,
                    });
                    balance_emitter
                        .maybe_emit(
                            &mut balance_emit_throttle,
                            BalanceEmitTrigger::ForcedMilestone,
                        )
                        .await;

                    tokio::time::sleep(poll_backoff).await;
                    poll_backoff = poll_backoff
                        .checked_mul(2)
                        .unwrap_or(MAX_POLL_BACKOFF)
                        .min(MAX_POLL_BACKOFF);
                    continue 'auto_sync;
                }

                if sync_complete {
                    // === Phase: Enhancing ===
                    // Fetch full transactions to extract memos for received notes.
                    // Compact blocks don't contain memo data, so we need to fetch
                    // the full transaction and decrypt it to get memos.

                    // Get total count for progress tracking.
                    let total_to_enhance = match count_txids_needing_memo_enhancement_blocking(
                        wallet_db_path.clone(),
                        wallet_dek.clone_key_material(),
                    )
                    .await
                    {
                        Ok(count) => count,
                        Err(err) => {
                            tracing::warn!(
                                wallet_id = %wallet_id,
                                error = ?err,
                                "failed to count transactions needing memo enhancement, skipping enhancement phase"
                            );
                            0
                        }
                    };

                    // Emit initial enhancing progress.
                    let enhancement_progress = |enhanced: u32, total: u32| -> u8 {
                        if total == 0 {
                            return 100;
                        }
                        // Map enhancement progress to 99-100 range.
                        let ratio = enhanced as f64 / total as f64;
                        (99.0 + ratio).min(100.0) as u8
                    };
                    balance_emitter
                        .maybe_emit(
                            &mut balance_emit_throttle,
                            BalanceEmitTrigger::ForcedMilestone,
                        )
                        .await;

                    update(SyncProgress {
                        phase: SyncPhase::Enhancing,
                        scan_frontier_height: u32::from(chain_tip),
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent: enhancement_progress(0, total_to_enhance),
                        eta_seconds: None,
                        retry_in_seconds: None,
                        error_message: None,
                    });

                    if total_to_enhance > 0 {
                        // Use concurrent enhancement with bounded parallelism.
                        // GrpcClient uses HTTP/2 multiplexing, and SQLite has busy_timeout
                        // configured, so moderate concurrency is safe.
                        const ENHANCEMENT_CONCURRENCY: usize = 4;
                        let mut join_set = tokio::task::JoinSet::new();
                        let mut enhanced_count: u32 = 0;
                        let mut batch_offset: u32 = 0;

                        // Process transactions in batches to limit memory usage.
                        'enhancement: loop {
                            // Check cancellation before fetching next batch.
                            if *cancel_rx.borrow() {
                                break 'enhancement;
                            }

                            // Fetch next batch of txids.
                            let txids_batch =
                                match get_txids_needing_memo_enhancement_batch_blocking(
                                    wallet_db_path.clone(),
                                    wallet_dek.clone_key_material(),
                                    batch_offset,
                                    ENHANCEMENT_BATCH_SIZE,
                                )
                                .await
                                {
                                    Ok(batch) => batch,
                                    Err(err) => {
                                        tracing::warn!(
                                            wallet_id = %wallet_id,
                                            batch_offset,
                                            error = ?err,
                                            "failed to get memo enhancement batch"
                                        );
                                        break 'enhancement;
                                    }
                                };

                            // If batch is empty, we're done.
                            if txids_batch.is_empty() {
                                break 'enhancement;
                            }

                            let batch_size = txids_batch.len() as u32;
                            batch_offset += batch_size;

                            // Emit progress at start of each batch.
                            update(SyncProgress {
                                phase: SyncPhase::Enhancing,
                                scan_frontier_height: u32::from(chain_tip),
                                wallet_tip_height: u32::from(chain_tip),
                                progress_percent: enhancement_progress(
                                    enhanced_count,
                                    total_to_enhance,
                                ),
                                eta_seconds: None,
                                retry_in_seconds: None,
                                error_message: None,
                            });

                            for txid_bytes in txids_batch {
                                // Check cancellation before spawning new tasks.
                                if *cancel_rx.borrow() {
                                    break 'enhancement;
                                }

                                // Limit concurrency by draining completed tasks.
                                while join_set.len() >= ENHANCEMENT_CONCURRENCY {
                                    if let Some(result) = join_set.join_next().await {
                                        enhanced_count += 1;
                                        match result {
                                            Ok(Ok(())) => {}
                                            Ok(Err((txid, err))) => {
                                                tracing::warn!(
                                                    wallet_id = %wallet_id,
                                                    txid = hex::encode(txid),
                                                    error = ?err,
                                                    "failed to enhance transaction memo"
                                                );
                                            }
                                            Err(join_err) => {
                                                tracing::warn!(
                                                    wallet_id = %wallet_id,
                                                    error = ?join_err,
                                                    "enhancement task panicked"
                                                );
                                            }
                                        }
                                    }
                                }

                                // Clone values for the spawned task.
                                let client = client.clone();
                                let wallet_db_path = wallet_db_path.clone();
                                let wallet_dek = Arc::clone(&wallet_dek);

                                join_set.spawn(async move {
                                    match enhance_transaction_memo(
                                        &client,
                                        &wallet_db_path,
                                        &wallet_dek,
                                        &params,
                                        txid_bytes,
                                    )
                                    .await
                                    {
                                        Ok(()) => {
                                            tracing::debug!(
                                                txid = hex::encode(txid_bytes),
                                                "enhanced transaction memo"
                                            );
                                            Ok(())
                                        }
                                        Err(err) => Err((txid_bytes, err)),
                                    }
                                });
                            }
                        }

                        // Drain remaining tasks.
                        while let Some(result) = join_set.join_next().await {
                            enhanced_count += 1;
                            match result {
                                Ok(Ok(())) => {}
                                Ok(Err((txid, err))) => {
                                    tracing::warn!(
                                        wallet_id = %wallet_id,
                                        txid = hex::encode(txid),
                                        error = ?err,
                                        "failed to enhance transaction memo"
                                    );
                                }
                                Err(join_err) => {
                                    tracing::warn!(
                                        wallet_id = %wallet_id,
                                        error = ?join_err,
                                        "enhancement task panicked"
                                    );
                                }
                            }
                        }

                        tracing::debug!(
                            wallet_id = %wallet_id,
                            enhanced_count,
                            total_to_enhance,
                            "memo enhancement phase complete"
                        );
                    }

                    // Final progress update
                    update(SyncProgress {
                        phase: SyncPhase::CatchingUp,
                        scan_frontier_height: u32::from(chain_tip),
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent: 100,
                        eta_seconds: None,
                        retry_in_seconds: None,
                        error_message: None,
                    });
                    balance_emitter
                        .maybe_emit(
                            &mut balance_emit_throttle,
                            BalanceEmitTrigger::ForcedMilestone,
                        )
                        .await;
                }

                tokio::time::sleep(POLL_INTERVAL).await;
            }

            // Clean up block cache directory (on blocking thread)
            remove_cache_dir_async(cache_dir).await;

            // Clear job entry (best effort).
            let mut state = state.lock().expect("mutex poisoned");
            state.jobs.remove(&wallet_id);
            state.started_at.remove(&wallet_id);
            state.progress_estimates.remove(&wallet_id);

            tracing::debug!(wallet_id = %wallet_id, "sync task finished");
        });

        let finished = handle.is_finished();
        self.state.lock().expect("mutex poisoned").jobs.insert(
            wallet_id,
            SyncJob {
                cancel: cancel_tx,
                handle,
            },
        );
        if finished {
            self.state
                .lock()
                .expect("mutex poisoned")
                .jobs
                .remove(&wallet_id);
        }

        Ok(())
    }

    pub fn stop_sync(
        &self,
        wallet_id: Uuid,
        on_progress: Option<SyncEventHandler>,
    ) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("mutex poisoned");
        let job = state.jobs.remove(&wallet_id);

        let Some(job) = job else {
            return Ok(());
        };

        let _ = job.cancel.send(true);
        job.handle.abort();

        state.progress.insert(wallet_id, default_progress());
        state.started_at.remove(&wallet_id);
        state.progress_estimates.remove(&wallet_id);
        drop(state);

        self.emit_progress(wallet_id, on_progress.as_ref());
        Ok(())
    }

    /// Returns the IDs of all wallets with currently running sync jobs.
    pub fn running_wallet_ids(&self) -> Vec<Uuid> {
        self.state
            .lock()
            .expect("mutex poisoned")
            .jobs
            .keys()
            .copied()
            .collect()
    }

    fn emit_progress(&self, wallet_id: Uuid, handler: Option<&SyncEventHandler>) {
        let Some(handler) = handler else { return };
        let progress = self.get_progress(wallet_id);
        handler(SyncProgressEvent {
            schema_version: SCHEMA_VERSION,
            event: "sync.progress".to_string(),
            progress,
        });
    }
}

impl Default for SyncService {
    fn default() -> Self {
        Self::new()
    }
}

fn record_balance(
    state: &Arc<Mutex<State>>,
    wallet_id: Uuid,
    account_id: u32,
    balance: &Balance,
) -> bool {
    let mut state = state.lock().expect("mutex poisoned");
    match state.balances.get(&(wallet_id, account_id)) {
        Some(existing) if existing == balance => false,
        _ => {
            state
                .balances
                .insert((wallet_id, account_id), balance.clone());
            true
        }
    }
}

fn emit_balance_events(
    state: &Arc<Mutex<State>>,
    wallet_id: Uuid,
    handler: &BalanceEventHandler,
    balances: Vec<(u32, Balance)>,
) {
    for (account_id, balance) in balances {
        if record_balance(state, wallet_id, account_id, &balance) {
            handler(BalanceChangedEvent {
                schema_version: SCHEMA_VERSION,
                event: "balance.changed".to_string(),
                account_id,
                balance,
            });
        }
    }
}

struct BalanceEmitter<'a> {
    state: &'a Arc<Mutex<State>>,
    wallet_id: Uuid,
    on_balance_task: Option<&'a BalanceEventHandler>,
    balance_db: &'a mut Option<Connection>,
    wallet_db_path: &'a PathBuf,
    wallet_dek: &'a Dek,
    network: Network,
    account_ids: &'a [u32],
}

impl BalanceEmitter<'_> {
    async fn maybe_emit(
        &mut self,
        throttle: &mut BalanceEmitThrottle,
        trigger: BalanceEmitTrigger,
    ) {
        let Some(handler) = self.on_balance_task else {
            return;
        };
        let now = Instant::now();
        if !throttle.should_emit_and_update(trigger, now) {
            return;
        }

        let db = match self.balance_db.take() {
            Some(db) => db,
            None => match open_wallet_db_async(
                self.wallet_db_path.clone(),
                self.wallet_dek.clone_key_material(),
            )
            .await
            {
                Ok(db) => db,
                Err(err) => {
                    tracing::warn!(
                        wallet_id = %self.wallet_id,
                        error = ?err,
                        "failed to open wallet db for balance updates"
                    );
                    return;
                }
            },
        };

        // Read balances on blocking thread, emit callbacks on async thread.
        match fetch_balances_blocking(db, self.network, self.account_ids.to_vec()).await {
            Ok((db, balances)) => {
                *self.balance_db = Some(db);
                emit_balance_events(self.state, self.wallet_id, handler, balances);
                throttle.record_emit(Instant::now());
            }
            Err(err) => {
                tracing::warn!(
                    wallet_id = %self.wallet_id,
                    error = ?err,
                    "failed to fetch balances on blocking worker"
                );
                match open_wallet_db_async(
                    self.wallet_db_path.clone(),
                    self.wallet_dek.clone_key_material(),
                )
                .await
                {
                    Ok(db) => {
                        *self.balance_db = Some(db);
                    }
                    Err(reopen_err) => {
                        tracing::warn!(
                            wallet_id = %self.wallet_id,
                            error = ?reopen_err,
                            "failed to reopen wallet db for balance updates"
                        );
                    }
                }
            }
        }
    }
}

fn open_wallet_db(wallet_db_path: &Path, dek: &Dek) -> anyhow::Result<Connection> {
    open_sqlcipher_db(
        wallet_db_path,
        dek,
        OpenSqlcipherOptions {
            create_if_missing: false,
            load_array_module: true,
        },
    )
}

/// Open wallet database on a blocking thread to avoid starving the async runtime.
async fn open_wallet_db_async(wallet_db_path: PathBuf, dek: Dek) -> anyhow::Result<Connection> {
    crate::tokio_runtime::spawn_blocking(move || open_wallet_db(&wallet_db_path, &dek))
        .await
        .context("spawn_blocking panicked")?
}

fn default_progress() -> SyncProgress {
    SyncProgress {
        phase: SyncPhase::Idle,
        scan_frontier_height: 0,
        wallet_tip_height: 0,
        progress_percent: 0,
        eta_seconds: None,
        retry_in_seconds: None,
        error_message: None,
    }
}

fn with_eta(state: &mut State, wallet_id: Uuid, mut progress: SyncProgress) -> SyncProgress {
    // Reset/clear ETA once syncing is done (or aborted) so we don't leak stale estimates.
    // For Offline/Error states, keep the retry_in_seconds but clear ETA.
    if matches!(
        progress.phase,
        SyncPhase::Idle | SyncPhase::Offline | SyncPhase::Error
    ) {
        progress.eta_seconds = None;
        state.progress_estimates.remove(&wallet_id);
        return progress;
    }

    // When caught up, we keep the job alive in CatchingUp, but reset ETA/progress
    // estimators so future tip advancement doesn't get stuck at 99% forever.
    if progress.phase == SyncPhase::CatchingUp
        && progress.wallet_tip_height > 0
        && progress.scan_frontier_height >= progress.wallet_tip_height
    {
        progress.progress_percent = 100;
        progress.eta_seconds = None;
        state.progress_estimates.remove(&wallet_id);
        return progress;
    }

    // These phases don't have a meaningful block frontier rate.
    if matches!(progress.phase, SyncPhase::Preparing | SyncPhase::Enhancing) {
        progress.eta_seconds = None;
        if let Some(estimate) = state.progress_estimates.get_mut(&wallet_id) {
            estimate.last_eta_seconds = None;
        }
        return progress;
    }

    let now = Instant::now();
    let estimate = state.progress_estimates.entry(wallet_id).or_default();
    let delta_t_for_clamp = estimate
        .last_update_at
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(1.0)
        .max(0.05);

    // Capture session start height (first observed non-zero frontier).
    if estimate.start_height.is_none() && progress.scan_frontier_height > 0 {
        estimate.start_height = Some(progress.scan_frontier_height);
        estimate.start_instant = Some(now);
    }

    // Track the highest observed target height for stable progress/ETA.
    if progress.wallet_tip_height > 0 {
        estimate.target_height = Some(
            estimate
                .target_height
                .unwrap_or(0)
                .max(progress.wallet_tip_height),
        );
    }

    // Compute height-based progress % (monotonic, stable), falling back to the precomputed
    // WalletSummary output-based progress when we don't have enough context yet.
    if let (Some(start), Some(target)) = (estimate.start_height, estimate.target_height)
        && target > start
        && progress.scan_frontier_height >= start
    {
        let done = progress.scan_frontier_height.saturating_sub(start) as u64;
        let total = target.saturating_sub(start) as u64;
        if total > 0 {
            let mut pct = ((done.saturating_mul(100)) / total) as u8;
            if pct >= 100 {
                pct = if progress.phase == SyncPhase::Idle {
                    100
                } else {
                    99
                };
            }

            if let Some(last) = estimate.last_percent {
                pct = pct.max(last);
            }
            estimate.last_percent = Some(pct);
            progress.progress_percent = pct;
        }
    }

    // ETA: use an EWMA of scan frontier movement (blocks/sec) and remaining blocks.
    let Some(target) = estimate.target_height else {
        progress.eta_seconds = None;
        return progress;
    };
    let frontier = progress.scan_frontier_height;
    if target == 0 || frontier == 0 || frontier >= target {
        progress.eta_seconds = None;
        return progress;
    }

    if let (Some(last_height), Some(last_update_at)) =
        (estimate.last_frontier_height, estimate.last_update_at)
    {
        if frontier < last_height {
            // Height went backwards (reorg/rescan). Reset estimator state.
            estimate.start_height = Some(frontier);
            estimate.start_instant = Some(now);
            estimate.last_frontier_height = Some(frontier);
            estimate.last_update_at = Some(now);
            estimate.ewma_blocks_per_sec = None;
            estimate.last_eta_seconds = None;
            progress.eta_seconds = None;
            return progress;
        }

        let delta_h = frontier.saturating_sub(last_height);
        let delta_t = now.duration_since(last_update_at).as_secs_f64();
        if delta_t >= 0.05 {
            let inst_rate = delta_h as f64 / delta_t;

            // Time-based EWMA so results don't depend on callback frequency.
            let tau = 20.0;
            let alpha = 1.0 - (-delta_t / tau).exp();
            estimate.ewma_blocks_per_sec = Some(match estimate.ewma_blocks_per_sec {
                Some(prev) => prev + alpha * (inst_rate - prev),
                None => inst_rate,
            });
        }
    }

    estimate.last_frontier_height = Some(frontier);
    estimate.last_update_at = Some(now);

    let avg_rate_blocks_per_sec = match (estimate.start_height, estimate.start_instant) {
        (Some(start_height), Some(start_instant)) => {
            let done = frontier.saturating_sub(start_height) as f64;
            let elapsed = now.duration_since(start_instant).as_secs_f64();
            if done > 0.0 && elapsed > 0.0 {
                Some(done / elapsed)
            } else {
                None
            }
        }
        _ => None,
    };

    let eta_rate_blocks_per_sec = match (estimate.ewma_blocks_per_sec, avg_rate_blocks_per_sec) {
        (Some(ewma), Some(avg))
            if ewma.is_finite() && avg.is_finite() && ewma > 0.0 && avg > 0.0 =>
        {
            // Blend short-term and long-term rates to avoid ETAs that overreact to short stalls.
            Some((0.7 * ewma) + (0.3 * avg))
        }
        (Some(ewma), _) if ewma.is_finite() && ewma > 0.0 => Some(ewma),
        (_, Some(avg)) if avg.is_finite() && avg > 0.0 => Some(avg),
        _ => None,
    };

    let computed_eta_seconds = eta_rate_blocks_per_sec.and_then(|rate| {
        // Early estimates are extremely noisy; wait for a minimum amount of work/time.
        const MIN_DONE_BLOCKS: u64 = 10_000;
        const MIN_ELAPSED_SECS: f64 = 5.0;

        let (Some(start_height), Some(start_instant)) =
            (estimate.start_height, estimate.start_instant)
        else {
            return None;
        };
        let done = frontier.saturating_sub(start_height) as u64;
        let elapsed = now.duration_since(start_instant).as_secs_f64();
        if done < MIN_DONE_BLOCKS || elapsed < MIN_ELAPSED_SECS {
            return None;
        }

        let remaining = (target - frontier) as f64;
        let eta = (remaining / rate).round();
        if eta.is_finite() && eta >= 0.0 {
            Some(eta as u64)
        } else {
            None
        }
    });

    // Keep ETA stable: allow decreases immediately, but clamp sudden large increases
    // (common when a single slow batch skews the instantaneous rate).
    progress.eta_seconds = match (estimate.last_eta_seconds, computed_eta_seconds) {
        (_, None) => {
            estimate.last_eta_seconds = None;
            None
        }
        (None, Some(new_eta)) => {
            estimate.last_eta_seconds = Some(new_eta);
            Some(new_eta)
        }
        (Some(prev), Some(mut new_eta)) => {
            let max_increase = ((prev as f64) * 0.2 * delta_t_for_clamp).ceil() as u64;
            let max_increase = max_increase
                .max((5.0 * delta_t_for_clamp).ceil() as u64)
                .max(1); // +20%/s or +5s/s
            let allowed = prev.saturating_add(max_increase);
            if new_eta > allowed {
                new_eta = allowed;
            }
            estimate.last_eta_seconds = Some(new_eta);
            Some(new_eta)
        }
    };

    progress
}

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}

/// Calculate sync progress percentage and fully scanned height from wallet summary.
///
/// Uses Zashi-style safe ratio handling:
/// - Composes scan + recovery progress for overall percentage
/// - denominator == 0 means 100% complete (no outputs to scan)
/// - Clamps ratio to 0.0-1.0 to handle backend anomalies
/// - Returns fully_scanned_height for monotonic progress display
fn calculate_progress_and_height<C, R>(
    wdb: &zcash_client_sqlite::WalletDb<
        C,
        zcash_protocol::consensus::Network,
        zcash_client_sqlite::util::SystemClock,
        R,
    >,
) -> (u8, u32)
where
    C: std::borrow::BorrowMut<Connection>,
    R: rand::RngCore + rand::CryptoRng,
{
    let summary = wdb
        .get_wallet_summary(ConfirmationsPolicy::default())
        .ok()
        .flatten();

    match summary {
        Some(s) => {
            let scan = s.progress().scan();
            let recovery = s.progress().recovery();

            // Compose scan + recovery like Zashi
            let numerator = *scan.numerator() + recovery.map_or(0, |r| *r.numerator());
            let denominator = *scan.denominator() + recovery.map_or(0, |r| *r.denominator());

            // Zashi: denominator == 0 means 100% complete
            let pct = if denominator == 0 {
                100
            } else {
                // Clamp to valid range (defensive against backend anomalies)
                let ratio = (numerator as f64 / denominator as f64).clamp(0.0, 1.0);
                (ratio * 100.0) as u8
            };

            (pct, u32::from(s.fully_scanned_height()))
        }
        None => {
            tracing::debug!("wallet summary unavailable for progress calculation");
            (0, 0)
        }
    }
}

/// Query birthday heights that need tree size backfill on a blocking thread.
async fn query_birthday_heights_blocking(
    conn: Connection,
) -> anyhow::Result<(Connection, anyhow::Result<Vec<u32>>)> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let result = (|| {
                let mut stmt = conn
                    .prepare(
                        r#"
                SELECT DISTINCT birthday_height
                FROM accounts
                WHERE birthday_sapling_tree_size IS NULL OR birthday_sapling_tree_size = 0
                   OR birthday_orchard_tree_size IS NULL OR birthday_orchard_tree_size = 0
                "#,
                    )
                    .context("failed to query account birthdays for tree size backfill")?;

                stmt.query_map([], |row| row.get::<_, u32>(0))?
                    .collect::<Result<Vec<_>, _>>()
                    .context("failed to read account birthday heights")
            })();
            (conn, result)
        }),
        "query_birthday_heights_blocking",
    )
    .await
}

/// Update account birthday tree sizes on a blocking thread.
async fn update_birthday_tree_sizes_blocking(
    conn: Connection,
    birthday_height: u32,
    sapling_tree_size: u64,
    orchard_tree_size: u64,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<(Connection, anyhow::Result<usize>)> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let result = conn
                .execute(
                    "UPDATE accounts
                 SET birthday_sapling_tree_size = ?1,
                     birthday_orchard_tree_size = ?2
                 WHERE birthday_height = ?3",
                    rusqlite::params![sapling_tree_size, orchard_tree_size, birthday_height],
                )
                .context("failed to update account birthday tree sizes");

            if let Ok(rows) = &result
                && *rows > 0
            {
                tracing::debug!(
                    wallet_id = %wallet_id,
                    birthday_height,
                    sapling_tree_size,
                    orchard_tree_size,
                    updated_accounts = rows,
                    "backfilled account birthday tree sizes"
                );
            }
            (conn, result)
        }),
        "update_birthday_tree_sizes_blocking",
    )
    .await
}

/// Backfill account birthday tree sizes from lightwalletd.
///
/// This function queries birthday heights on a blocking thread, fetches tree states
/// via async network calls, and updates the database on blocking threads.
async fn backfill_birthday_tree_sizes(
    conn: Connection,
    client: &zbag_network::grpc_client::GrpcClient,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<(Connection, anyhow::Result<()>)> {
    // Query birthday heights on blocking thread
    let (mut conn, heights_result) = match query_birthday_heights_blocking(conn).await {
        Ok(result) => result,
        Err(err) => return Err(err),
    };

    let birthday_heights = match heights_result {
        Ok(heights) => heights,
        Err(e) => return Ok((conn, Err(e))),
    };

    for birthday_height in birthday_heights {
        let prior_height =
            zcash_protocol::consensus::BlockHeight::from(birthday_height.saturating_sub(1));

        // Async network call
        let chain_state = match fetch_chain_state(client, prior_height, wallet_id).await {
            Ok(state) => state,
            Err(e) => return Ok((conn, Err(e))),
        };

        let sapling_tree_size = chain_state.final_sapling_tree().tree_size();
        let orchard_tree_size = chain_state.final_orchard_tree().tree_size();

        // Update on blocking thread
        let (returned_conn, update_result) = match update_birthday_tree_sizes_blocking(
            conn,
            birthday_height,
            sapling_tree_size,
            orchard_tree_size,
            wallet_id,
        )
        .await
        {
            Ok(result) => result,
            Err(err) => {
                return Err(err);
            }
        };
        conn = returned_conn;

        if let Err(e) = update_result {
            return Ok((conn, Err(e)));
        }
    }

    Ok((conn, Ok(())))
}

/// Write a compact block to the filesystem block cache.
fn write_block_to_cache(
    blocks_dir: &std::path::Path,
    block: &CompactBlock,
) -> anyhow::Result<BlockMeta> {
    // Validate block height fits in u32 (Zcash blocks are well within this range)
    let height_u32 = u32::try_from(block.height)
        .with_context(|| format!("block height {} exceeds u32::MAX", block.height))?;
    let height = BlockHeight::from_u32(height_u32);
    let hash_bytes: [u8; 32] = match block.hash.as_slice().try_into() {
        Ok(h) => h,
        Err(_) => {
            tracing::warn!(
                block_height = block.height,
                hash_len = block.hash.len(),
                "malformed block hash, using zeros"
            );
            [0u8; 32]
        }
    };
    let block_hash = BlockHash(hash_bytes);

    let block_meta = BlockMeta {
        height,
        block_hash,
        block_time: block.time,
        sapling_outputs_count: block.vtx.iter().map(|tx| tx.outputs.len() as u32).sum(),
        orchard_actions_count: block.vtx.iter().map(|tx| tx.actions.len() as u32).sum(),
    };

    let blocks_dir_buf = blocks_dir.to_path_buf();
    let block_path = block_meta.block_file_path(&blocks_dir_buf);
    let mut file = secure_open_options()
        .open(&block_path)
        .with_context(|| format!("failed to create block file: {}", block_path.display()))?;
    file.write_all(&block.encode_to_vec())
        .with_context(|| format!("failed to write block file: {}", block_path.display()))?;

    Ok(block_meta)
}

/// Create an empty chain state at the given height.
fn empty_chain_state(height: BlockHeight) -> ChainState {
    ChainState::empty(height, BlockHash([0; 32]))
}

/// Fetch the chain state from lightwalletd.
///
/// For the very first scan from genesis (height 0), empty state is correct.
/// For incremental syncs, we need the actual tree state for proper witness computation.
async fn fetch_chain_state(
    client: &zbag_network::grpc_client::GrpcClient,
    height: BlockHeight,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<ChainState> {
    // For height 0 (genesis), empty state is correct
    if u32::from(height) == 0 {
        return Ok(empty_chain_state(height));
    }

    let tree_state = client.get_tree_state(height).await.map_err(|e| {
        anyhow::anyhow!(
            "failed to fetch tree state at height {}: {}",
            u32::from(height),
            e
        )
    })?;

    let state = tree_state.to_chain_state().map_err(|e| {
        anyhow::anyhow!(
            "failed to parse tree state at height {}: {}",
            u32::from(height),
            e
        )
    })?;

    tracing::debug!(
        wallet_id = %wallet_id,
        height = %u32::from(height),
        "fetched tree state from lightwalletd"
    );

    Ok(state)
}

/// Get the effective batch size for a given block height.
/// Uses smaller batches during sandblasting periods when blocks are larger.
fn effective_batch_size(height: BlockHeight) -> u32 {
    let h = u32::from(height);
    if SANDBLASTING_RANGE.contains(&h) {
        BATCH_SIZE_SANDBLASTING
    } else {
        BATCH_SIZE
    }
}

/// Delete cached block files that were scanned in the current batch.
fn delete_cached_block_files_for_batch(blocks_dir: &std::path::Path, block_metas: &[BlockMeta]) {
    if block_metas.is_empty() {
        return;
    }

    let blocks_dir_buf = blocks_dir.to_path_buf();
    for block_meta in block_metas {
        let path = block_meta.block_file_path(&blocks_dir_buf);
        if let Err(e) = std::fs::remove_file(&path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::trace!(path = ?path, error = ?e, "failed to delete block file");
        }
    }
}

/// Delete cached block files for a range of heights.
/// Called after scanning to prevent file accumulation.
fn delete_cached_block_files(blocks_dir: &std::path::Path, start: BlockHeight, end: BlockHeight) {
    let start_u32 = u32::from(start);
    let end_u32 = u32::from(end);

    // Read directory once and filter matching files
    let Ok(entries) = std::fs::read_dir(blocks_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        // Block files are named: {height}-{hash}-compactblock
        let Some(height_str) = filename.split('-').next() else {
            continue;
        };
        let Ok(height) = height_str.parse::<u32>() else {
            continue;
        };
        if height >= start_u32
            && height < end_u32
            && let Err(e) = std::fs::remove_file(&path)
        {
            tracing::trace!(path = ?path, error = ?e, "failed to delete block file");
        }
    }
}

/// Batch size for memo enhancement queries.
/// This limits memory usage when enhancing wallets with many historical transactions.
const ENHANCEMENT_BATCH_SIZE: u32 = 100;

/// Counts transactions with received notes that have NULL memos.
///
/// Used for progress tracking during the enhancement phase.
#[doc(hidden)]
pub fn count_txids_needing_memo_enhancement(
    wallet_db_path: &Path,
    wallet_dek: &Dek,
) -> anyhow::Result<u32> {
    let conn = open_wallet_db(wallet_db_path, wallet_dek)?;
    let count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT DISTINCT transactions.txid FROM transactions
            JOIN sapling_received_notes ON sapling_received_notes.transaction_id = transactions.id_tx
            WHERE sapling_received_notes.memo IS NULL
            UNION
            SELECT DISTINCT transactions.txid FROM transactions
            JOIN orchard_received_notes ON orchard_received_notes.transaction_id = transactions.id_tx
            WHERE orchard_received_notes.memo IS NULL
        )",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Returns a batch of txids for transactions with received notes that have NULL memos.
///
/// These transactions need enhancement to fetch memo data from full transactions.
/// Uses LIMIT/OFFSET pagination to avoid loading all txids into memory at once.
///
/// Note: This opens a separate connection from `enhance_transaction_memo`. A theoretical
/// race exists if another process modifies the database between query and update, but this
/// is benign in practice: the wallet is single-process, and the UPDATE is idempotent.
#[doc(hidden)]
pub fn get_txids_needing_memo_enhancement_batch(
    wallet_db_path: &Path,
    wallet_dek: &Dek,
    offset: u32,
    limit: u32,
) -> anyhow::Result<Vec<[u8; 32]>> {
    let conn = open_wallet_db(wallet_db_path, wallet_dek)?;
    let mut stmt = conn.prepare(
        "SELECT txid FROM (
            SELECT DISTINCT transactions.txid FROM transactions
            JOIN sapling_received_notes ON sapling_received_notes.transaction_id = transactions.id_tx
            WHERE sapling_received_notes.memo IS NULL
            UNION
            SELECT DISTINCT transactions.txid FROM transactions
            JOIN orchard_received_notes ON orchard_received_notes.transaction_id = transactions.id_tx
            WHERE orchard_received_notes.memo IS NULL
        ) LIMIT ?1 OFFSET ?2",
    )?;

    let mut txids = Vec::new();
    let mut skipped_rows = 0u32;
    let mut malformed_txids = 0u32;

    let rows = stmt.query_map([limit, offset], |row| row.get::<_, Vec<u8>>(0))?;

    for result in rows {
        match result {
            Ok(bytes) => {
                if let Ok(txid_array) = bytes.try_into() {
                    txids.push(txid_array);
                } else {
                    malformed_txids += 1;
                    tracing::warn!("malformed txid in database: expected 32 bytes");
                }
            }
            Err(err) => {
                skipped_rows += 1;
                tracing::warn!(
                    error = ?err,
                    "failed to read txid row from database"
                );
            }
        }
    }

    if skipped_rows > 0 || malformed_txids > 0 {
        tracing::warn!(
            skipped_rows,
            malformed_txids,
            "memo enhancement query skipped some rows"
        );
    }

    Ok(txids)
}

/// Fetch, decrypt, and store memos for a single transaction.
///
/// This is called during the Enhancing phase to populate memo data
/// for received notes that were scanned from compact blocks.
async fn enhance_transaction_memo(
    client: &zbag_network::grpc_client::GrpcClient,
    wallet_db_path: &Path,
    wallet_dek: &Dek,
    params: &zcash_protocol::consensus::Network,
    txid_bytes: [u8; 32],
) -> anyhow::Result<()> {
    let txid = TxId::from_bytes(txid_bytes);

    // 1. Fetch the raw transaction
    let raw_tx = client.get_transaction(&txid).await?;

    let wallet_db_path = wallet_db_path.to_path_buf();
    let wallet_dek = wallet_dek.clone_key_material();
    let params = *params;

    crate::tokio_runtime::spawn_blocking(move || {
        enhance_transaction_memo_blocking(&wallet_db_path, &wallet_dek, params, txid_bytes, raw_tx)
    })
    .await
    .context("spawn_blocking panicked")?
}

fn enhance_transaction_memo_blocking(
    wallet_db_path: &Path,
    wallet_dek: &Dek,
    params: zcash_protocol::consensus::Network,
    txid_bytes: [u8; 32],
    raw_tx: zcash_client_backend::proto::service::RawTransaction,
) -> anyhow::Result<()> {
    // 2. Determine branch ID from mined height
    // Handle sentinel values from lightwalletd protobuf:
    // - 0 means mempool (unmined) - this is a lightwalletd convention, not genesis block.
    //   Zcash mainnet/testnet genesis is at height 1, so height 0 unambiguously means mempool.
    // - u64::MAX means transaction is on a fork
    let height_u32 = match raw_tx.height {
        0 => anyhow::bail!("cannot enhance mempool transaction (not yet mined)"),
        u64::MAX => anyhow::bail!("transaction is on a fork"),
        h => u32::try_from(h).context("block height exceeds u32::MAX")?,
    };
    let mined_height = BlockHeight::from_u32(height_u32);
    let branch_id = BranchId::for_height(&params, mined_height);

    // 3. Parse the transaction
    let tx =
        Transaction::read(&raw_tx.data[..], branch_id).context("failed to parse transaction")?;

    // 4. Open a separate connection for enhancement to avoid borrow conflicts
    let mut conn = open_wallet_db(wallet_db_path, wallet_dek)?;

    // 5. Get UFVKs for decryption using the WalletRead trait
    let ufvks = {
        let wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );
        wdb.get_unified_full_viewing_keys()?
    };

    // 6. Decrypt the transaction
    let decrypted = decrypt_transaction(&params, Some(mined_height), None, &tx, &ufvks);

    // 7. Update memos in a transaction to ensure atomicity.
    // Without this, a crash between updates could leave some outputs with memos
    // while others remain NULL, and the transaction would be skipped on re-sync.
    let db_tx = conn.transaction()?;

    // 7a. Update Sapling memos (only if NULL for idempotency)
    for output in decrypted.sapling_outputs() {
        let memo_bytes: &[u8] = output.memo().as_slice();
        // output.index() is a usize from zcash_client_backend; Zcash transactions have at most
        // 2^16 outputs per pool, so this cast is safe.
        db_tx.execute(
            "UPDATE sapling_received_notes SET memo = ?1
             WHERE transaction_id = (SELECT id_tx FROM transactions WHERE txid = ?2)
             AND output_index = ?3
             AND memo IS NULL",
            rusqlite::params![memo_bytes, &txid_bytes[..], output.index() as i64],
        )?;
    }

    // 7b. Update Orchard memos (only if NULL for idempotency)
    for output in decrypted.orchard_outputs() {
        let memo_bytes: &[u8] = output.memo().as_slice();
        // output.index() is a usize from zcash_client_backend; Zcash transactions have at most
        // 2^16 actions per bundle, so this cast is safe.
        db_tx.execute(
            "UPDATE orchard_received_notes SET memo = ?1
             WHERE transaction_id = (SELECT id_tx FROM transactions WHERE txid = ?2)
             AND action_index = ?3
             AND memo IS NULL",
            rusqlite::params![memo_bytes, &txid_bytes[..], output.index() as i64],
        )?;
    }

    db_tx.commit()?;

    Ok(())
}

/// Download blocks with retry and exponential backoff.
async fn download_blocks_with_retry(
    client: &zbag_network::grpc_client::GrpcClient,
    start: BlockHeight,
    end_exclusive: BlockHeight,
    max_retries: u32,
) -> anyhow::Result<Vec<CompactBlock>> {
    // Guard: empty range returns empty vec
    if end_exclusive <= start {
        return Ok(vec![]);
    }
    // Convert exclusive end to inclusive for lightwalletd API
    let end_inclusive = end_exclusive.saturating_sub(1);

    let mut attempt = 0;
    loop {
        match client.get_block_range(start, end_inclusive).await {
            Ok(mut stream) => {
                let mut blocks = Vec::new();
                loop {
                    match stream.message().await {
                        Ok(Some(block)) => blocks.push(block),
                        Ok(None) => break,
                        Err(err) => {
                            // Stream error - will retry from scratch
                            if attempt < max_retries {
                                tracing::warn!(
                                    attempt = attempt + 1,
                                    max_retries = max_retries,
                                    error = ?err,
                                    "stream error, will retry"
                                );
                                break;
                            }
                            return Err(anyhow::Error::from(err));
                        }
                    }
                }
                if !blocks.is_empty() || attempt >= max_retries {
                    return Ok(blocks);
                }
            }
            Err(err) if attempt < max_retries => {
                attempt += 1;
                let delay_ms = 1000 * 2u64.pow(attempt.min(6)); // Max ~64 seconds
                let delay = std::time::Duration::from_millis(delay_ms.min(60_000)); // Cap at 60 seconds
                tracing::warn!(
                    attempt = attempt,
                    max_retries = max_retries,
                    delay_secs = delay.as_secs(),
                    error = ?err,
                    "download failed, retrying"
                );
                tokio::time::sleep(delay).await;
            }
            Err(err) => {
                return Err(err);
            }
        }
        attempt += 1;
    }
}

// =============================================================================
// Async wrappers for blocking filesystem and SQLite operations
// =============================================================================
// These functions move blocking work onto the Tokio blocking thread pool to
// avoid starving async worker threads during sync.

async fn join_spawn_blocking<T>(
    join: tokio::task::JoinHandle<T>,
    operation: &'static str,
) -> anyhow::Result<T> {
    join.await
        .with_context(|| format!("{operation}: spawn_blocking task failed"))
}

/// Initialize block cache directory on a blocking thread.
async fn create_cache_dir_async(cache_dir: PathBuf) -> anyhow::Result<()> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || create_dir_all_secure(&cache_dir)),
        "create_cache_dir_async",
    )
    .await??;
    Ok(())
}

/// Count memo enhancement candidates on a blocking thread.
async fn count_txids_needing_memo_enhancement_blocking(
    wallet_db_path: PathBuf,
    wallet_dek: Dek,
) -> anyhow::Result<u32> {
    crate::tokio_runtime::spawn_blocking(move || {
        count_txids_needing_memo_enhancement(&wallet_db_path, &wallet_dek)
    })
    .await
    .context("spawn_blocking panicked")?
}

/// Fetch a memo enhancement txid batch on a blocking thread.
async fn get_txids_needing_memo_enhancement_batch_blocking(
    wallet_db_path: PathBuf,
    wallet_dek: Dek,
    offset: u32,
    limit: u32,
) -> anyhow::Result<Vec<[u8; 32]>> {
    crate::tokio_runtime::spawn_blocking(move || {
        get_txids_needing_memo_enhancement_batch(&wallet_db_path, &wallet_dek, offset, limit)
    })
    .await
    .context("spawn_blocking panicked")?
}

/// Initialize FsBlockDb on a blocking thread.
async fn init_fsblock_db_async(
    cache_dir: PathBuf,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<FsBlockDb> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let mut db = FsBlockDb::for_path(&cache_dir)?;
            // Initialize the block metadata database schema
            if let Err(err) = init_blockmeta_db(&mut db) {
                tracing::warn!(
                    wallet_id = %wallet_id,
                    error = ?err,
                    "failed to init blockmeta db schema"
                );
            }
            Ok::<FsBlockDb, zcash_client_sqlite::FsBlockDbError>(db)
        }),
        "init_fsblock_db_async",
    )
    .await?
    .map_err(|err| anyhow::anyhow!("failed to initialize fs block db: {err:?}"))
}

/// Remove cache directory on a blocking thread.
async fn remove_cache_dir_async(cache_dir: PathBuf) {
    let path_for_log = cache_dir.clone();
    if let Err(err) = crate::tokio_runtime::spawn_blocking(move || {
        if let Err(e) = std::fs::remove_dir_all(&cache_dir) {
            tracing::debug!(path = ?cache_dir, error = ?e, "failed to cleanup block cache directory");
        }
    })
    .await
    {
        tracing::warn!(
            path = ?path_for_log,
            error = ?err,
            "cache cleanup worker panicked"
        );
    }
}

/// Write blocks to cache and register metadata on a blocking thread.
/// Returns the block metadata for successfully cached blocks.
async fn write_blocks_to_cache_async(
    blocks_dir: PathBuf,
    blocks: Vec<CompactBlock>,
    fsblock_db: FsBlockDb,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<(FsBlockDb, Vec<BlockMeta>)> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let mut block_metas = Vec::with_capacity(blocks.len());
            for block in &blocks {
                match write_block_to_cache(&blocks_dir, block) {
                    Ok(meta) => block_metas.push(meta),
                    Err(err) => {
                        tracing::error!(
                            wallet_id = %wallet_id,
                            block_height = block.height,
                            error = ?err,
                            "failed to cache block - block may not be scanned"
                        );
                    }
                }
            }

            // Register block metadata
            if !block_metas.is_empty()
                && let Err(err) = fsblock_db.write_block_metadata(&block_metas)
            {
                tracing::error!(
                    wallet_id = %wallet_id,
                    error = ?err,
                    "failed to write block metadata"
                );
            }

            (fsblock_db, block_metas)
        }),
        "write_blocks_to_cache_async",
    )
    .await
}

/// Result of a batch scan operation.
struct ScanBatchResult {
    conn: Connection,
    fsblock_db: FsBlockDb,
    scan_outcome: ScanOutcome,
    scan_activity: ScanBatchActivity,
    progress_percent: u8,
    fully_scanned: u32,
}

/// Outcome of scanning a batch.
enum ScanOutcome {
    Success,
    ReorgDetected { rewind_height: BlockHeight },
    Error(String),
}

/// Scan a batch of blocks on a blocking thread.
///
/// This moves all SQLite operations (scan_cached_blocks, truncate, etc.) off the async runtime.
#[allow(clippy::too_many_arguments)]
async fn scan_batch_blocking(
    mut conn: Connection,
    fsblock_db: FsBlockDb,
    params: zcash_protocol::consensus::Network,
    batch_range_start: BlockHeight,
    chain_state: ChainState,
    batch_block_metas: Vec<BlockMeta>,
    blocks_dir: PathBuf,
    batch_range_end: BlockHeight,
    state: Arc<Mutex<State>>,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<ScanBatchResult> {
    let in_flight = BlockingScanInFlightGuard::acquire(state, wallet_id);

    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            #[cfg(test)]
            let _finish_guard = blocking_scan_test_hook::enter_blocking_scan();
            let _in_flight = in_flight;

            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let limit = batch_block_metas.len();
            let mut scan_activity = ScanBatchActivity {
                scanned_blocks: u32::try_from(limit).unwrap_or(u32::MAX),
                ..Default::default()
            };

            let scan_outcome = if limit > 0 {
                #[cfg(test)]
                let _scan_call_guard = blocking_scan_test_hook::enter_scan_call();

                match scan_cached_blocks(
                    &params,
                    &fsblock_db,
                    &mut wdb,
                    batch_range_start,
                    &chain_state,
                    limit,
                ) {
                    Ok(scan_result) => {
                        scan_activity = ScanBatchActivity {
                            scanned_blocks: u32::try_from(limit).unwrap_or(u32::MAX),
                            spent_sapling: u32::try_from(scan_result.spent_sapling_note_count())
                                .unwrap_or(u32::MAX),
                            spent_orchard: u32::try_from(scan_result.spent_orchard_note_count())
                                .unwrap_or(u32::MAX),
                            received_sapling: u32::try_from(
                                scan_result.received_sapling_note_count(),
                            )
                            .unwrap_or(u32::MAX),
                            received_orchard: u32::try_from(
                                scan_result.received_orchard_note_count(),
                            )
                            .unwrap_or(u32::MAX),
                        };
                        tracing::debug!(
                            wallet_id = %wallet_id,
                            scanned_range = ?scan_result.scanned_range(),
                            spent_sapling = scan_result.spent_sapling_note_count(),
                            spent_orchard = scan_result.spent_orchard_note_count(),
                            received_sapling = scan_result.received_sapling_note_count(),
                            received_orchard = scan_result.received_orchard_note_count(),
                            "scanned blocks"
                        );
                        ScanOutcome::Success
                    }
                    Err(ChainError::Scan(scan_err)) if scan_err.is_continuity_error() => {
                        // Chain reorg detected. Rewind the wallet to recover.
                        let rewind_height = scan_err.at_height().saturating_sub(10);
                        tracing::warn!(
                            wallet_id = %wallet_id,
                            error_height = %u32::from(scan_err.at_height()),
                            rewind_height = %u32::from(rewind_height),
                            "chain reorg detected, rewinding wallet"
                        );

                        // Truncate the wallet database to the rewind height.
                        if let Err(truncate_err) = wdb.truncate_to_height(rewind_height) {
                            tracing::error!(
                                wallet_id = %wallet_id,
                                error = ?truncate_err,
                                "failed to truncate wallet for reorg recovery"
                            );
                            ScanOutcome::Error(format!("truncate failed: {truncate_err}"))
                        } else {
                            // Clear the block cache from rewind height onwards.
                            if let Err(cache_err) = fsblock_db.truncate_to_height(rewind_height) {
                                tracing::debug!(
                                    wallet_id = %wallet_id,
                                    error = ?cache_err,
                                    "failed to truncate block cache after reorg"
                                );
                            }

                            // Delete cached block files that are now invalid.
                            delete_cached_block_files(
                                &blocks_dir,
                                rewind_height + 1,
                                batch_range_end,
                            );

                            ScanOutcome::ReorgDetected { rewind_height }
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            wallet_id = %wallet_id,
                            range_start = %u32::from(batch_range_start),
                            limit = limit,
                            error = ?err,
                            "failed to scan blocks, aborting range"
                        );
                        ScanOutcome::Error(format!("{err}"))
                    }
                }
            } else {
                ScanOutcome::Success
            };

            // Clean up scanned blocks from cache (metadata) on success
            if matches!(scan_outcome, ScanOutcome::Success) && limit > 0 {
                let prior_height = batch_range_start.saturating_sub(1);
                if let Err(err) = fsblock_db.truncate_to_height(prior_height) {
                    tracing::debug!(
                        wallet_id = %wallet_id,
                        error = ?err,
                        "failed to truncate block cache metadata"
                    );
                }

                // Delete the actual block files to prevent accumulation
                delete_cached_block_files_for_batch(&blocks_dir, &batch_block_metas);
            }

            // Calculate progress while we still have wdb
            let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);

            ScanBatchResult {
                conn,
                fsblock_db,
                scan_outcome,
                scan_activity,
                progress_percent,
                fully_scanned,
            }
        }),
        "scan_batch_blocking",
    )
    .await
}

/// Run update_chain_tip on a blocking thread.
async fn update_chain_tip_blocking(
    mut conn: Connection,
    params: zcash_protocol::consensus::Network,
    chain_tip: BlockHeight,
) -> anyhow::Result<(Connection, Result<(), String>)> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let result = wdb.update_chain_tip(chain_tip).map_err(|e| format!("{e}"));

            (conn, result)
        }),
        "update_chain_tip_blocking",
    )
    .await
}

/// Get suggested scan ranges on a blocking thread.
async fn suggest_scan_ranges_blocking(
    mut conn: Connection,
    params: zcash_protocol::consensus::Network,
) -> anyhow::Result<(
    Connection,
    Result<Vec<zcash_client_backend::data_api::scanning::ScanRange>, String>,
)> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let result = wdb.suggest_scan_ranges().map_err(|e| format!("{e}"));

            (conn, result)
        }),
        "suggest_scan_ranges_blocking",
    )
    .await
}

/// Get chain height and progress on a blocking thread.
async fn get_progress_blocking(
    mut conn: Connection,
    params: zcash_protocol::consensus::Network,
) -> anyhow::Result<(Connection, u8, u32, Option<BlockHeight>)> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);
            let chain_height = wdb.chain_height().ok().flatten();

            (conn, progress_percent, fully_scanned, chain_height)
        }),
        "get_progress_blocking",
    )
    .await
}

/// Fetch balances for all accounts on a blocking thread.
///
/// Returns the connection and a vector of `(account_id, balance)` pairs for
/// accounts whose balance query succeeded.
async fn fetch_balances_blocking(
    conn: Connection,
    network: Network,
    account_ids: Vec<u32>,
) -> anyhow::Result<(Connection, Vec<(u32, Balance)>)> {
    join_spawn_blocking(
        crate::tokio_runtime::spawn_blocking(move || {
            let mut conn = conn;
            let mut balances = Vec::with_capacity(account_ids.len());
            for account_id in account_ids {
                let Ok(balance) = crate::balance::get_balance(&mut conn, network, account_id)
                else {
                    continue;
                };
                balances.push((account_id, balance));
            }
            (conn, balances)
        }),
        "fetch_balances_blocking",
    )
    .await
}

#[cfg(test)]
mod blocking_scan_test_hook {
    use std::sync::mpsc;
    use std::sync::{Mutex, OnceLock};

    #[derive(Debug)]
    struct BlockingScanHook {
        started_tx: mpsc::Sender<()>,
        release_rx: mpsc::Receiver<()>,
        finished_tx: mpsc::Sender<()>,
    }

    #[derive(Debug)]
    pub(super) struct BlockingScanGate {
        started_rx: mpsc::Receiver<()>,
        release_tx: mpsc::Sender<()>,
        finished_rx: mpsc::Receiver<()>,
    }

    impl BlockingScanGate {
        pub(super) fn wait_until_started(&self) {
            self.started_rx.recv().expect("scan start signal dropped");
        }

        pub(super) fn release(&self) {
            self.release_tx.send(()).expect("release signal dropped");
        }

        pub(super) fn wait_until_finished(&self) {
            self.finished_rx.recv().expect("scan finish signal dropped");
        }
    }

    #[derive(Debug)]
    pub(super) struct FinishSignalGuard(Option<mpsc::Sender<()>>);

    impl Drop for FinishSignalGuard {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    static BLOCKING_SCAN_HOOK: OnceLock<Mutex<Option<BlockingScanHook>>> = OnceLock::new();
    static SCAN_CALL_HOOK: OnceLock<Mutex<Option<BlockingScanHook>>> = OnceLock::new();

    fn install_in(slot: &OnceLock<Mutex<Option<BlockingScanHook>>>) -> BlockingScanGate {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();

        let hook = BlockingScanHook {
            started_tx,
            release_rx,
            finished_tx,
        };

        let mut slot = slot
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("mutex poisoned");
        assert!(slot.is_none(), "blocking scan test hook already installed");
        *slot = Some(hook);

        BlockingScanGate {
            started_rx,
            release_tx,
            finished_rx,
        }
    }

    fn enter_in(slot: &OnceLock<Mutex<Option<BlockingScanHook>>>) -> FinishSignalGuard {
        let hook = slot
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("mutex poisoned")
            .take();
        let Some(hook) = hook else {
            return FinishSignalGuard(None);
        };

        let _ = hook.started_tx.send(());
        let _ = hook.release_rx.recv();
        FinishSignalGuard(Some(hook.finished_tx))
    }

    pub(super) fn install() -> BlockingScanGate {
        install_in(&BLOCKING_SCAN_HOOK)
    }

    pub(super) fn install_scan_call() -> BlockingScanGate {
        install_in(&SCAN_CALL_HOOK)
    }

    pub(super) fn enter_blocking_scan() -> FinishSignalGuard {
        enter_in(&BLOCKING_SCAN_HOOK)
    }

    pub(super) fn enter_scan_call() -> FinishSignalGuard {
        enter_in(&SCAN_CALL_HOOK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static BLOCKING_SCAN_TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// Creates a minimal wallet database schema sufficient for memo enhancement queries.
    fn create_minimal_wallet_schema(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE transactions (
                id_tx INTEGER PRIMARY KEY,
                txid BLOB NOT NULL
            );
            CREATE TABLE sapling_received_notes (
                id INTEGER PRIMARY KEY,
                transaction_id INTEGER NOT NULL,
                memo BLOB
            );
            CREATE TABLE orchard_received_notes (
                id INTEGER PRIMARY KEY,
                transaction_id INTEGER NOT NULL,
                memo BLOB
            );
            ",
        )
        .expect("create minimal wallet schema");
    }

    fn progress_for_test(phase: SyncPhase) -> SyncProgress {
        SyncProgress {
            phase,
            scan_frontier_height: 0,
            wallet_tip_height: 0,
            progress_percent: 0,
            eta_seconds: None,
            retry_in_seconds: None,
            error_message: None,
        }
    }

    #[test]
    fn balance_emit_throttle_emits_on_first_batch_and_dual_thresholds() {
        let mut throttle = BalanceEmitThrottle::default();
        let start = Instant::now();

        assert!(throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch {
                scanned_blocks: 100
            },
            start
        ));
        throttle.record_emit(start);

        assert!(!throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch {
                scanned_blocks: 100
            },
            start + std::time::Duration::from_millis(500)
        ));
        assert!(throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch {
                scanned_blocks: 150
            },
            start + std::time::Duration::from_millis(800)
        ));
        throttle.record_emit(start + std::time::Duration::from_millis(800));

        assert!(!throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch { scanned_blocks: 10 },
            start + std::time::Duration::from_millis(2000)
        ));
        assert!(throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch { scanned_blocks: 10 },
            start + std::time::Duration::from_millis(2400)
        ));
    }

    #[test]
    fn balance_emit_throttle_forced_milestone_bypasses_thresholds() {
        let mut throttle = BalanceEmitThrottle::default();
        let start = Instant::now();

        assert!(throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch { scanned_blocks: 1 },
            start
        ));
        throttle.record_emit(start);

        assert!(!throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch { scanned_blocks: 1 },
            start + std::time::Duration::from_millis(100)
        ));
        assert!(throttle.should_emit_and_update(
            BalanceEmitTrigger::ForcedMilestone,
            start + std::time::Duration::from_millis(100)
        ));
        throttle.record_emit(start + std::time::Duration::from_millis(100));

        assert!(!throttle.should_emit_and_update(
            BalanceEmitTrigger::SuccessfulScanBatch {
                scanned_blocks: 100
            },
            start + std::time::Duration::from_millis(200)
        ));
    }

    #[test]
    fn balance_emit_throttle_long_scans_emit_periodically_before_final_milestone() {
        let mut throttle = BalanceEmitThrottle::default();
        let start = Instant::now();
        let mut periodic_emit_batches = Vec::new();

        for batch_idx in 0..20u32 {
            let now = start + std::time::Duration::from_millis(u64::from(batch_idx) * 100);
            if throttle.should_emit_and_update(
                BalanceEmitTrigger::SuccessfulScanBatch {
                    scanned_blocks: 100,
                },
                now,
            ) {
                periodic_emit_batches.push(batch_idx);
                throttle.record_emit(now);
            }
        }

        assert_eq!(periodic_emit_batches, vec![0, 3, 6, 9, 12, 15, 18]);
        assert!(
            periodic_emit_batches.len() > 1,
            "long scans should emit periodically before final completion"
        );
        assert!(throttle.should_emit_and_update(
            BalanceEmitTrigger::ForcedMilestone,
            start + std::time::Duration::from_secs(10)
        ));
    }

    #[test]
    fn adaptive_batch_controller_grows_on_fast_quiet_scans() {
        let mut controller = AdaptiveBatchController::new(BlockHeight::from_u32(3_100_000));
        let initial = controller.next_download_size(BlockHeight::from_u32(3_100_000));
        assert_eq!(initial, BATCH_SIZE);

        let adjustment = controller.record_scan_result(
            BlockHeight::from_u32(3_100_000),
            Duration::from_millis(500),
            ScanBatchActivity {
                scanned_blocks: BATCH_SIZE,
                ..Default::default()
            },
        );
        let adjustment = adjustment.expect("expected growth for fast empty batch");
        assert_eq!(adjustment.decision, AdaptiveBatchDecision::Grew);
        assert!(adjustment.current > adjustment.previous);
        assert!(
            controller.next_download_size(BlockHeight::from_u32(3_100_010)) > initial,
            "controller should increase batch size on fast quiet scans"
        );
    }

    #[test]
    fn adaptive_batch_controller_shrinks_on_slow_scans() {
        let mut controller = AdaptiveBatchController::new(BlockHeight::from_u32(3_100_000));
        let grow = controller
            .record_scan_result(
                BlockHeight::from_u32(3_100_000),
                Duration::from_millis(500),
                ScanBatchActivity {
                    scanned_blocks: BATCH_SIZE,
                    ..Default::default()
                },
            )
            .expect("expected growth before shrink test");
        assert_eq!(grow.decision, AdaptiveBatchDecision::Grew);

        let shrink = controller
            .record_scan_result(
                BlockHeight::from_u32(3_100_250),
                Duration::from_secs(3),
                ScanBatchActivity {
                    scanned_blocks: grow.current,
                    ..Default::default()
                },
            )
            .expect("expected shrink for slow scan");
        assert_eq!(shrink.decision, AdaptiveBatchDecision::Shrunk);
        assert!(shrink.current < shrink.previous);
    }

    #[test]
    fn adaptive_batch_controller_stays_fixed_in_sandblasting_range() {
        let mut controller = AdaptiveBatchController::new(BlockHeight::from_u32(1_900_000));
        assert_eq!(
            controller.next_download_size(BlockHeight::from_u32(1_900_000)),
            BATCH_SIZE_SANDBLASTING
        );

        let adjustment = controller.record_scan_result(
            BlockHeight::from_u32(1_900_000),
            Duration::from_millis(300),
            ScanBatchActivity {
                scanned_blocks: BATCH_SIZE_SANDBLASTING,
                ..Default::default()
            },
        );
        assert!(
            adjustment.is_none(),
            "sandblasting period should keep fixed conservative batch size"
        );
    }

    fn open_test_app_db() -> (tempfile::TempDir, AppDb) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let app_db =
            AppDb::open(temp_dir.path().join("app.db")).expect("open temporary app metadata db");
        (temp_dir, app_db)
    }

    fn wait_until_blocking_scans_clear(service: &SyncService, wallet_id: Uuid) {
        let timeout = std::time::Duration::from_secs(1);
        let poll = std::time::Duration::from_millis(5);
        let start = std::time::Instant::now();

        loop {
            let is_blocking = service
                .state
                .lock()
                .expect("mutex poisoned")
                .blocking_scans
                .contains_key(&wallet_id);
            if !is_blocking {
                return;
            }

            assert!(
                start.elapsed() < timeout,
                "blocking scan in-flight marker did not clear in time"
            );
            std::thread::sleep(poll);
        }
    }

    fn spawn_blocked_scan_job(
        service: &SyncService,
        wallet_id: Uuid,
        on_progress_after_scan: Option<SyncEventHandler>,
    ) -> blocking_scan_test_hook::BlockingScanGate {
        let gate = blocking_scan_test_hook::install();
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let blocks_dir = temp_dir.path().join("blocks");
        std::fs::create_dir_all(&blocks_dir).expect("create blocks dir");
        let conn = Connection::open_in_memory().expect("open in-memory wallet db");
        let fsblock_db = FsBlockDb::for_path(temp_dir.path()).expect("init fs block db");

        let state_for_task = Arc::clone(&service.state);
        let handle = crate::tokio_runtime::spawn(async move {
            let _temp_dir_guard = temp_dir;
            let scan_result = scan_batch_blocking(
                conn,
                fsblock_db,
                zcash_consensus_network(Network::Testnet),
                BlockHeight::from_u32(1),
                empty_chain_state(BlockHeight::from_u32(0)),
                Vec::new(),
                blocks_dir,
                BlockHeight::from_u32(2),
                Arc::clone(&state_for_task),
                wallet_id,
            )
            .await
            .expect("scan batch should not fail");

            if let Some(handler) = on_progress_after_scan {
                let progress = SyncProgress {
                    phase: SyncPhase::Scanning,
                    scan_frontier_height: scan_result.fully_scanned,
                    wallet_tip_height: 0,
                    progress_percent: scan_result.progress_percent,
                    eta_seconds: None,
                    retry_in_seconds: None,
                    error_message: None,
                };
                let mut state = state_for_task.lock().expect("mutex poisoned");
                state.progress.insert(wallet_id, progress.clone());
                drop(state);

                handler(SyncProgressEvent {
                    schema_version: SCHEMA_VERSION,
                    event: "sync.progress".to_string(),
                    progress,
                });
            }
        });

        let (cancel_tx, _) = watch::channel(false);
        let mut state = service.state.lock().expect("mutex poisoned");
        state.jobs.insert(
            wallet_id,
            SyncJob {
                cancel: cancel_tx,
                handle,
            },
        );
        state
            .progress
            .insert(wallet_id, progress_for_test(SyncPhase::Scanning));
        state.started_at.insert(wallet_id, Instant::now());
        drop(state);

        gate
    }

    fn spawn_scan_call_blocked_job(
        service: &SyncService,
        wallet_id: Uuid,
        on_progress_after_scan: Option<SyncEventHandler>,
    ) -> blocking_scan_test_hook::BlockingScanGate {
        let gate = blocking_scan_test_hook::install_scan_call();
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let blocks_dir = temp_dir.path().join("blocks");
        std::fs::create_dir_all(&blocks_dir).expect("create blocks dir");
        let conn = Connection::open_in_memory().expect("open in-memory wallet db");
        let fsblock_db = FsBlockDb::for_path(temp_dir.path()).expect("init fs block db");

        let state_for_task = Arc::clone(&service.state);
        let handle = crate::tokio_runtime::spawn(async move {
            let _temp_dir_guard = temp_dir;
            let scan_result = scan_batch_blocking(
                conn,
                fsblock_db,
                zcash_consensus_network(Network::Testnet),
                BlockHeight::from_u32(1),
                empty_chain_state(BlockHeight::from_u32(0)),
                vec![BlockMeta {
                    height: BlockHeight::from_u32(1),
                    block_hash: BlockHash([0; 32]),
                    block_time: 0,
                    sapling_outputs_count: 0,
                    orchard_actions_count: 0,
                }],
                blocks_dir,
                BlockHeight::from_u32(2),
                Arc::clone(&state_for_task),
                wallet_id,
            )
            .await
            .expect("scan batch should not fail");

            if let Some(handler) = on_progress_after_scan {
                let progress = SyncProgress {
                    phase: SyncPhase::Scanning,
                    scan_frontier_height: scan_result.fully_scanned,
                    wallet_tip_height: 0,
                    progress_percent: scan_result.progress_percent,
                    eta_seconds: None,
                    retry_in_seconds: None,
                    error_message: None,
                };
                let mut state = state_for_task.lock().expect("mutex poisoned");
                state.progress.insert(wallet_id, progress.clone());
                drop(state);

                handler(SyncProgressEvent {
                    schema_version: SCHEMA_VERSION,
                    event: "sync.progress".to_string(),
                    progress,
                });
            }
        });

        let (cancel_tx, _) = watch::channel(false);
        let mut state = service.state.lock().expect("mutex poisoned");
        state.jobs.insert(
            wallet_id,
            SyncJob {
                cancel: cancel_tx,
                handle,
            },
        );
        state
            .progress
            .insert(wallet_id, progress_for_test(SyncPhase::Scanning));
        state.started_at.insert(wallet_id, Instant::now());
        drop(state);

        gate
    }

    #[test]
    fn stop_sync_while_blocking_scan_is_in_flight_resets_idle_and_stops_progress_events() {
        let _serial = BLOCKING_SCAN_TEST_MUTEX.lock().expect("mutex poisoned");
        let service = SyncService::new();
        let wallet_id = Uuid::new_v4();
        let emitted_phases = Arc::new(Mutex::new(Vec::new()));
        let emitted_phases_for_handler = Arc::clone(&emitted_phases);
        let on_progress: SyncEventHandler = Arc::new(move |event| {
            emitted_phases_for_handler
                .lock()
                .expect("mutex poisoned")
                .push(event.progress.phase);
        });

        let gate = spawn_blocked_scan_job(&service, wallet_id, Some(Arc::clone(&on_progress)));
        gate.wait_until_started();

        assert!(service.running_wallet_ids().contains(&wallet_id));

        service
            .stop_sync(wallet_id, Some(Arc::clone(&on_progress)))
            .expect("stop sync while blocked");

        assert_eq!(service.get_progress(wallet_id).phase, SyncPhase::Idle);
        assert!(!service.running_wallet_ids().contains(&wallet_id));

        let phases_after_stop = emitted_phases.lock().expect("mutex poisoned").clone();
        assert_eq!(
            phases_after_stop,
            vec![SyncPhase::Idle],
            "stop_sync should emit a single Idle progress update"
        );

        gate.release();
        gate.wait_until_finished();

        let phases_after_release = emitted_phases.lock().expect("mutex poisoned").clone();
        assert_eq!(
            phases_after_release,
            vec![SyncPhase::Idle],
            "no additional progress updates should be emitted after stop_sync"
        );
    }

    #[test]
    fn start_sync_is_blocked_until_in_flight_blocking_scan_unwinds() {
        let _serial = BLOCKING_SCAN_TEST_MUTEX.lock().expect("mutex poisoned");
        let service = SyncService::new();
        let wallet_id = Uuid::new_v4();
        let (temp_dir, app_db) = open_test_app_db();
        let wallet_db_path = temp_dir.path().join("wallet.db");

        let gate = spawn_blocked_scan_job(&service, wallet_id, None);
        gate.wait_until_started();

        service
            .stop_sync(wallet_id, None)
            .expect("stop blocked sync job");

        let err = service
            .start_sync(
                &app_db,
                wallet_id,
                Network::Testnet,
                wallet_db_path.clone(),
                Dek([0u8; 32]),
                vec![],
                None,
                None,
                None,
            )
            .expect_err("restart should be blocked while prior blocking scan is still unwinding");
        let ipc_err = crate::error::find_engine_ipc_error(&err).expect("expected engine IPC error");
        assert_eq!(ipc_err.code, errors::SYNC_IN_PROGRESS);

        gate.release();
        gate.wait_until_finished();
        wait_until_blocking_scans_clear(&service, wallet_id);

        service
            .start_sync(
                &app_db,
                wallet_id,
                Network::Testnet,
                wallet_db_path,
                Dek([0u8; 32]),
                vec![],
                None,
                None,
                None,
            )
            .expect("restart should succeed after blocking scan unwind finishes");
        service
            .stop_sync(wallet_id, None)
            .expect("stop restarted sync");
    }

    #[test]
    fn stop_sync_while_scan_cached_blocks_is_in_flight_resets_idle_and_stops_progress_events() {
        let _serial = BLOCKING_SCAN_TEST_MUTEX.lock().expect("mutex poisoned");
        let service = SyncService::new();
        let wallet_id = Uuid::new_v4();
        let emitted_phases = Arc::new(Mutex::new(Vec::new()));
        let emitted_phases_for_handler = Arc::clone(&emitted_phases);
        let on_progress: SyncEventHandler = Arc::new(move |event| {
            emitted_phases_for_handler
                .lock()
                .expect("mutex poisoned")
                .push(event.progress.phase);
        });

        let gate = spawn_scan_call_blocked_job(&service, wallet_id, Some(Arc::clone(&on_progress)));
        gate.wait_until_started();

        assert!(service.running_wallet_ids().contains(&wallet_id));

        service
            .stop_sync(wallet_id, Some(Arc::clone(&on_progress)))
            .expect("stop sync while scan_cached_blocks is blocked");

        assert_eq!(service.get_progress(wallet_id).phase, SyncPhase::Idle);
        assert!(!service.running_wallet_ids().contains(&wallet_id));

        let phases_after_stop = emitted_phases.lock().expect("mutex poisoned").clone();
        assert_eq!(
            phases_after_stop,
            vec![SyncPhase::Idle],
            "stop_sync should emit a single Idle progress update"
        );

        gate.release();
        gate.wait_until_finished();

        let phases_after_release = emitted_phases.lock().expect("mutex poisoned").clone();
        assert_eq!(
            phases_after_release,
            vec![SyncPhase::Idle],
            "no additional progress updates should be emitted after stop_sync"
        );
    }

    #[test]
    fn start_sync_is_blocked_until_in_flight_scan_cached_blocks_unwinds() {
        let _serial = BLOCKING_SCAN_TEST_MUTEX.lock().expect("mutex poisoned");
        let service = SyncService::new();
        let wallet_id = Uuid::new_v4();
        let (temp_dir, app_db) = open_test_app_db();
        let wallet_db_path = temp_dir.path().join("wallet.db");

        let gate = spawn_scan_call_blocked_job(&service, wallet_id, None);
        gate.wait_until_started();

        service
            .stop_sync(wallet_id, None)
            .expect("stop blocked scan_cached_blocks job");

        let err = service
            .start_sync(
                &app_db,
                wallet_id,
                Network::Testnet,
                wallet_db_path.clone(),
                Dek([0u8; 32]),
                vec![],
                None,
                None,
                None,
            )
            .expect_err(
                "restart should be blocked while prior scan_cached_blocks call is still unwinding",
            );
        let ipc_err = crate::error::find_engine_ipc_error(&err).expect("expected engine IPC error");
        assert_eq!(ipc_err.code, errors::SYNC_IN_PROGRESS);

        gate.release();
        gate.wait_until_finished();
        wait_until_blocking_scans_clear(&service, wallet_id);

        service
            .start_sync(
                &app_db,
                wallet_id,
                Network::Testnet,
                wallet_db_path,
                Dek([0u8; 32]),
                vec![],
                None,
                None,
                None,
            )
            .expect("restart should succeed after scan_cached_blocks unwind finishes");
        service
            .stop_sync(wallet_id, None)
            .expect("stop restarted sync");
    }

    #[test]
    fn get_txids_needing_memo_enhancement_returns_null_memo_txids() {
        // Use tempfile for automatic cleanup even if test panics
        let temp_file = tempfile::Builder::new()
            .suffix(".db")
            .tempfile()
            .expect("create temp file");
        let path = temp_file.path().to_path_buf();
        let dek = Dek([0u8; 32]);

        // Create and populate the database
        {
            let conn = open_sqlcipher_db(
                &path,
                &dek,
                OpenSqlcipherOptions {
                    create_if_missing: true,
                    load_array_module: false,
                },
            )
            .expect("create wallet db");
            create_minimal_wallet_schema(&conn);

            // Insert transaction with NULL memo (needs enhancement)
            let txid_null = [0x01u8; 32];
            conn.execute(
                "INSERT INTO transactions (id_tx, txid) VALUES (1, ?1)",
                [&txid_null[..]],
            )
            .expect("insert tx 1");
            conn.execute(
                "INSERT INTO sapling_received_notes (transaction_id, memo) VALUES (1, NULL)",
                [],
            )
            .expect("insert note with null memo");

            // Insert transaction with populated memo (does not need enhancement)
            let txid_with_memo = [0x02u8; 32];
            conn.execute(
                "INSERT INTO transactions (id_tx, txid) VALUES (2, ?1)",
                [&txid_with_memo[..]],
            )
            .expect("insert tx 2");
            conn.execute(
                "INSERT INTO orchard_received_notes (transaction_id, memo) VALUES (2, X'F600000000')",
                [],
            )
            .expect("insert note with memo");
        }

        // Query using the batch function (offset=0, limit=1000 to get all)
        let txids =
            get_txids_needing_memo_enhancement_batch(&path, &dek, 0, 1000).expect("query txids");

        assert_eq!(txids.len(), 1, "should return only txid with NULL memo");
        assert_eq!(txids[0], [0x01u8; 32]);
        // temp_file dropped here, automatic cleanup
    }

    #[test]
    fn get_txids_needing_memo_enhancement_returns_empty_when_all_memos_populated() {
        let temp_file = tempfile::Builder::new()
            .suffix(".db")
            .tempfile()
            .expect("create temp file");
        let path = temp_file.path().to_path_buf();
        let dek = Dek([0u8; 32]);

        // Create and populate the database with only populated memos
        {
            let conn = open_sqlcipher_db(
                &path,
                &dek,
                OpenSqlcipherOptions {
                    create_if_missing: true,
                    load_array_module: false,
                },
            )
            .expect("create wallet db");
            create_minimal_wallet_schema(&conn);

            let txid = [0x03u8; 32];
            conn.execute(
                "INSERT INTO transactions (id_tx, txid) VALUES (1, ?1)",
                [&txid[..]],
            )
            .expect("insert tx");
            conn.execute(
                "INSERT INTO sapling_received_notes (transaction_id, memo) VALUES (1, X'F600000000')",
                [],
            )
            .expect("insert note with memo");
        }

        let txids =
            get_txids_needing_memo_enhancement_batch(&path, &dek, 0, 1000).expect("query txids");
        assert!(txids.is_empty(), "no txids should need enhancement");
        // temp_file dropped here, automatic cleanup
    }

    #[test]
    fn get_txids_needing_memo_enhancement_deduplicates_across_pools() {
        let temp_file = tempfile::Builder::new()
            .suffix(".db")
            .tempfile()
            .expect("create temp file");
        let path = temp_file.path().to_path_buf();
        let dek = Dek([0u8; 32]);

        // Create transaction with NULL memos in both sapling and orchard tables
        {
            let conn = open_sqlcipher_db(
                &path,
                &dek,
                OpenSqlcipherOptions {
                    create_if_missing: true,
                    load_array_module: false,
                },
            )
            .expect("create wallet db");
            create_minimal_wallet_schema(&conn);

            let txid = [0x04u8; 32];
            conn.execute(
                "INSERT INTO transactions (id_tx, txid) VALUES (1, ?1)",
                [&txid[..]],
            )
            .expect("insert tx");
            // Same transaction has NULL memos in both pools
            conn.execute(
                "INSERT INTO sapling_received_notes (transaction_id, memo) VALUES (1, NULL)",
                [],
            )
            .expect("insert sapling note");
            conn.execute(
                "INSERT INTO orchard_received_notes (transaction_id, memo) VALUES (1, NULL)",
                [],
            )
            .expect("insert orchard note");
        }

        let txids =
            get_txids_needing_memo_enhancement_batch(&path, &dek, 0, 1000).expect("query txids");
        assert_eq!(
            txids.len(),
            1,
            "txid should appear only once despite multiple NULL memo notes"
        );
        // temp_file dropped here, automatic cleanup
    }
}
