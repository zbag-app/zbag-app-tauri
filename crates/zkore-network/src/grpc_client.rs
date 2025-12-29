use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use tonic::Code;
use tonic::transport::{Channel, Endpoint};

use zcash_client_backend::proto::compact_formats::CompactBlock;
use zcash_client_backend::proto::service::{
    BlockId, BlockRange, ChainSpec, Empty, GetSubtreeRootsArg, LightdInfo, RawTransaction,
    SubtreeRoot, TreeState, compact_tx_streamer_client::CompactTxStreamerClient,
};
use zcash_protocol::consensus::BlockHeight;

use tonic::Streaming;

use crate::transport::{SelectedTransport, TransportConfig, TransportSelector};

/// CompactTxStreamer gRPC client wrapper.
///
/// Note: In v1 we require mempool support (`GetMempoolStream`) for pending tx detection.
#[derive(Clone)]
pub struct GrpcClient {
    endpoint: String,
    transport: TransportSelector,
}

impl GrpcClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport: TransportSelector::new(TransportConfig::default()),
        }
    }

    pub fn new_with_tor(endpoint: impl Into<String>, tor: Arc<zkore_tor::TorManager>) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport: TransportSelector::with_tor(TransportConfig::default(), tor),
        }
    }

    pub fn new_with_transport(endpoint: impl Into<String>, transport: TransportSelector) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn connect(&self) -> anyhow::Result<CompactTxStreamerClient<Channel>> {
        tracing::debug!(endpoint = %self.endpoint, "grpc connect");

        let selected = match self.transport.select() {
            Ok(selected) => selected,
            Err(err) => {
                tracing::warn!(
                    endpoint = %self.endpoint,
                    error = ?err,
                    "grpc transport selection failed"
                );
                return Err(anyhow::Error::new(err));
            }
        };

        match selected {
            SelectedTransport::Direct => {
                tracing::debug!(endpoint = %self.endpoint, "grpc selected direct transport");

                let endpoint = Endpoint::new(self.endpoint.clone())
                    .context("invalid gRPC endpoint URL")?
                    .timeout(Duration::from_secs(120)) // 2 minutes for streaming operations
                    .connect_timeout(Duration::from_secs(30)); // 30 seconds for connection

                let channel = match endpoint.connect().await {
                    Ok(channel) => channel,
                    Err(err) => {
                        tracing::warn!(
                            endpoint = %self.endpoint,
                            error = ?err,
                            "grpc direct connect failed"
                        );
                        return Err(anyhow::Error::new(err)).context("failed to connect");
                    }
                };
                Ok(CompactTxStreamerClient::new(channel))
            }
            SelectedTransport::Tor { client } => {
                tracing::debug!(endpoint = %self.endpoint, "grpc selected Tor transport");

                let uri = self.endpoint.parse().context("invalid gRPC endpoint URL")?;

                let conn = tokio::time::timeout(
                    self.transport.config().timeout,
                    client.connect_to_lightwalletd(uri),
                )
                .await
                .context("Tor gRPC connection timed out")?
                .map_err(anyhow::Error::new)
                .context("failed to connect through Tor")?;

                Ok(conn)
            }
        }
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

    pub async fn get_lightd_info(&self) -> anyhow::Result<LightdInfo> {
        let mut client = self.connect().await.context("failed to connect")?;

        let mut req = tonic::Request::new(Empty {});
        req.set_timeout(Duration::from_secs(3));

        let info = client
            .get_lightd_info(req)
            .await
            .map_err(|status| anyhow::anyhow!(status))
            .context("GetLightdInfo RPC failed")?
            .into_inner();

        Ok(info)
    }

    pub async fn probe_server(&self) -> anyhow::Result<LightdInfo> {
        let info = self.get_lightd_info().await?;
        self.probe_mempool_support().await?;
        Ok(info)
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

    /// Get the chain tip height and hash.
    pub async fn get_latest_block(&self) -> anyhow::Result<(BlockHeight, Vec<u8>)> {
        let mut client = self.connect().await.context("failed to connect")?;

        let mut req = tonic::Request::new(ChainSpec {});
        req.set_timeout(Duration::from_secs(5));

        let response = client
            .get_latest_block(req)
            .await
            .map_err(|status| anyhow::anyhow!(status))
            .context("GetLatestBlock RPC failed")?
            .into_inner();

        Ok((BlockHeight::from_u32(response.height as u32), response.hash))
    }

    /// Download compact blocks in a range.
    pub async fn get_block_range(
        &self,
        start: BlockHeight,
        end: BlockHeight,
    ) -> anyhow::Result<Streaming<CompactBlock>> {
        let mut client = self.connect().await.context("failed to connect")?;

        let request = BlockRange {
            start: Some(BlockId {
                height: u64::from(start),
                hash: vec![],
            }),
            end: Some(BlockId {
                height: u64::from(end),
                hash: vec![],
            }),
        };

        Ok(client.get_block_range(request).await?.into_inner())
    }

    /// Get tree state at a height.
    pub async fn get_tree_state(&self, height: BlockHeight) -> anyhow::Result<TreeState> {
        let mut client = self.connect().await.context("failed to connect")?;

        let mut req = tonic::Request::new(BlockId {
            height: u64::from(height),
            hash: vec![],
        });
        req.set_timeout(Duration::from_secs(10));

        Ok(client
            .get_tree_state(req)
            .await
            .map_err(|status| anyhow::anyhow!(status))
            .context("GetTreeState RPC failed")?
            .into_inner())
    }

    /// Get subtree roots for commitment trees.
    pub async fn get_subtree_roots(
        &self,
        start_index: u32,
        shielded_protocol: i32,
        max_entries: u32,
    ) -> anyhow::Result<Streaming<SubtreeRoot>> {
        let mut client = self.connect().await.context("failed to connect")?;

        let request = GetSubtreeRootsArg {
            start_index,
            shielded_protocol,
            max_entries,
        };

        Ok(client.get_subtree_roots(request).await?.into_inner())
    }
}
