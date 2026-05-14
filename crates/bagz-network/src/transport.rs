use std::sync::Arc;
use std::time::Duration;

use zstash_core::domain::TorStatus;

use zstash_tor::TorManager;

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub timeout: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Clone)]
pub struct TransportSelector {
    config: TransportConfig,
    tor: Option<Arc<TorManager>>,
}

impl TransportSelector {
    pub fn new(config: TransportConfig) -> Self {
        Self { config, tor: None }
    }

    pub fn with_tor(config: TransportConfig, tor: Arc<TorManager>) -> Self {
        Self {
            config,
            tor: Some(tor),
        }
    }

    pub fn config(&self) -> &TransportConfig {
        &self.config
    }

    pub fn select(&self) -> Result<SelectedTransport, TransportError> {
        let Some(tor) = self.tor.as_ref() else {
            return Ok(SelectedTransport::Direct);
        };

        let state = tor.state();
        if !state.enabled {
            return Ok(SelectedTransport::Direct);
        }

        if state.status != TorStatus::On {
            return Err(TransportError::TorNotReady {
                status: state.status,
                last_error: state.last_error,
            });
        }

        let Some(client) = tor.tor_client() else {
            return Err(TransportError::TorNotReady {
                status: state.status,
                last_error: state.last_error,
            });
        };

        Ok(SelectedTransport::Tor {
            client: Box::new(client),
        })
    }
}

#[derive(Clone)]
pub enum SelectedTransport {
    Direct,
    Tor {
        client: Box<zcash_client_backend::tor::Client>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("tor enabled but not ready (status={status:?})")]
    TorNotReady {
        status: TorStatus,
        last_error: Option<String>,
    },
}
