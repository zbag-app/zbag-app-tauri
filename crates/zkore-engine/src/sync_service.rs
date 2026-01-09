use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Context as _;
use rusqlite::{Connection, OpenFlags};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use uuid::Uuid;
use zeroize::Zeroize;

use std::io::Write as _;

use prost::Message as _;
use zcash_client_backend::data_api::chain::{ChainState, scan_cached_blocks};
use zcash_client_backend::data_api::scanning::ScanPriority;
use zcash_client_backend::data_api::wallet::ConfirmationsPolicy;
use zcash_client_backend::data_api::{WalletRead as _, WalletWrite as _};
use zcash_client_backend::proto::compact_formats::CompactBlock;
use zcash_client_sqlite::FsBlockDb;
use zcash_client_sqlite::chain::BlockMeta;
use zcash_client_sqlite::chain::init::init_blockmeta_db;
use zcash_primitives::block::BlockHash;
use zcash_protocol::consensus::BlockHeight;

use zkore_core::domain::{Balance, Network, SyncPhase, SyncProgress};
use zkore_core::errors;
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::events::{BalanceChangedEvent, SyncProgressEvent};

use crate::db::AppDb;
use crate::encryption::Dek;
use crate::error::ipc_err;
use crate::server_resolver;

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
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(20);

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
        tor_manager: Option<std::sync::Arc<zkore_tor::TorManager>>,
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

            let client = match tor_manager {
                Some(ref tor) => {
                    zkore_network::grpc_client::GrpcClient::new_with_tor(grpc_url, Arc::clone(tor))
                }
                None => zkore_network::grpc_client::GrpcClient::new(grpc_url),
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
                    if tor_state.status == zkore_core::domain::TorStatus::On {
                        tracing::info!(wallet_id = %wallet_id, "Tor connected, starting sync");
                        break;
                    }

                    // If Tor is in error state, log and continue (will fail in main loop)
                    if tor_state.status == zkore_core::domain::TorStatus::Error {
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

            let mut wallet_db = if on_balance_task.as_ref().is_some() {
                match open_wallet_db(&wallet_db_path, &wallet_dek) {
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

            let maybe_emit_balances = |wallet_db: &mut Option<Connection>| {
                let Some(handler) = on_balance_task.as_ref() else {
                    return;
                };
                let Some(db) = wallet_db.as_mut() else {
                    return;
                };

                for account_id in &account_ids {
                    let Ok(balance) = crate::balance::get_balance(db, network, *account_id) else {
                        continue;
                    };

                    if record_balance(&state, wallet_id, *account_id, &balance) {
                        handler(BalanceChangedEvent {
                            schema_version: SCHEMA_VERSION,
                            event: "balance.changed".to_string(),
                            account_id: *account_id,
                            balance,
                        });
                    }
                }
            };

            let mut update = |progress: SyncProgress| {
                let mut state = state.lock().expect("mutex poisoned");
                let progress = with_eta(&mut state, wallet_id, progress);
                state.progress.insert(wallet_id, progress.clone());
                drop(state);
                emit(progress);
                maybe_emit_balances(&mut wallet_db);
            };

            // === Phase: Preparing ===
            update(SyncProgress {
                phase: SyncPhase::Preparing,
                scan_frontier_height: 0,
                wallet_tip_height: 0,
                progress_percent: 0,
                eta_seconds: None,
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

            // Initialize block cache directory
            let cache_dir = wallet_db_path
                .parent()
                .unwrap_or(&wallet_db_path)
                .join("block_cache");
            if let Err(err) = std::fs::create_dir_all(&cache_dir) {
                tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to create block cache dir");
                update(default_progress());
                let mut state = state.lock().expect("mutex poisoned");
                state.jobs.remove(&wallet_id);
                state.started_at.remove(&wallet_id);
                return;
            }

            // Initialize FsBlockDb
            let mut fsblock_db = match FsBlockDb::for_path(&cache_dir) {
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

            // Initialize the block metadata database schema
            if let Err(err) = init_blockmeta_db(&mut fsblock_db) {
                tracing::warn!(wallet_id = %wallet_id, error = ?err, "failed to init blockmeta db schema");
                // Continue - scanning might still work without metadata
            }

            // Open wallet DB for sync operations
            let mut sync_wallet_conn = match open_wallet_db(&wallet_db_path, &wallet_dek) {
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
            if let Err(err) =
                backfill_birthday_tree_sizes(&mut sync_wallet_conn, &client, wallet_id).await
            {
                tracing::warn!(
                    wallet_id = %wallet_id,
                    error = ?err,
                    "failed to backfill account birthday tree sizes; progress percent may be inaccurate"
                );
            }

            let params = zcash_consensus_network(network);
            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut sync_wallet_conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            let mut poll_backoff = POLL_INTERVAL;

            // === Persistent sync loop ===
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
                        let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);
                        let wallet_tip_height = wdb
                            .chain_height()
                            .ok()
                            .flatten()
                            .map(u32::from)
                            .unwrap_or(0);
                        update(SyncProgress {
                            phase: SyncPhase::Idle,
                            scan_frontier_height: fully_scanned,
                            wallet_tip_height,
                            progress_percent,
                            eta_seconds: None,
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

                // Update chain tip in wallet (retry with backoff on transient errors).
                if let Err(err) = wdb.update_chain_tip(chain_tip) {
                    tracing::warn!(
                        wallet_id = %wallet_id,
                        error = ?err,
                        "failed to update chain tip"
                    );
                    let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);
                    update(SyncProgress {
                        phase: SyncPhase::Idle,
                        scan_frontier_height: fully_scanned,
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent,
                        eta_seconds: None,
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
                'sync_loop: loop {
                    // Check cancellation at start of each iteration
                    if *cancel_rx.borrow() {
                        tracing::debug!(wallet_id = %wallet_id, "sync cancelled");
                        update(default_progress());
                        break 'auto_sync;
                    }

                    // Get suggested scan ranges
                    let ranges = match wdb.suggest_scan_ranges() {
                        Ok(ranges) => ranges,
                        Err(err) => {
                            tracing::error!(
                                wallet_id = %wallet_id,
                                error = ?err,
                                "failed to get scan ranges"
                            );
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

                        let wallet_tip = wdb.chain_height().ok().flatten().unwrap_or_else(|| {
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
                        let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);
                        let initial_frontier =
                            fully_scanned.max(u32::from(range_start.saturating_sub(1)));
                        update(SyncProgress {
                            phase: SyncPhase::Downloading,
                            scan_frontier_height: initial_frontier,
                            wallet_tip_height: u32::from(wallet_tip),
                            progress_percent,
                            eta_seconds: None,
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

                                    // Write blocks to cache
                                    let mut block_metas = Vec::new();
                                    for block in &batch.blocks {
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
                                        && let Err(err) =
                                            fsblock_db.write_block_metadata(&block_metas)
                                    {
                                        tracing::error!(
                                            wallet_id = %wallet_id,
                                            error = ?err,
                                            "failed to write block metadata"
                                        );
                                    }

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
                                                range_error = true;
                                                break;
                                            }
                                        }
                                    };
                                    let limit = batch.blocks.len();

                                    if limit > 0 {
                                        match scan_cached_blocks(
                                            &params,
                                            &fsblock_db,
                                            &mut wdb,
                                            batch.range_start,
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
                                            }
                                            Err(err) => {
                                                tracing::error!(
                                                    wallet_id = %wallet_id,
                                                    range_start = %u32::from(batch.range_start),
                                                    limit = limit,
                                                    error = ?err,
                                                    "failed to scan blocks, aborting range"
                                                );
                                                range_error = true;
                                                break;
                                            }
                                        }

                                        // Clean up scanned blocks from cache (metadata)
                                        if let Err(err) =
                                            fsblock_db.truncate_to_height(prior_height)
                                        {
                                            tracing::debug!(
                                                wallet_id = %wallet_id,
                                                error = ?err,
                                                "failed to truncate block cache metadata"
                                            );
                                        }

                                        // Delete the actual block files to prevent accumulation
                                        delete_cached_block_files(
                                            &blocks_dir,
                                            batch.range_start,
                                            batch.range_end,
                                        );
                                    }

                                    // Update progress after scan
                                    let (progress_percent, fully_scanned) =
                                        calculate_progress_and_height(&wdb);
                                    update(SyncProgress {
                                        phase: SyncPhase::Scanning,
                                        scan_frontier_height: fully_scanned,
                                        wallet_tip_height: u32::from(wallet_tip),
                                        progress_percent,
                                        eta_seconds: None,
                                    });
                                }
                                DownloadResult::RangeComplete => {
                                    tracing::debug!(
                                        wallet_id = %wallet_id,
                                        "range download complete"
                                    );
                                    break;
                                }
                                DownloadResult::Error(_err) => {
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

                        // Final progress update for the range
                        let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);
                        update(SyncProgress {
                            phase: SyncPhase::Scanning,
                            scan_frontier_height: fully_scanned,
                            wallet_tip_height: u32::from(wallet_tip),
                            progress_percent,
                            eta_seconds: None,
                        });
                    }
                }

                if sync_error {
                    let (progress_percent, fully_scanned) = calculate_progress_and_height(&wdb);
                    update(SyncProgress {
                        phase: SyncPhase::Idle,
                        scan_frontier_height: fully_scanned,
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent,
                        eta_seconds: None,
                    });

                    tokio::time::sleep(poll_backoff).await;
                    poll_backoff = poll_backoff
                        .checked_mul(2)
                        .unwrap_or(MAX_POLL_BACKOFF)
                        .min(MAX_POLL_BACKOFF);
                    continue 'auto_sync;
                }

                if sync_complete {
                    // Final update triggers balance emission via the update closure
                    update(SyncProgress {
                        phase: SyncPhase::CatchingUp,
                        scan_frontier_height: u32::from(chain_tip),
                        wallet_tip_height: u32::from(chain_tip),
                        progress_percent: 100,
                        eta_seconds: None,
                    });
                }

                tokio::time::sleep(POLL_INTERVAL).await;
            }

            // Clean up block cache directory
            if let Err(e) = std::fs::remove_dir_all(&cache_dir) {
                tracing::debug!(path = ?cache_dir, error = ?e, "failed to cleanup block cache directory");
            }

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

fn open_wallet_db(wallet_db_path: &PathBuf, dek: &Dek) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(wallet_db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .with_context(|| format!("failed to open wallet db: {}", wallet_db_path.display()))?;

    let mut dek_hex = dek.0.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");
    conn.execute_batch(&pragma)
        .context("failed to apply wallet db encryption key")?;

    dek_hex.zeroize();
    pragma.zeroize();

    rusqlite::vtab::array::load_module(&conn).context("failed to load sqlite array module")?;

    // Force an early read to detect an incorrect key.
    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
        .context("wallet db is not readable (incorrect key or corrupted db)")?;

    Ok(conn)
}

fn default_progress() -> SyncProgress {
    SyncProgress {
        phase: SyncPhase::Idle,
        scan_frontier_height: 0,
        wallet_tip_height: 0,
        progress_percent: 0,
        eta_seconds: None,
    }
}

fn with_eta(state: &mut State, wallet_id: Uuid, mut progress: SyncProgress) -> SyncProgress {
    // Reset/clear ETA once syncing is done (or aborted) so we don't leak stale estimates.
    if progress.phase == SyncPhase::Idle {
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

async fn backfill_birthday_tree_sizes(
    conn: &mut Connection,
    client: &zkore_network::grpc_client::GrpcClient,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<()> {
    let birthday_heights = {
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
            .context("failed to read account birthday heights")?
    };

    for birthday_height in birthday_heights {
        let prior_height =
            zcash_protocol::consensus::BlockHeight::from(birthday_height.saturating_sub(1));

        let chain_state = fetch_chain_state(client, prior_height, wallet_id).await?;

        let sapling_tree_size = chain_state.final_sapling_tree().tree_size();
        let orchard_tree_size = chain_state.final_orchard_tree().tree_size();

        let rows = conn
            .execute(
                "UPDATE accounts
                 SET birthday_sapling_tree_size = ?1,
                     birthday_orchard_tree_size = ?2
                 WHERE birthday_height = ?3",
                rusqlite::params![sapling_tree_size, orchard_tree_size, birthday_height],
            )
            .context("failed to update account birthday tree sizes")?;

        if rows > 0 {
            tracing::debug!(
                wallet_id = %wallet_id,
                birthday_height,
                sapling_tree_size,
                orchard_tree_size,
                updated_accounts = rows,
                "backfilled account birthday tree sizes"
            );
        }
    }

    Ok(())
}

/// Write a compact block to the filesystem block cache.
fn write_block_to_cache(
    blocks_dir: &std::path::Path,
    block: &CompactBlock,
) -> anyhow::Result<BlockMeta> {
    let height = BlockHeight::from_u32(block.height as u32);
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
    let mut file = std::fs::File::create(&block_path)
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
    client: &zkore_network::grpc_client::GrpcClient,
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

/// Download blocks with retry and exponential backoff.
async fn download_blocks_with_retry(
    client: &zkore_network::grpc_client::GrpcClient,
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
