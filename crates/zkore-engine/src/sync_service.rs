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

/// Batch size for downloading blocks. Smaller batches improve pipelining granularity.
const BATCH_SIZE: u32 = 100;

/// Number of batches to buffer ahead for download/scan pipelining.
const LOOKAHEAD_BATCHES: usize = 2;

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
            })),
        }
    }

    pub fn get_progress(&self, wallet_id: Uuid) -> SyncProgress {
        self.state
            .lock()
            .expect("mutex poisoned")
            .progress
            .get(&wallet_id)
            .cloned()
            .unwrap_or_else(default_progress)
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
                Some(tor) => zkore_network::grpc_client::GrpcClient::new_with_tor(grpc_url, tor),
                None => zkore_network::grpc_client::GrpcClient::new(grpc_url),
            };

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
                let progress = with_eta(&state, wallet_id, progress);
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
                progress_percent: 2,
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

            // Get chain tip
            let chain_tip = match client.get_latest_block().await {
                Ok((height, _hash)) => height,
                Err(err) => {
                    tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to get chain tip");
                    update(default_progress());
                    let mut state = state.lock().expect("mutex poisoned");
                    state.jobs.remove(&wallet_id);
                    state.started_at.remove(&wallet_id);
                    return;
                }
            };
            tracing::debug!(wallet_id = %wallet_id, chain_tip = %u32::from(chain_tip), "got chain tip");

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

            let params = zcash_consensus_network(network);
            let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
                &mut sync_wallet_conn,
                params,
                zcash_client_sqlite::util::SystemClock,
                rand::rngs::OsRng,
            );

            // Update chain tip in wallet
            if let Err(err) = wdb.update_chain_tip(chain_tip) {
                tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to update chain tip");
                update(default_progress());
                let mut state = state.lock().expect("mutex poisoned");
                state.jobs.remove(&wallet_id);
                state.started_at.remove(&wallet_id);
                return;
            }

            // === Main sync loop ===
            let mut sync_complete = false;
            'sync_loop: loop {
                // Check cancellation at start of each iteration
                if *cancel_rx.borrow() {
                    tracing::debug!(wallet_id = %wallet_id, "sync cancelled");
                    update(default_progress());
                    break 'sync_loop;
                }

                // Get suggested scan ranges
                let ranges = match wdb.suggest_scan_ranges() {
                    Ok(ranges) => ranges,
                    Err(err) => {
                        tracing::error!(wallet_id = %wallet_id, error = ?err, "failed to get scan ranges");
                        update(default_progress());
                        break 'sync_loop;
                    }
                };

                if ranges.is_empty() {
                    tracing::debug!(wallet_id = %wallet_id, "no more ranges to scan, sync complete");
                    sync_complete = true;
                    break 'sync_loop;
                }

                for range in ranges {
                    // Check cancellation before each range
                    if *cancel_rx.borrow() {
                        tracing::debug!(wallet_id = %wallet_id, "sync cancelled during range processing");
                        update(default_progress());
                        break 'sync_loop;
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

                    let wallet_tip = wdb.chain_height().ok().flatten().unwrap_or(range_start);

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
                                tracing::debug!(wallet_id = %download_wallet_id, "download task cancelled");
                                let _ = batch_tx.send(DownloadResult::Cancelled).await;
                                return;
                            }

                            let batch_end = std::cmp::min(current + BATCH_SIZE, range_end);

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
                                    if batch_tx.send(DownloadResult::Batch(batch)).await.is_err() {
                                        // Receiver dropped, scan loop exited early
                                        tracing::debug!(wallet_id = %download_wallet_id, "batch receiver dropped, stopping download");
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
                    update(SyncProgress {
                        phase: SyncPhase::Downloading,
                        scan_frontier_height: u32::from(range_start),
                        wallet_tip_height: u32::from(wallet_tip),
                        progress_percent: calculate_progress(&wdb).max(5),
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
                                update(default_progress());
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
                                    tracing::debug!(wallet_id = %wallet_id, "sync cancelled during scan");
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
                                    && let Err(err) = fsblock_db.write_block_metadata(&block_metas)
                                {
                                    tracing::error!(
                                        wallet_id = %wallet_id,
                                        error = ?err,
                                        "failed to write block metadata"
                                    );
                                }

                                // Update progress (downloading phase while receiving batches)
                                update(SyncProgress {
                                    phase: SyncPhase::Downloading,
                                    scan_frontier_height: u32::from(batch.range_end),
                                    wallet_tip_height: u32::from(wallet_tip),
                                    progress_percent: calculate_progress(&wdb).max(5),
                                    eta_seconds: None,
                                });

                                // Scan the batch immediately after caching
                                // For the first batch in the range, use the pre-fetched tree state.
                                // For subsequent batches, use empty state - the scanner maintains
                                // internal state between calls so it doesn't need the tree state again.
                                let prior_height = batch.range_start.saturating_sub(1);
                                let chain_state = if is_first_batch_in_range {
                                    is_first_batch_in_range = false;
                                    range_chain_state.clone()
                                } else {
                                    empty_chain_state(prior_height)
                                };
                                let limit = batch.blocks.len();

                                if limit > 0 {
                                    update(SyncProgress {
                                        phase: SyncPhase::Scanning,
                                        scan_frontier_height: u32::from(batch.range_start),
                                        wallet_tip_height: u32::from(wallet_tip),
                                        progress_percent: calculate_progress(&wdb).max(10),
                                        eta_seconds: None,
                                    });

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
                                                "failed to scan blocks - transactions in this range may be missed"
                                            );
                                            // Continue - partial scan is ok, but error is logged
                                        }
                                    }

                                    // Clean up scanned blocks from cache
                                    if let Err(err) = fsblock_db.truncate_to_height(prior_height) {
                                        tracing::debug!(
                                            wallet_id = %wallet_id,
                                            error = ?err,
                                            "failed to truncate block cache"
                                        );
                                    }
                                }

                                // Update progress after scan
                                update(SyncProgress {
                                    phase: SyncPhase::Scanning,
                                    scan_frontier_height: u32::from(batch.range_end),
                                    wallet_tip_height: u32::from(wallet_tip),
                                    progress_percent: calculate_progress(&wdb).max(15),
                                    eta_seconds: None,
                                });
                            }
                            DownloadResult::RangeComplete => {
                                tracing::debug!(wallet_id = %wallet_id, "range download complete");
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
                    let _ = download_handle.await;

                    if range_cancelled {
                        update(default_progress());
                        break 'sync_loop;
                    }

                    if range_error {
                        update(default_progress());
                        break 'sync_loop;
                    }

                    // Final progress update for the range
                    update(SyncProgress {
                        phase: SyncPhase::Scanning,
                        scan_frontier_height: u32::from(range_end),
                        wallet_tip_height: u32::from(wallet_tip),
                        progress_percent: calculate_progress(&wdb).max(15),
                        eta_seconds: None,
                    });
                }
            }

            // === Final state ===
            if sync_complete {
                // Final update triggers balance emission via the update closure
                update(SyncProgress {
                    phase: SyncPhase::Idle,
                    scan_frontier_height: u32::from(chain_tip),
                    wallet_tip_height: u32::from(chain_tip),
                    progress_percent: 100,
                    eta_seconds: None,
                });
            }

            // Clean up block cache directory
            if let Err(err) = std::fs::remove_dir_all(&cache_dir) {
                tracing::debug!(wallet_id = %wallet_id, error = ?err, "failed to cleanup block cache");
            }

            // Clear job entry (best effort).
            let mut state = state.lock().expect("mutex poisoned");
            state.jobs.remove(&wallet_id);
            state.started_at.remove(&wallet_id);

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

        self.emit_progress(wallet_id, on_progress.as_ref());
        Ok(())
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

fn with_eta(state: &State, wallet_id: Uuid, mut progress: SyncProgress) -> SyncProgress {
    if progress.progress_percent == 0 || progress.progress_percent >= 100 {
        progress.eta_seconds = None;
        return progress;
    }

    let Some(started_at) = state.started_at.get(&wallet_id) else {
        progress.eta_seconds = None;
        return progress;
    };

    let elapsed = started_at.elapsed().as_secs_f64();
    let done = progress.progress_percent as f64 / 100.0;
    if done <= 0.0 || elapsed <= 0.0 {
        progress.eta_seconds = None;
        return progress;
    }

    let total_estimated = elapsed / done;
    let remaining = (total_estimated - elapsed).max(0.0);
    progress.eta_seconds = Some(remaining.round() as u64);
    progress
}

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}

/// Calculate sync progress percentage from wallet summary.
fn calculate_progress<C, R>(
    wdb: &zcash_client_sqlite::WalletDb<
        C,
        zcash_protocol::consensus::Network,
        zcash_client_sqlite::util::SystemClock,
        R,
    >,
) -> u8
where
    C: std::borrow::BorrowMut<Connection>,
    R: rand::RngCore + rand::CryptoRng,
{
    let summary = wdb
        .get_wallet_summary(ConfirmationsPolicy::default())
        .ok()
        .flatten();
    if let Some(summary) = summary {
        let scan_progress = summary.progress().scan();
        let numerator = *scan_progress.numerator();
        let denominator = *scan_progress.denominator();
        if denominator > 0 {
            return ((numerator as f64 / denominator as f64) * 100.0) as u8;
        }
    }
    0
}

/// Write a compact block to the filesystem block cache.
fn write_block_to_cache(
    blocks_dir: &std::path::Path,
    block: &CompactBlock,
) -> anyhow::Result<BlockMeta> {
    let height = BlockHeight::from_u32(block.height as u32);
    let hash_bytes: [u8; 32] = block.hash.as_slice().try_into().unwrap_or([0u8; 32]);
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

/// Download blocks with retry and exponential backoff.
async fn download_blocks_with_retry(
    client: &zkore_network::grpc_client::GrpcClient,
    start: BlockHeight,
    end: BlockHeight,
    max_retries: u32,
) -> anyhow::Result<Vec<CompactBlock>> {
    let mut attempt = 0;
    loop {
        match client.get_block_range(start, end).await {
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
