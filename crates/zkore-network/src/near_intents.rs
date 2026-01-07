use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::http_client::{HttpClient, HttpClientError};

const DEFAULT_BASE_URL: &str = "https://1click.chaindefuser.com";

#[derive(Clone)]
pub struct NearIntentsClient {
    base_url: String,
    http: HttpClient,
}

impl NearIntentsClient {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            http: HttpClient::new()?,
        })
    }

    pub fn new_with_tor(tor: std::sync::Arc<zkore_tor::TorManager>) -> anyhow::Result<Self> {
        Ok(Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            http: HttpClient::new_with_tor(tor)?,
        })
    }

    pub fn with_base_url(base_url: impl Into<String>) -> anyhow::Result<Self> {
        Ok(Self {
            base_url: base_url.into(),
            http: HttpClient::new()?,
        })
    }

    pub fn with_base_url_and_http(
        base_url: impl Into<String>,
        http: HttpClient,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            base_url: base_url.into(),
            http,
        })
    }

    pub async fn get_quote(&self, req: QuoteRequest) -> Result<QuoteResponse, NearIntentsError> {
        let mut url = reqwest::Url::parse(&format!("{}/v0/quote", self.base_url))
            .map_err(|_| NearIntentsError::InvalidResponse("invalid base url".to_string()))?;

        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("defuse_asset_identifier_in", &req.input_asset);
            qp.append_pair("defuse_asset_identifier_out", &req.output_asset);
            qp.append_pair("exact_amount_in", &req.input_amount);
            qp.append_pair("dry", "true");
        }

        let res = self.http.get_json(url).await.map_err(map_http_error)?;

        Self::handle_rate_limit(res.status, res.retry_after)?;

        if !(200..300).contains(&res.status) {
            return Err(NearIntentsError::Http {
                status: res.status,
                message: res.body.to_string(),
            });
        }

        parse_quote_response(&res.body)
    }

    pub async fn submit_deposit(
        &self,
        req: DepositSubmitRequest,
    ) -> Result<DepositSubmitResponse, NearIntentsError> {
        let url = reqwest::Url::parse(&format!("{}/v0/deposit/submit", self.base_url))
            .map_err(|_| NearIntentsError::InvalidResponse("invalid base url".to_string()))?;

        let res = self
            .http
            .post_json(url, &req)
            .await
            .map_err(map_http_error)?;

        Self::handle_rate_limit(res.status, res.retry_after)?;

        if !(200..300).contains(&res.status) {
            return Err(NearIntentsError::Http {
                status: res.status,
                message: res.body.to_string(),
            });
        }

        parse_deposit_submit_response(&res.body)
    }

    pub async fn get_status(&self, req: StatusRequest) -> Result<StatusResponse, NearIntentsError> {
        let mut url = reqwest::Url::parse(&format!("{}/v0/status", self.base_url))
            .map_err(|_| NearIntentsError::InvalidResponse("invalid base url".to_string()))?;

        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("depositAddress", &req.deposit_address);
            if let Some(m) = req.deposit_memo.as_deref() {
                qp.append_pair("depositMemo", m);
            }
        }

        let res = self.http.get_json(url).await.map_err(map_http_error)?;

        Self::handle_rate_limit(res.status, res.retry_after)?;

        if !(200..300).contains(&res.status) {
            return Err(NearIntentsError::Http {
                status: res.status,
                message: res.body.to_string(),
            });
        }

        parse_status_response(&res.body)
    }

    fn handle_rate_limit(
        status: u16,
        retry_after: Option<Duration>,
    ) -> Result<(), NearIntentsError> {
        if status != 429 {
            return Ok(());
        }

        Err(NearIntentsError::RateLimited { retry_after })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteRequest {
    pub input_asset: String,
    pub input_amount: String,
    pub output_asset: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteResponse {
    pub quote_id: String,
    pub output_amount: String,
    pub fee_amount: String,
    pub fee_asset: String,
    pub deadline_ms: i64,
    pub rate: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepositSubmitRequest {
    pub quote_id: String,
    #[serde(default)]
    pub destination_address: Option<String>,
    #[serde(default)]
    pub refund_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositSubmitResponse {
    pub remote_id: Option<String>,
    pub deposit_address: String,
    pub deposit_memo: Option<String>,
    pub deadline_ms: Option<i64>,
    pub output_amount: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRequest {
    pub deposit_address: String,
    pub deposit_memo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusResponse {
    pub status: RemoteStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteStatus {
    AwaitingDeposit,
    Pending,
    Success,
    Refunded,
    Failed,
    Unknown(String),
}

pub fn map_remote_status_to_local_state(status: &RemoteStatus) -> zkore_core::domain::SwapState {
    use zkore_core::domain::SwapState;
    match status {
        RemoteStatus::AwaitingDeposit => SwapState::AwaitingDeposit,
        RemoteStatus::Pending => SwapState::Pending,
        RemoteStatus::Success => SwapState::Confirming,
        RemoteStatus::Refunded => SwapState::Refunded,
        RemoteStatus::Failed => SwapState::Failed,
        RemoteStatus::Unknown(_) => SwapState::Pending,
    }
}

#[derive(Debug, Error)]
pub enum NearIntentsError {
    #[error("rate limited")]
    RateLimited { retry_after: Option<Duration> },
    #[error("Tor is enabled but not ready")]
    TorNotReady,
    #[error("transport error: {0}")]
    Transport(String),
    #[error("http error: status={status} {message}")]
    Http { status: u16, message: String },
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

fn map_http_error(err: HttpClientError) -> NearIntentsError {
    match err {
        HttpClientError::FailClosed(_) => NearIntentsError::TorNotReady,
        HttpClientError::DirectTransport(e) => NearIntentsError::Transport(e.to_string()),
        HttpClientError::TorTransport(message) => NearIntentsError::Transport(message),
        HttpClientError::Timeout => NearIntentsError::Transport("timeout".to_string()),
        HttpClientError::InvalidUrl(message) => NearIntentsError::InvalidResponse(message),
        HttpClientError::InvalidBody(message) => NearIntentsError::InvalidResponse(message),
    }
}

fn parse_quote_response(body: &serde_json::Value) -> Result<QuoteResponse, NearIntentsError> {
    let quote_id = body
        .get("quote_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NearIntentsError::InvalidResponse("missing quote_id".to_string()))?
        .to_string();

    let output_amount = body
        .get("output_amount")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NearIntentsError::InvalidResponse("missing output_amount".to_string()))?
        .to_string();

    let fee_amount = body
        .get("fee_amount")
        .and_then(|v| v.as_str())
        .unwrap_or("0")
        .to_string();

    let fee_asset = body
        .get("fee_asset")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let deadline_ms = body.get("deadline").and_then(|v| v.as_i64()).unwrap_or(0);

    let rate = body
        .get("rate")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    Ok(QuoteResponse {
        quote_id,
        output_amount,
        fee_amount,
        fee_asset,
        deadline_ms,
        rate,
    })
}

fn parse_deposit_submit_response(
    body: &serde_json::Value,
) -> Result<DepositSubmitResponse, NearIntentsError> {
    let deposit_address = body
        .get("deposit_address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NearIntentsError::InvalidResponse("missing deposit_address".to_string()))?
        .to_string();

    Ok(DepositSubmitResponse {
        remote_id: body
            .get("remote_id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        deposit_address,
        deposit_memo: body
            .get("deposit_memo")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        deadline_ms: body.get("deadline").and_then(|v| v.as_i64()),
        output_amount: body
            .get("output_amount")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

fn parse_status_response(body: &serde_json::Value) -> Result<StatusResponse, NearIntentsError> {
    let raw = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN");
    let status = match raw {
        "AWAITING_DEPOSIT" => RemoteStatus::AwaitingDeposit,
        "PENDING" => RemoteStatus::Pending,
        "SUCCESS" => RemoteStatus::Success,
        "REFUNDED" => RemoteStatus::Refunded,
        "FAILED" => RemoteStatus::Failed,
        other => RemoteStatus::Unknown(other.to_string()),
    };

    Ok(StatusResponse {
        status,
        message: body
            .get("message")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}
