use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use tokio::sync::RwLock;
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
///
/// Connection pooling: For direct connections, the gRPC channel is cached and reused
/// across all RPC calls. HTTP/2 keep-alive is enabled to maintain connection health.
/// For Tor connections, the channel is created fresh each time as Tor circuits may change.
///
/// Connection resilience: The cached channel can be reset via `reset_connection()` if
/// it becomes stale. The `get_client_with_retry()` method automatically handles
/// reconnection on connection failures.
#[derive(Clone)]
pub struct GrpcClient {
    endpoint: String,
    transport: TransportSelector,
    /// Cached channel for direct (non-Tor) connections.
    /// Uses Arc<RwLock<Option>> for thread-safe lazy initialization with reset capability.
    direct_channel: Arc<RwLock<Option<Channel>>>,
}

impl GrpcClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport: TransportSelector::new(TransportConfig::default()),
            direct_channel: Arc::new(RwLock::new(None)),
        }
    }

    pub fn new_with_tor(endpoint: impl Into<String>, tor: Arc<zkore_tor::TorManager>) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport: TransportSelector::with_tor(TransportConfig::default(), tor),
            direct_channel: Arc::new(RwLock::new(None)),
        }
    }

    pub fn new_with_transport(endpoint: impl Into<String>, transport: TransportSelector) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport,
            direct_channel: Arc::new(RwLock::new(None)),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Get or create a cached channel for direct (non-Tor) connections.
    ///
    /// The channel is lazily initialized on first use and reused for all subsequent
    /// RPC calls. HTTP/2 keep-alive is enabled to maintain connection health.
    async fn get_direct_channel(&self) -> anyhow::Result<Channel> {
        // First, try to get an existing channel with a read lock
        {
            let guard = self.direct_channel.read().await;
            if let Some(channel) = guard.as_ref() {
                return Ok(channel.clone());
            }
        }

        // No cached channel, acquire write lock to create one
        let mut guard = self.direct_channel.write().await;

        // Double-check: another task may have created the channel while we waited for the write lock
        if let Some(channel) = guard.as_ref() {
            return Ok(channel.clone());
        }

        tracing::debug!(endpoint = %self.endpoint, "grpc creating new direct channel with connection pooling");

        let endpoint = Endpoint::new(self.endpoint.clone())
            .context("invalid gRPC endpoint URL")?
            .timeout(Duration::from_secs(120)) // 2 minutes for streaming operations
            .connect_timeout(Duration::from_secs(30)) // 30 seconds for connection
            // HTTP/2 keep-alive settings for connection pooling
            .http2_keep_alive_interval(Duration::from_secs(10))
            .keep_alive_timeout(Duration::from_secs(20))
            .keep_alive_while_idle(true);

        match endpoint.connect().await {
            Ok(channel) => {
                tracing::debug!(endpoint = %self.endpoint, "grpc direct channel established");
                *guard = Some(channel.clone());
                Ok(channel)
            }
            Err(err) => {
                tracing::warn!(
                    endpoint = %self.endpoint,
                    error = ?err,
                    "grpc direct connect failed"
                );
                Err(anyhow::Error::new(err)).context("failed to connect")
            }
        }
    }

    /// Clear cached channel to force reconnection on next use.
    ///
    /// This is useful when the cached connection becomes stale or the server
    /// closes it. The next call to `get_client()` or `get_direct_channel()`
    /// will establish a fresh connection.
    pub async fn reset_connection(&self) {
        let mut guard = self.direct_channel.write().await;
        if guard.is_some() {
            tracing::debug!(endpoint = %self.endpoint, "grpc resetting cached channel");
            *guard = None;
        }
    }

    /// Check if an error indicates a connection failure that warrants a retry.
    ///
    /// Returns true for transport-level errors that suggest the connection
    /// is broken and should be re-established.
    fn is_connection_error(err: &anyhow::Error) -> bool {
        let err_str = format!("{err:?}").to_lowercase();

        // Check for common connection failure patterns
        err_str.contains("connection")
            || err_str.contains("transport")
            || err_str.contains("broken pipe")
            || err_str.contains("reset by peer")
            || err_str.contains("timed out")
            || err_str.contains("refused")
            || err_str.contains("hyper")
            || err_str.contains("h2")
    }

    /// Get a gRPC client using connection pooling.
    ///
    /// For direct connections, reuses the cached channel.
    /// For Tor connections, creates a fresh connection (Tor circuits may change).
    pub async fn get_client(&self) -> anyhow::Result<CompactTxStreamerClient<Channel>> {
        tracing::debug!(endpoint = %self.endpoint, "grpc get_client");

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
                tracing::debug!(endpoint = %self.endpoint, "grpc selected direct transport (pooled)");
                let channel = self.get_direct_channel().await?;
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

    /// Get a gRPC client with automatic retry on connection failure.
    ///
    /// If the first attempt fails with a connection error, the cached channel
    /// is reset and a fresh connection is attempted. This handles stale
    /// connections that may have been closed by the server.
    ///
    /// For most use cases, prefer `get_client()` which is sufficient when
    /// connections are healthy. Use this method for operations that may
    /// encounter stale connections after periods of inactivity.
    pub async fn get_client_with_retry(&self) -> anyhow::Result<CompactTxStreamerClient<Channel>> {
        match self.get_client().await {
            Ok(client) => Ok(client),
            Err(err) if Self::is_connection_error(&err) => {
                tracing::info!(
                    endpoint = %self.endpoint,
                    error = ?err,
                    "grpc connection failed, resetting and retrying"
                );
                self.reset_connection().await;
                self.get_client().await
            }
            Err(err) => Err(err),
        }
    }

    /// Legacy connect method - prefer using `get_client()` for connection pooling.
    #[deprecated(note = "use get_client() for connection pooling")]
    pub async fn connect(&self) -> anyhow::Result<CompactTxStreamerClient<Channel>> {
        self.get_client().await
    }

    pub async fn probe_mempool_support(&self) -> anyhow::Result<()> {
        let mut client = self.get_client().await.context("failed to connect")?;

        let mut req = tonic::Request::new(Empty {});
        req.set_timeout(Duration::from_secs(10));

        match client.get_mempool_stream(req).await {
            Ok(_stream) => Ok(()),
            Err(status) if status.code() == Code::Unimplemented => Err(anyhow::anyhow!(
                "server missing GetMempoolStream capability"
            )),
            Err(status) if status.code() == Code::DeadlineExceeded => Ok(()),
            Err(status) if status.code() == Code::Cancelled => Ok(()),
            Err(status) => Err(anyhow::anyhow!(status)).context("mempool probe failed"),
        }
    }

    pub async fn get_lightd_info(&self) -> anyhow::Result<LightdInfo> {
        let mut client = self.get_client().await.context("failed to connect")?;

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
        let mut client = self.get_client().await.context("failed to connect")?;

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
        let mut client = self.get_client().await.context("failed to connect")?;

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
        let mut client = self.get_client().await.context("failed to connect")?;

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
        let mut client = self.get_client().await.context("failed to connect")?;

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
        let mut client = self.get_client().await.context("failed to connect")?;

        let request = GetSubtreeRootsArg {
            start_index,
            shielded_protocol,
            max_entries,
        };

        Ok(client.get_subtree_roots(request).await?.into_inner())
    }
}
