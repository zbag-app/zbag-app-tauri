use std::time::Duration;

use anyhow::Context as _;

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

pub trait Transport: Send + Sync {
    fn http_client(&self) -> anyhow::Result<reqwest::Client>;
}

#[derive(Debug, Clone)]
pub struct DirectTransport {
    pub config: TransportConfig,
}

impl Transport for DirectTransport {
    fn http_client(&self) -> anyhow::Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(self.config.timeout)
            .build()
            .context("failed to build reqwest client")
    }
}
