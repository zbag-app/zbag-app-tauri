use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use tokio::task::JoinHandle;
use tokio::sync::watch;
use uuid::Uuid;

use zkore_core::domain::{Network, SyncPhase, SyncProgress};
use zkore_core::errors;
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::events::SyncProgressEvent;

use crate::db::AppDb;
use crate::error::ipc_err;
use crate::server_resolver;

type SyncEventHandler = Arc<dyn Fn(SyncProgressEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct SyncService {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    jobs: HashMap<Uuid, SyncJob>,
    progress: HashMap<Uuid, SyncProgress>,
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
        on_progress: Option<SyncEventHandler>,
    ) -> anyhow::Result<()> {
        {
            let mut state = self.state.lock().expect("mutex poisoned");
            if state.jobs.contains_key(&wallet_id) {
                return Err(ipc_err(errors::SYNC_IN_PROGRESS, "sync already in progress"));
            }

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

            let update = |progress: SyncProgress| {
                let mut state = state.lock().expect("mutex poisoned");
                state.progress.insert(wallet_id, progress.clone());
                drop(state);
                emit(progress);
            };

            update(SyncProgress {
                phase: SyncPhase::Downloading,
                scan_frontier_height: 0,
                wallet_tip_height: 0,
                progress_percent: 10,
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
        });

        let finished = handle.is_finished();
        self.state
            .lock()
            .expect("mutex poisoned")
            .jobs
            .insert(wallet_id, SyncJob { cancel: cancel_tx, handle });
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

fn default_progress() -> SyncProgress {
    SyncProgress {
        phase: SyncPhase::Idle,
        scan_frontier_height: 0,
        wallet_tip_height: 0,
        progress_percent: 0,
        eta_seconds: None,
    }
}
