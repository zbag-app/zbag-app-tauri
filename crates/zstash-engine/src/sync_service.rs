use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

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

use zstash_core::domain::{Balance, Network, SyncPhase, SyncProgress};
use zstash_core::errors;
use zstash_core::ipc::v1::common::SCHEMA_VERSION;
use zstash_core::ipc::v1::events::{BalanceChangedEvent, SyncProgressEvent};

use crate::db::{AppDb, OpenSqlcipherOptions, open_sqlcipher_db};
use crate::encryption::Dek;
use crate::error::ipc_err;
use crate::server_resolver;
use zstash_core::permissions::{create_dir_all_secure, secure_open_options};

/// Default batch size for downloading blocks.
/// Matches Zashi's SYNC_BATCH_SIZE for optimal performance.
const BATCH_SIZE: u32 = 1000;

/// Smaller batch size for sandblasting periods where blocks are much larger.
/// Matches Zashi's SYNC_BATCH_SMALL_SIZE.
const BATCH_SIZE_SANDBLASTING: u32 = 100;

/// Known Zcash mainnet sandblasting period (blocks 1.71M to 2.05M).
/// During this range, we use smaller batches due to larger block sizes.
const SANDBLASTING_RANGE: std::ops::RangeInclusive<u32> = 1_710_000..=2_050_000;

/// Number of batches to buffer ahead for download/scan pipelining.
const LOOKAHEAD_BATCHES: usize = 2;

/// Poll interval once the wallet is caught up to tip.
pub(crate) const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(20);

/// Maximum backoff after repeated sync failures.
const MAX_POLL_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// A downloaded batch of blocks ready for scanning.
struct DownloadedBatch {
    range_start: BlockHeight,
    range_end: BlockHeight,
    blocks: Vec<CompactBlock>,
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

#[derive(Debug)]
struct SyncJob {
    cancel: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

impl SyncService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                jobs: HashMap::new(),
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
        tor_manager: Option<std::sync::Arc<zstash_tor::TorManager>>,
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
            if state.jobs.contains_key(&wallet_id) {
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
                    zstash_network::grpc_client::GrpcClient::new_with_tor(grpc_url, Arc::clone(tor))
                }
                None => zstash_network::grpc_client::GrpcClient::new(grpc_url),
            };

            // Wait for Tor to be ready if enabled but not connected
            if let Some(ref tor) = tor_manager {
                loop {
                    if *cancel_rx.borrow() {
                        tracing::debug!(wallet_id = %wallet_id, "sync cancelled while waiting for Tor");
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
                    if tor_state.status == zstash_core::domain::TorStatus::On {
                        tracing::info!(wallet_id = %wallet_id, "Tor connected, starting sync");
                        break;
                    }

                    // If Tor is in error state, log and continue (will fail in main loop)
                    if tor_state.status == zstash_core::domain::TorStatus::Error {
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

            let mut balance_db = if on_balance_task.as_ref().is_some() {
                // Copy DEK bytes for the balance connection (original stays with sync)
                match open_wallet_db_async(wallet_db_path.clone(), Dek(wallet_dek.0)).await {
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
                update(default_progress());
                let mut state = state.lock().expect("mutex poisoned");
                state.jobs.remove(&wallet_id);
                state.started_at.remove(&wallet_id);
                return;
            }

            // Initialize FsBlockDb (on blocking thread)
            let mut fsblock_db = match init_fsblock_db_async(cache_dir.clone()).await {
                Ok(db) => db,
                Err(err) => {
                    tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to init FsBlockDb");
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
                Dek(wallet_dek.0),
            )
            .await
            {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to open wallet db for sync");
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
                backfill_birthday_tree_sizes(sync_wallet_conn, &client, wallet_id).await;
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
                // Check cancellation at start of each iteration
                if *cancel_rx.borrow() {
                    tracing::debug!(wallet_id = %wallet_id, "sync cancelled");
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
                            get_progress_blocking(sync_wallet_conn, params).await;
                        sync_wallet_conn = conn;
                        let wallet_tip_height = chain_height.map(u32::from).unwrap_or(0);
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
                    update_chain_tip_blocking(sync_wallet_conn, params, chain_tip).await;
                sync_wallet_conn = conn;
                if let Err(err) = update_result {
                    tracing::warn!(
                        wallet_id = %wallet_id,
                        error = %err,
                        "failed to update chain tip"
                    );
                    let (conn, progress_percent, fully_scanned, _) =
                        get_progress_blocking(sync_wallet_conn, params).await;
                    sync_wallet_conn = conn;
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
                let mut sync_error_message: Option<&'static str> = None;
                'sync_loop: loop {
                    // Check cancellation at start of each iteration
                    if *cancel_rx.borrow() {
                        tracing::debug!(wallet_id = %wallet_id, "sync cancelled");
                        update(default_progress());
                        break 'auto_sync;
                    }

                    // Get suggested scan ranges on blocking thread
                    let (conn, ranges_result) =
                        suggest_scan_ranges_blocking(sync_wallet_conn, params).await;
                    sync_wallet_conn = conn;
                    let ranges = match ranges_result {
                        Ok(ranges) => ranges,
                        Err(err) => {
                            tracing::error!(
                                wallet_id = %wallet_id,
                                error = %err,
                                "failed to get scan ranges"
                            );
                            sync_error_message = Some("Failed to determine scan ranges");
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
                            get_progress_blocking(sync_wallet_conn, params).await;
                        sync_wallet_conn = conn;
                        let wallet_tip = chain_height.unwrap_or_else(|| {
                            tracing::debug!("chain height unavailable for progress calculation");
                            range_start
                        });

                        // === Pipelined download and scan ===
                        // Create channel for downloaded batches
                        let (batch_tx, mut batch_rx) =
                            mpsc::channel::<DownloadResult>(LOOKAHEAD_BATCHES);

                        // Clone what the download task needs
                        let download_client = client.clone();
                        let download_cancel_rx = cancel_rx.clone();
                        let download_wallet_id = wallet_id;

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

                                let batch_size = effective_batch_size(current);
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
                            get_progress_blocking(sync_wallet_conn, params).await;
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
                                    sync_error_message = Some("Failed to fetch chain state");
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
                                DownloadResult::Batch(batch) => {
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
                                    let (db, block_metas) = write_blocks_to_cache_async(
                                        blocks_dir.clone(),
                                        batch.blocks.clone(),
                                        fsblock_db,
                                        wallet_id,
                                    )
                                    .await;
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
                                                    Some("Failed to fetch chain state");
                                                range_error = true;
                                                break;
                                            }
                                        }
                                    };
                                    let limit = block_metas.len();

                                    // Scan blocks on blocking thread
                                    let scan_result = scan_batch_blocking(
                                        sync_wallet_conn,
                                        fsblock_db,
                                        params,
                                        batch.range_start,
                                        chain_state,
                                        limit,
                                        blocks_dir.clone(),
                                        batch.range_end,
                                        wallet_id,
                                    )
                                    .await;

                                    // Restore ownership
                                    sync_wallet_conn = scan_result.conn;
                                    fsblock_db = scan_result.fsblock_db;

                                    match scan_result.scan_outcome {
                                        ScanOutcome::Success => {
                                            // Update progress after scan
                                            update(SyncProgress {
                                                phase: SyncPhase::Scanning,
                                                scan_frontier_height: scan_result.fully_scanned,
                                                wallet_tip_height: u32::from(wallet_tip),
                                                progress_percent: scan_result.progress_percent,
                                                eta_seconds: None,
                                            });

                                            // Emit balance updates on blocking thread
                                            if let Some(db) = balance_db.take() {
                                                balance_db = Some(
                                                    emit_balances_blocking(
                                                        db,
                                                        network,
                                                        account_ids.clone(),
                                                        Arc::clone(&state),
                                                        wallet_id,
                                                        on_balance_task.clone(),
                                                    )
                                                    .await,
                                                );
                                            }
                                        }
                                        ScanOutcome::ReorgDetected { rewind_height: _ } => {
                                            // Break to re-fetch scan ranges with the rewound state.
                                            // Don't set range_error - this is a recoverable situation.
                                            break;
                                        }
                                        ScanOutcome::Error(_) => {
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
                                    sync_error_message = Some("Failed to download blocks");
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
                            update(default_progress());
                            break 'auto_sync;
                        }

                        if range_error {
                            sync_error = true;
                            break 'sync_loop;
                        }

                        // Final progress update for the range (on blocking thread)
                        let (conn, progress_percent, fully_scanned, _) =
                            get_progress_blocking(sync_wallet_conn, params).await;
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
                        get_progress_blocking(sync_wallet_conn, params).await;
                    sync_wallet_conn = conn;
                    update(SyncProgress {
                        phase: SyncPhase::Error,
                        scan_frontier_height: fully_scanned,
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent,
                        eta_seconds: None,
                        retry_in_seconds: Some(retry_in_seconds),
                        error_message: sync_error_message.map(str::to_string),
                    });

                    tokio::time::sleep(poll_backoff).await;
                    poll_backoff = poll_backoff
                        .checked_mul(2)
                        .unwrap_or(MAX_POLL_BACKOFF)
                        .min(MAX_POLL_BACKOFF);
                    continue 'auto_sync;
                }

                if sync_complete {
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

                    // Emit final balance updates on blocking thread
                    if let Some(db) = balance_db.take() {
                        balance_db = Some(
                            emit_balances_blocking(
                                db,
                                network,
                                account_ids.clone(),
                                Arc::clone(&state),
                                wallet_id,
                                on_balance_task.clone(),
                            )
                            .await,
                        );
                    }
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
        let job = self
            .state
            .lock()
            .expect("mutex poisoned")
            .jobs
            .remove(&wallet_id);

        let Some(job) = job else {
            return Ok(());
        };

        let _ = job.cancel.send(true);
        job.handle.abort();

        self.state
            .lock()
            .expect("mutex poisoned")
            .progress
            .insert(wallet_id, default_progress());

        self.state
            .lock()
            .expect("mutex poisoned")
            .started_at
            .remove(&wallet_id);
        self.state
            .lock()
            .expect("mutex poisoned")
            .progress_estimates
            .remove(&wallet_id);

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
) -> (Connection, anyhow::Result<Vec<u32>>) {
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
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Update account birthday tree sizes on a blocking thread.
async fn update_birthday_tree_sizes_blocking(
    conn: Connection,
    birthday_height: u32,
    sapling_tree_size: u64,
    orchard_tree_size: u64,
    wallet_id: uuid::Uuid,
) -> (Connection, anyhow::Result<usize>) {
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
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Backfill account birthday tree sizes from lightwalletd.
///
/// This function queries birthday heights on a blocking thread, fetches tree states
/// via async network calls, and updates the database on blocking threads.
async fn backfill_birthday_tree_sizes(
    conn: Connection,
    client: &zstash_network::grpc_client::GrpcClient,
    wallet_id: uuid::Uuid,
) -> (Connection, anyhow::Result<()>) {
    // Query birthday heights on blocking thread
    let (mut conn, heights_result) = query_birthday_heights_blocking(conn).await;

    let birthday_heights = match heights_result {
        Ok(heights) => heights,
        Err(e) => return (conn, Err(e)),
    };

    for birthday_height in birthday_heights {
        let prior_height =
            zcash_protocol::consensus::BlockHeight::from(birthday_height.saturating_sub(1));

        // Async network call
        let chain_state = match fetch_chain_state(client, prior_height, wallet_id).await {
            Ok(state) => state,
            Err(e) => return (conn, Err(e)),
        };

        let sapling_tree_size = chain_state.final_sapling_tree().tree_size();
        let orchard_tree_size = chain_state.final_orchard_tree().tree_size();

        // Update on blocking thread
        let (returned_conn, update_result) = update_birthday_tree_sizes_blocking(
            conn,
            birthday_height,
            sapling_tree_size,
            orchard_tree_size,
            wallet_id,
        )
        .await;
        conn = returned_conn;

        if let Err(e) = update_result {
            return (conn, Err(e));
        }
    }

    (conn, Ok(()))
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
    client: &zstash_network::grpc_client::GrpcClient,
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
    client: &zstash_network::grpc_client::GrpcClient,
    wallet_db_path: &Path,
    wallet_dek: &Dek,
    params: &zcash_protocol::consensus::Network,
    txid_bytes: [u8; 32],
) -> anyhow::Result<()> {
    let txid = TxId::from_bytes(txid_bytes);

    // 1. Fetch the raw transaction
    let raw_tx = client.get_transaction(&txid).await?;

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
    let branch_id = BranchId::for_height(params, mined_height);

    // 3. Parse the transaction
    let tx =
        Transaction::read(&raw_tx.data[..], branch_id).context("failed to parse transaction")?;

    // 4. Open a separate connection for enhancement to avoid borrow conflicts
    let mut conn = open_wallet_db(wallet_db_path, wallet_dek)?;

    // 5. Get UFVKs for decryption using the WalletRead trait
    let ufvks = {
        let wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            *params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );
        wdb.get_unified_full_viewing_keys()?
    };

    // 6. Decrypt the transaction
    let decrypted = decrypt_transaction(params, Some(mined_height), None, &tx, &ufvks);

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
    client: &zstash_network::grpc_client::GrpcClient,
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

/// Initialize block cache directory on a blocking thread.
async fn create_cache_dir_async(cache_dir: PathBuf) -> std::io::Result<()> {
    crate::tokio_runtime::spawn_blocking(move || std::fs::create_dir_all(&cache_dir))
        .await
        .expect("spawn_blocking panicked")
}

/// Initialize FsBlockDb on a blocking thread.
async fn init_fsblock_db_async(
    cache_dir: PathBuf,
) -> Result<FsBlockDb, zcash_client_sqlite::FsBlockDbError> {
    crate::tokio_runtime::spawn_blocking(move || {
        let mut db = FsBlockDb::for_path(&cache_dir)?;
        // Initialize the block metadata database schema
        if let Err(err) = init_blockmeta_db(&mut db) {
            tracing::warn!(error = ?err, "failed to init blockmeta db schema");
        }
        Ok(db)
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Remove cache directory on a blocking thread.
async fn remove_cache_dir_async(cache_dir: PathBuf) {
    let _ = crate::tokio_runtime::spawn_blocking(move || {
        if let Err(e) = std::fs::remove_dir_all(&cache_dir) {
            tracing::debug!(path = ?cache_dir, error = ?e, "failed to cleanup block cache directory");
        }
    })
    .await;
}

/// Write blocks to cache and register metadata on a blocking thread.
/// Returns the block metadata for successfully cached blocks.
async fn write_blocks_to_cache_async(
    blocks_dir: PathBuf,
    blocks: Vec<CompactBlock>,
    fsblock_db: FsBlockDb,
    wallet_id: uuid::Uuid,
) -> (FsBlockDb, Vec<BlockMeta>) {
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
        let fsblock_db = fsblock_db;
        if !block_metas.is_empty() {
            if let Err(err) = fsblock_db.write_block_metadata(&block_metas) {
                tracing::error!(
                    wallet_id = %wallet_id,
                    error = ?err,
                    "failed to write block metadata"
                );
            }
        }

        (fsblock_db, block_metas)
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Delete cached block files on a blocking thread.
async fn delete_cached_block_files_async(
    blocks_dir: PathBuf,
    start: BlockHeight,
    end: BlockHeight,
) {
    let _ = crate::tokio_runtime::spawn_blocking(move || {
        delete_cached_block_files(&blocks_dir, start, end);
    })
    .await;
}

/// Truncate FsBlockDb on a blocking thread.
async fn truncate_fsblock_db_async(
    fsblock_db: FsBlockDb,
    height: BlockHeight,
    wallet_id: uuid::Uuid,
) -> FsBlockDb {
    crate::tokio_runtime::spawn_blocking(move || {
        let db = fsblock_db;
        if let Err(err) = db.truncate_to_height(height) {
            tracing::debug!(
                wallet_id = %wallet_id,
                error = ?err,
                "failed to truncate block cache metadata"
            );
        }
        db
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Result of a batch scan operation.
struct ScanBatchResult {
    conn: Connection,
    fsblock_db: FsBlockDb,
    scan_outcome: ScanOutcome,
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
    limit: usize,
    blocks_dir: PathBuf,
    batch_range_end: BlockHeight,
    wallet_id: uuid::Uuid,
) -> ScanBatchResult {
    crate::tokio_runtime::spawn_blocking(move || {
        let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let scan_outcome = if limit > 0 {
            match scan_cached_blocks(
                &params,
                &fsblock_db,
                &mut wdb,
                batch_range_start,
                &chain_state,
                limit,
            ) {
                Ok(scan_result) => {
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
                        delete_cached_block_files(&blocks_dir, rewind_height + 1, batch_range_end);

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
            delete_cached_block_files(&blocks_dir, batch_range_start, batch_range_end);
        }

        // Calculate progress while we still have wdb
        let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);

        // Drop wdb to release borrow on conn
        drop(wdb);

        ScanBatchResult {
            conn,
            fsblock_db,
            scan_outcome,
            progress_percent,
            fully_scanned,
        }
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Run update_chain_tip on a blocking thread.
async fn update_chain_tip_blocking(
    mut conn: Connection,
    params: zcash_protocol::consensus::Network,
    chain_tip: BlockHeight,
) -> (Connection, Result<(), String>) {
    crate::tokio_runtime::spawn_blocking(move || {
        let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let result = wdb.update_chain_tip(chain_tip).map_err(|e| format!("{e}"));

        drop(wdb);
        (conn, result)
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Get suggested scan ranges on a blocking thread.
async fn suggest_scan_ranges_blocking(
    mut conn: Connection,
    params: zcash_protocol::consensus::Network,
) -> (
    Connection,
    Result<Vec<zcash_client_backend::data_api::scanning::ScanRange>, String>,
) {
    crate::tokio_runtime::spawn_blocking(move || {
        let wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let result = wdb.suggest_scan_ranges().map_err(|e| format!("{e}"));

        drop(wdb);
        (conn, result)
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Get chain height and progress on a blocking thread.
async fn get_progress_blocking(
    mut conn: Connection,
    params: zcash_protocol::consensus::Network,
) -> (Connection, u8, u32, Option<BlockHeight>) {
    crate::tokio_runtime::spawn_blocking(move || {
        let wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);
        let chain_height = wdb.chain_height().ok().flatten();

        drop(wdb);
        (conn, progress_percent, fully_scanned, chain_height)
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Get balance on a blocking thread.
async fn get_balance_blocking(
    mut conn: Connection,
    network: Network,
    account_id: u32,
) -> (Connection, anyhow::Result<Balance>) {
    crate::tokio_runtime::spawn_blocking(move || {
        let result = crate::balance::get_balance(&mut conn, network, account_id);
        (conn, result)
    })
    .await
    .expect("spawn_blocking panicked")
}

/// Emit balance changed events for all accounts on a blocking thread.
///
/// Returns the connection and a vector of (account_id, balance) pairs for accounts
/// whose balance has changed since the last emission.
async fn emit_balances_blocking(
    conn: Connection,
    network: Network,
    account_ids: Vec<u32>,
    state: Arc<Mutex<State>>,
    wallet_id: uuid::Uuid,
    handler: Option<BalanceEventHandler>,
) -> Connection {
    let Some(handler) = handler else {
        return conn;
    };

    crate::tokio_runtime::spawn_blocking(move || {
        let mut conn = conn;
        for account_id in account_ids {
            let Ok(balance) = crate::balance::get_balance(&mut conn, network, account_id) else {
                continue;
            };

            if record_balance(&state, wallet_id, account_id, &balance) {
                handler(BalanceChangedEvent {
                    schema_version: SCHEMA_VERSION,
                    event: "balance.changed".to_string(),
                    account_id,
                    balance,
                });
            }
        }
        conn
    })
    .await
    .expect("spawn_blocking panicked")
}
