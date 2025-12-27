use anyhow::Context as _;

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

    pub async fn connect(&self) -> anyhow::Result<()> {
        // TODO(T048): Build tonic channel and service client bindings for CompactTxStreamer.
        Err(anyhow::anyhow!(
            "grpc client not implemented (endpoint={})",
            self.endpoint
        ))
    }

    pub async fn probe_mempool_support(&self) -> anyhow::Result<()> {
        // TODO(T048a): Call CompactTxStreamer.GetMempoolStream and fail if UNIMPLEMENTED.
        self.connect().await.context("failed to connect")?;
        Ok(())
    }
}

