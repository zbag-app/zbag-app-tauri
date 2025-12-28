use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Context as _;
use rusqlite::{Connection, OpenFlags};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use uuid::Uuid;
use zeroize::Zeroize;

use zkore_core::domain::{Balance, Network, SyncPhase, SyncProgress};
use zkore_core::errors;
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::events::{BalanceChangedEvent, SyncProgressEvent};

use crate::db::AppDb;
use crate::encryption::Dek;
use crate::error::ipc_err;
use crate::server_resolver;

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

    pub fn start_sync(
        &self,
        app_db: &AppDb,
        wallet_id: Uuid,
        network: Network,
        wallet_db_path: PathBuf,
        wallet_dek: Dek,
        account_ids: Vec<u32>,
        on_progress: Option<SyncEventHandler>,
        on_balance_changed: Option<BalanceEventHandler>,
    ) -> anyhow::Result<()> {
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

        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        let state = Arc::clone(&self.state);
        let on_progress_task = on_progress.clone();
        let on_balance_task = on_balance_changed.clone();

        let handle = tokio::spawn(async move {
            let client = zkore_network::grpc_client::GrpcClient::new(grpc_url);

            let emit = |progress: SyncProgress| {
                if let Some(handler) = on_progress_task.as_ref() {
                    handler(SyncProgressEvent {
                        schema_version: SCHEMA_VERSION,
                        event: "sync.progress".to_string(),
                        progress: progress.clone(),
                    });
                }
            };

            let mut wallet_db = match on_balance_task.as_ref() {
                Some(_) => open_wallet_db(&wallet_db_path, &wallet_dek).ok(),
                None => None,
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

            update(SyncProgress {
                phase: SyncPhase::Downloading,
                scan_frontier_height: 0,
                wallet_tip_height: 0,
                progress_percent: 5,
                eta_seconds: None,
            });

            // Skeleton: connect and ensure CompactTxStreamer is reachable.
            let connect_fut = client.connect();
            tokio::select! {
                _ = cancel_rx.changed() => {
                    update(default_progress());
                }
                res = connect_fut => {
                    if res.is_ok() {
                        update(SyncProgress {
                            phase: SyncPhase::Scanning,
                            scan_frontier_height: 0,
                            wallet_tip_height: 0,
                            progress_percent: 30,
                            eta_seconds: None,
                        });

                        tokio::select! {
                            _ = cancel_rx.changed() => {
                                update(default_progress());
                            }
                            _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
                        }

                        update(SyncProgress {
                            phase: SyncPhase::Enhancing,
                            scan_frontier_height: 0,
                            wallet_tip_height: 0,
                            progress_percent: 70,
                            eta_seconds: None,
                        });

                        tokio::select! {
                            _ = cancel_rx.changed() => {
                                update(default_progress());
                            }
                            _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
                        }

                        update(SyncProgress {
                            phase: SyncPhase::CatchingUp,
                            scan_frontier_height: 0,
                            wallet_tip_height: 0,
                            progress_percent: 90,
                            eta_seconds: None,
                        });

                        tokio::select! {
                            _ = cancel_rx.changed() => {
                                update(default_progress());
                            }
                            _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
                        }

                        update(SyncProgress {
                            phase: SyncPhase::Idle,
                            scan_frontier_height: 0,
                            wallet_tip_height: 0,
                            progress_percent: 100,
                            eta_seconds: None,
                        });
                    } else {
                        update(default_progress());
                    }
                }
            }

            // Clear job entry (best effort).
            let mut state = state.lock().expect("mutex poisoned");
            state.jobs.remove(&wallet_id);
            state.started_at.remove(&wallet_id);
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
