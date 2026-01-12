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

    pub fn new_with_tor(tor: std::sync::Arc<zstash_tor::TorManager>) -> anyhow::Result<Self> {
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

    /// Get a quote from the 1Click API.
    ///
    /// Uses POST with JSON body (new API format).
    /// Set `dry=true` for quote preview, `dry=false` to get deposit address.
    pub async fn get_quote(
        &self,
        mut req: QuoteRequest,
    ) -> Result<QuoteResponse, NearIntentsError> {
        let url = reqwest::Url::parse(&format!("{}/v0/quote", self.base_url))
            .map_err(|_| NearIntentsError::InvalidResponse("invalid base url".to_string()))?;

        let res = self
            .http
            .post_json(url, &req)
            .await
            .map_err(map_http_error)?;

        Self::handle_rate_limit(res.status, res.retry_after)?;

        if !(200..300).contains(&res.status) {
            // The 1Click API sometimes returns a 400 "Failed to get quote" when the quote
            // cannot be produced within the requested `quoteWaitingTimeMs`. Retry once
            // with a more forgiving waiting time to reduce flakiness for otherwise-valid
            // requests.
            if res.status == 400
                && res
                    .body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .is_some_and(|m| m == "Failed to get quote")
                && req.quote_waiting_time_ms.unwrap_or(0) < 10_000
            {
                req.quote_waiting_time_ms = Some(10_000);
                let url =
                    reqwest::Url::parse(&format!("{}/v0/quote", self.base_url)).map_err(|_| {
                        NearIntentsError::InvalidResponse("invalid base url".to_string())
                    })?;

                let res = self
                    .http
                    .post_json(url, &req)
                    .await
                    .map_err(map_http_error)?;

                Self::handle_rate_limit(res.status, res.retry_after)?;

                if (200..300).contains(&res.status) {
                    return parse_quote_response(&res.body);
                }

                return Err(NearIntentsError::Http {
                    status: res.status,
                    message: res.body.to_string(),
                });
            }

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

/// Quote request for the new 1Click API (POST /v0/quote).
///
/// Reference: <https://docs.near-intents.org/near-intents/integration/distribution-channels/1click-api>
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    pub origin_asset: String,
    pub destination_asset: String,
    /// Amount in smallest units (e.g., wei for ETH, zatoshis for ZEC)
    pub amount: String,
    pub swap_type: String,
    pub slippage_tolerance: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_waiting_time_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_fees: Option<Vec<AppFee>>,
    pub deposit_type: String,
    pub refund_to: String,
    pub refund_type: String,
    pub recipient: String,
    pub recipient_type: String,
    /// ISO 8601 timestamp (e.g., "2026-01-09T12:00:00Z")
    pub deadline: String,
    pub dry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppFee {
    pub recipient: String,
    /// Basis points (1/100th of a percent, e.g., 100 = 1%)
    pub fee: u32,
}

/// Quote response from the new 1Click API.
///
/// The API returns a nested structure with `quote`, `quoteRequest`, `signature`, etc.
/// We extract the relevant fields from `quote` and `correlationId` from the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteResponse {
    pub amount_in: String,
    pub amount_in_formatted: String,
    pub amount_in_usd: String,
    pub min_amount_in: String,
    pub amount_out: String,
    pub amount_out_formatted: String,
    pub amount_out_usd: String,
    pub min_amount_out: String,
    /// ISO 8601 timestamp, present only when dry=false
    pub deadline_iso: Option<String>,
    /// Milliseconds since epoch (parsed from deadline_iso for UI countdown)
    pub deadline_ms: Option<i64>,
    /// ISO 8601 timestamp, present only when dry=false
    pub time_when_inactive_iso: Option<String>,
    pub time_estimate_secs: u64,
    /// Deposit address is inside `quote`, present only when dry=false
    pub deposit_address: Option<String>,
    pub deposit_memo: Option<String>,
    pub correlation_id: String,
}

/// Deposit submit request for FromZec flow (after broadcasting ZEC tx).
///
/// The new API expects txHash + depositAddress instead of quote_id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositSubmitRequest {
    pub tx_hash: String,
    pub deposit_address: String,
}

/// Deposit submit response (acknowledgement only - no new data expected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositSubmitResponse {
    pub success: bool,
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

/// Remote status values from the 1Click API.
///
/// Reference: <https://docs.near-intents.org/near-intents/integration/distribution-channels/1click-api>
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteStatus {
    /// Waiting for user to send funds to deposit address
    PendingDeposit,
    /// Deposit tx detected but not yet confirmed
    KnownDepositTx,
    /// Partial deposit received (wrong amount)
    IncompleteDeposit,
    /// Swap is being processed
    Processing,
    /// Swap completed successfully
    Success,
    /// Funds were refunded
    Refunded,
    /// Swap failed
    Failed,
    /// Unknown status value
    Unknown(String),
}

pub fn map_remote_status_to_local_state(status: &RemoteStatus) -> zstash_core::domain::SwapState {
    use zstash_core::domain::SwapState;
    match status {
        RemoteStatus::PendingDeposit => SwapState::AwaitingDeposit,
        RemoteStatus::KnownDepositTx => SwapState::Pending,
        RemoteStatus::IncompleteDeposit => SwapState::Failed,
        RemoteStatus::Processing => SwapState::Pending,
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

/// Parse the nested quote response from the 1Click API.
///
/// Response structure:
/// ```json
/// {
///   "quote": {
///     "amountIn": "...",
///     "amountInFormatted": "...",
///     "amountOut": "...",
///     "depositAddress": "...",  // only when dry=false
///     "deadline": "2026-01-09T12:00:00Z",  // ISO 8601
///     ...
///   },
///   "correlationId": "uuid"
/// }
/// ```
fn parse_quote_response(body: &serde_json::Value) -> Result<QuoteResponse, NearIntentsError> {
    let quote = body
        .get("quote")
        .ok_or_else(|| NearIntentsError::InvalidResponse("missing quote object".to_string()))?;

    let correlation_id = body
        .get("correlationId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NearIntentsError::InvalidResponse("missing correlationId".to_string()))?
        .to_string();

    let amount_in = quote
        .get("amountIn")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NearIntentsError::InvalidResponse("missing quote.amountIn".to_string()))?
        .to_string();

    let amount_in_formatted = quote
        .get("amountInFormatted")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            NearIntentsError::InvalidResponse("missing quote.amountInFormatted".to_string())
        })?
        .to_string();

    let amount_in_usd = quote
        .get("amountInUsd")
        .and_then(|v| v.as_str())
        .unwrap_or("0")
        .to_string();

    let min_amount_in = quote
        .get("minAmountIn")
        .and_then(|v| v.as_str())
        .unwrap_or(&amount_in)
        .to_string();

    let amount_out = quote
        .get("amountOut")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NearIntentsError::InvalidResponse("missing quote.amountOut".to_string()))?
        .to_string();

    let amount_out_formatted = quote
        .get("amountOutFormatted")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            NearIntentsError::InvalidResponse("missing quote.amountOutFormatted".to_string())
        })?
        .to_string();

    let amount_out_usd = quote
        .get("amountOutUsd")
        .and_then(|v| v.as_str())
        .unwrap_or("0")
        .to_string();

    let min_amount_out = quote
        .get("minAmountOut")
        .and_then(|v| v.as_str())
        .unwrap_or(&amount_out)
        .to_string();

    let time_estimate_secs = quote
        .get("timeEstimate")
        .and_then(|v| v.as_u64())
        .unwrap_or(120);

    // Parse ISO 8601 deadline to milliseconds
    let deadline_iso = quote
        .get("deadline")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let deadline_ms = deadline_iso.as_ref().and_then(|iso| {
        chrono::DateTime::parse_from_rfc3339(iso)
            .ok()
            .map(|dt| dt.timestamp_millis())
    });

    let time_when_inactive_iso = quote
        .get("timeWhenInactive")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // depositAddress is INSIDE the quote object (only present when dry=false)
    let deposit_address = quote
        .get("depositAddress")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let deposit_memo = quote
        .get("depositMemo")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(QuoteResponse {
        amount_in,
        amount_in_formatted,
        amount_in_usd,
        min_amount_in,
        amount_out,
        amount_out_formatted,
        amount_out_usd,
        min_amount_out,
        deadline_iso,
        deadline_ms,
        time_when_inactive_iso,
        time_estimate_secs,
        deposit_address,
        deposit_memo,
        correlation_id,
    })
}

/// Parse deposit submit response (simple acknowledgement).
fn parse_deposit_submit_response(
    _body: &serde_json::Value,
) -> Result<DepositSubmitResponse, NearIntentsError> {
    // New API just acknowledges - the actual data was in the quote response
    Ok(DepositSubmitResponse { success: true })
}

/// Parse status response from the 1Click API.
fn parse_status_response(body: &serde_json::Value) -> Result<StatusResponse, NearIntentsError> {
    let raw = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN");

    let status = match raw {
        "PENDING_DEPOSIT" => RemoteStatus::PendingDeposit,
        "KNOWN_DEPOSIT_TX" => RemoteStatus::KnownDepositTx,
        "INCOMPLETE_DEPOSIT" => RemoteStatus::IncompleteDeposit,
        "PROCESSING" => RemoteStatus::Processing,
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
