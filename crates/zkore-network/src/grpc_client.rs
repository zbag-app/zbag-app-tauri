use std::time::Duration;

use anyhow::Context as _;
use tonic::Code;
use tonic::transport::{Channel, Endpoint};

use zcash_client_backend::proto::service::Empty;
use zcash_client_backend::proto::service::RawTransaction;
use zcash_client_backend::proto::service::compact_tx_streamer_client::CompactTxStreamerClient;

/// CompactTxStreamer gRPC client wrapper.
///
/// Note: In v1 we require mempool support (`GetMempoolStream`) for pending tx detection.
#[derive(Debug, Clone)]
pub struct GrpcClient {
    endpoint: String,
}

impl GrpcClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn connect(&self) -> anyhow::Result<CompactTxStreamerClient<Channel>> {
        let endpoint = Endpoint::from_shared(self.endpoint.clone())
            .context("invalid gRPC endpoint URL")?
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(10));

        let channel = endpoint.connect().await.context("failed to connect")?;
        Ok(CompactTxStreamerClient::new(channel))
    }

    pub async fn probe_mempool_support(&self) -> anyhow::Result<()> {
        let mut client = self.connect().await.context("failed to connect")?;

        let mut req = tonic::Request::new(Empty {});
        req.set_timeout(Duration::from_secs(2));

        match client.get_mempool_stream(req).await {
            Ok(_stream) => Ok(()),
            Err(status) if status.code() == Code::Unimplemented => Err(anyhow::anyhow!(
                "server missing GetMempoolStream capability"
            )),
            Err(status) if status.code() == Code::DeadlineExceeded => Ok(()),
            Err(status) => Err(anyhow::anyhow!(status)).context("mempool probe failed"),
        }
    }

    pub async fn send_transaction(&self, tx_bytes: Vec<u8>) -> anyhow::Result<()> {
        let mut client = self.connect().await.context("failed to connect")?;

        let mut req = tonic::Request::new(RawTransaction {
            data: tx_bytes,
            height: 0,
        });
        req.set_timeout(Duration::from_secs(10));

        let response = client
            .send_transaction(req)
            .await
            .map_err(|status| anyhow::anyhow!(status))
            .context("SendTransaction RPC failed")?
            .into_inner();

        if response.error_code != 0 {
            return Err(anyhow::anyhow!(
                "broadcast rejected (code {}): {}",
                response.error_code,
                response.error_message
            ));
        }

        Ok(())
    }
}
