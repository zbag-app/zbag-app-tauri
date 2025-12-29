use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use http::Uri;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::warn;

use zkore_core::domain::{TorState, TorStatus};
use zkore_core::ipc::v1::common::SCHEMA_VERSION;
use zkore_core::ipc::v1::events::TorStatusEvent;

type BootstrapFuture =
    Pin<Box<dyn Future<Output = Result<zcash_client_backend::tor::Client, String>> + Send>>;
type BootstrapFn = Arc<dyn Fn(PathBuf) -> BootstrapFuture + Send + Sync>;

type HealthcheckFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
type HealthcheckFn =
    Arc<dyn Fn(zcash_client_backend::tor::Client) -> HealthcheckFuture + Send + Sync>;

#[derive(Clone)]
pub struct TorManagerConfig {
    pub tor_dir: PathBuf,
    pub bootstrap_timeout: Duration,
    pub healthcheck_timeout: Duration,
    pub healthcheck_url: Uri,
    pub bootstrap: BootstrapFn,
    pub health_check: HealthcheckFn,
}

impl TorManagerConfig {
    pub fn new(tor_dir: PathBuf) -> Self {
        let healthcheck_url: Uri = "https://example.com/"
            .parse()
            .expect("static healthcheck URL must be valid");
        let healthcheck_url_for_closure = healthcheck_url.clone();

        let bootstrap: BootstrapFn = Arc::new(|tor_dir| {
            Box::pin(async move {
                zcash_client_backend::tor::Client::create(&tor_dir, |_| {})
                    .await
                    .map_err(|e| e.to_string())
            })
        });

        let health_check: HealthcheckFn = Arc::new(move |client| {
            let url = healthcheck_url_for_closure.clone();
            Box::pin(async move {
                client
                    .http_get(
                        url,
                        |builder| builder,
                        |_body| async { Ok(()) },
                        0,
                        |_| None,
                    )
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })
        });

        Self {
            tor_dir,
            bootstrap_timeout: Duration::from_secs(60),
            healthcheck_timeout: Duration::from_secs(15),
            healthcheck_url,
            bootstrap,
            health_check,
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
    pub fn new(config: TorManagerConfig, initial_state: TorState) -> Self {
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

    fn set_state(&self, state: TorState, client: Option<zcash_client_backend::tor::Client>) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        inner.state = state;
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

            if let Err(err) = tokio::fs::create_dir_all(&config.tor_dir).await {
                if *cancel_rx.borrow() {
                    return;
                }
                warn!("failed to create tor directory: {err}");
                this.set_state(
                    TorState {
                        enabled: true,
                        status: TorStatus::Error,
                        last_error: Some(format!("failed to create tor directory: {err}")),
                    },
                    None,
                );
                return;
            }

            let client = match tokio::select! {
                _ = cancel_rx.changed() => return,
                res = timeout(config.bootstrap_timeout, (config.bootstrap)(config.tor_dir.clone())) => res,
            } {
                Err(_) => {
                    this.set_state(
                        TorState {
                            enabled: true,
                            status: TorStatus::Error,
                            last_error: Some("Tor bootstrap timed out".to_string()),
                        },
                        None,
                    );
                    return;
                }
                Ok(Err(err)) => {
                    this.set_state(
                        TorState {
                            enabled: true,
                            status: TorStatus::Error,
                            last_error: Some(err),
                        },
                        None,
                    );
                    return;
                }
                Ok(Ok(client)) => client,
            };

            let health = tokio::select! {
                _ = cancel_rx.changed() => return,
                res = timeout(config.healthcheck_timeout, (config.health_check)(client.clone())) => res,
            };

            match health {
                Err(_) => {
                    this.set_state(
                        TorState {
                            enabled: true,
                            status: TorStatus::Error,
                            last_error: Some("Tor health check timed out".to_string()),
                        },
                        None,
                    );
                }
                Ok(Err(err)) => {
                    this.set_state(
                        TorState {
                            enabled: true,
                            status: TorStatus::Error,
                            last_error: Some(err),
                        },
                        None,
                    );
                }
                Ok(Ok(())) => {
                    this.set_state(
                        TorState {
                            enabled: true,
                            status: TorStatus::On,
                            last_error: None,
                        },
                        Some(client),
                    );
                }
            }
        });

        let mut inner = self.inner.lock().expect("mutex poisoned");
        inner.bootstrap_task = Some(handle);
    }

    pub fn tor_dir(&self) -> &Path {
        &self.config.tor_dir
    }
}
