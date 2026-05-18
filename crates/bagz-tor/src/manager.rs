use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::warn;

use bagz_core::domain::{TorState, TorStatus};
use bagz_core::ipc::v1::common::SCHEMA_VERSION;
use bagz_core::ipc::v1::events::TorStatusEvent;
use bagz_core::permissions::create_dir_all_secure_async;

type BootstrapFuture =
    Pin<Box<dyn Future<Output = Result<zcash_client_backend::tor::Client, String>> + Send>>;
type BootstrapFn = Arc<dyn Fn(PathBuf) -> BootstrapFuture + Send + Sync>;

#[derive(Clone)]
pub struct TorManagerConfig {
    pub tor_dir: PathBuf,
    pub bootstrap_timeout: Duration,
    pub bootstrap: BootstrapFn,
}

impl TorManagerConfig {
    pub fn new(tor_dir: PathBuf) -> Self {
        let bootstrap: BootstrapFn = Arc::new(|tor_dir| {
            Box::pin(async move {
                zcash_client_backend::tor::Client::create(&tor_dir, |_| {})
                    .await
                    .map_err(|e| e.to_string())
            })
        });

        Self {
            tor_dir,
            bootstrap_timeout: Duration::from_secs(60),
            bootstrap,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TorManagerError {
    #[error("tokio runtime is not available")]
    MissingTokioRuntime,
}

struct Inner {
    state: TorState,
    tor_client: Option<zcash_client_backend::tor::Client>,
    cancel_bootstrap: Option<watch::Sender<bool>>,
    bootstrap_task: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct TorManager {
    config: TorManagerConfig,
    inner: Arc<Mutex<Inner>>,
    events_tx: watch::Sender<TorStatusEvent>,
}

impl TorManager {
    pub fn new(config: TorManagerConfig, mut initial_state: TorState) -> Self {
        // If Tor is disabled, `Off` is the only sensible runtime status.
        if !initial_state.enabled {
            initial_state.status = TorStatus::Off;
            initial_state.last_error = None;
        }

        // A Tor client cannot be restored from persisted state. If we start in `On`,
        // callers may temporarily observe an inconsistent "On but no client" state
        // until bootstrap is (re-)started.
        if initial_state.status == TorStatus::On {
            initial_state.status = TorStatus::Off;
            initial_state.last_error = None;
        }

        let initial_event = TorStatusEvent {
            schema_version: SCHEMA_VERSION,
            event: "tor.status".to_string(),
            state: initial_state.clone(),
        };
        let (events_tx, _events_rx) = watch::channel(initial_event);

        Self {
            config,
            inner: Arc::new(Mutex::new(Inner {
                state: initial_state,
                tor_client: None,
                cancel_bootstrap: None,
                bootstrap_task: None,
            })),
            events_tx,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<TorStatusEvent> {
        self.events_tx.subscribe()
    }

    pub fn state(&self) -> TorState {
        self.inner.lock().expect("mutex poisoned").state.clone()
    }

    pub fn tor_client(&self) -> Option<zcash_client_backend::tor::Client> {
        let inner = self.inner.lock().expect("mutex poisoned");
        if inner.state.enabled && inner.state.status == TorStatus::On {
            inner.tor_client.clone()
        } else {
            None
        }
    }

    pub fn start_if_enabled(&self) -> Result<(), TorManagerError> {
        if self.state().enabled {
            let _ = self.set_enabled(true)?;
        }
        Ok(())
    }

    /// Enables or disables Tor, updating [`TorState`] and emitting a status event.
    ///
    /// - `set_enabled(false)`: Immediately sets status to `Off`, cancels any in-progress
    ///   bootstrap, and clears the client.
    /// - `set_enabled(true)` when already `On` with a client: No-op (returns current state).
    /// - `set_enabled(true)` otherwise (including `Error` or `Connecting` states): Cancels
    ///   any in-progress bootstrap and starts a fresh one.
    ///
    /// To force a restart when already `On`, toggle `set_enabled(false)` then `set_enabled(true)`.
    pub fn set_enabled(&self, enabled: bool) -> Result<TorState, TorManagerError> {
        tokio::runtime::Handle::try_current().map_err(|_| TorManagerError::MissingTokioRuntime)?;

        let (cancel_rx, should_spawn) = {
            let mut inner = self.inner.lock().expect("mutex poisoned");
            inner.state.enabled = enabled;

            if let Some(tx) = inner.cancel_bootstrap.take() {
                let _ = tx.send(true);
            }
            if let Some(handle) = inner.bootstrap_task.take() {
                handle.abort();
            }

            if !enabled {
                inner.state.status = TorStatus::Off;
                inner.state.last_error = None;
                inner.tor_client = None;
                self.publish_state_locked(&inner);
                return Ok(inner.state.clone());
            }

            if inner.state.status == TorStatus::On && inner.tor_client.is_some() {
                self.publish_state_locked(&inner);
                return Ok(inner.state.clone());
            }

            inner.state.status = TorStatus::Connecting;
            inner.state.last_error = None;
            inner.tor_client = None;
            self.publish_state_locked(&inner);

            let (cancel_tx, cancel_rx) = watch::channel(false);
            inner.cancel_bootstrap = Some(cancel_tx);
            (cancel_rx, true)
        };

        if should_spawn {
            self.spawn_bootstrap(cancel_rx);
        }

        Ok(self.state())
    }

    fn publish_state_locked(&self, inner: &Inner) {
        let _ = self.events_tx.send(TorStatusEvent {
            schema_version: SCHEMA_VERSION,
            event: "tor.status".to_string(),
            state: inner.state.clone(),
        });
    }

    fn set_status_if_active(
        &self,
        cancel_rx: &watch::Receiver<bool>,
        status: TorStatus,
        last_error: Option<String>,
        client: Option<zcash_client_backend::tor::Client>,
    ) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        // `cancel_rx` prevents stale bootstrap tasks (from prior enable/restart cycles)
        // from publishing state, while `enabled` ensures we never publish while Tor is
        // disabled even if cancellation races.
        if *cancel_rx.borrow() || !inner.state.enabled {
            return;
        }
        inner.state.status = status;
        inner.state.last_error = last_error;
        inner.tor_client = client;
        self.publish_state_locked(&inner);
    }

    fn spawn_bootstrap(&self, mut cancel_rx: watch::Receiver<bool>) {
        let config = self.config.clone();
        let this = self.clone();

        let handle = tokio::spawn(async move {
            if *cancel_rx.borrow() {
                return;
            }

            if let Err(err) = create_dir_all_secure_async(&config.tor_dir).await {
                if *cancel_rx.borrow() {
                    return;
                }
                warn!("failed to create tor directory: {err}");
                // No explicit cancellation check needed: set_status_if_active has an internal
                // guard, and we return immediately after.
                this.set_status_if_active(
                    &cancel_rx,
                    TorStatus::Error,
                    Some(format!("failed to create tor directory: {err}")),
                    None,
                );
                return;
            }

            // If Tor was disabled (or restart requested) while creating the directory,
            // exit before starting bootstrap work.
            if *cancel_rx.borrow() {
                return;
            }

            let client = match tokio::select! {
                _ = cancel_rx.changed() => return,
                res = timeout(config.bootstrap_timeout, (config.bootstrap)(config.tor_dir.clone())) => res,
            } {
                Err(_) => {
                    this.set_status_if_active(
                        &cancel_rx,
                        TorStatus::Error,
                        Some("Tor bootstrap timed out".to_string()),
                        None,
                    );
                    return;
                }
                Ok(Err(err)) => {
                    this.set_status_if_active(&cancel_rx, TorStatus::Error, Some(err), None);
                    return;
                }
                Ok(Ok(client)) => client,
            };

            // Bootstrap success indicates Tor is ready - first real network call
            // (to lightwalletd) will validate actual connectivity
            this.set_status_if_active(&cancel_rx, TorStatus::On, None, Some(client));
        });

        let mut inner = self.inner.lock().expect("mutex poisoned");
        // Cancellation is the primary shutdown mechanism, but keeping a handle lets us
        // best-effort abort on fast toggles.
        inner.bootstrap_task = Some(handle);
    }

    pub fn tor_dir(&self) -> &Path {
        &self.config.tor_dir
    }
}
