use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapType {
    ToZec,
    FromZec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapState {
    Draft,
    AwaitingDeposit,
    Pending,
    Confirming,
    Completed,
    Refunded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapIntent {
    pub swap_type: SwapType,
    pub input_asset: String,
    pub input_amount: String,
    pub output_asset: String,
    pub destination_address: Option<String>,
    pub refund_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapInfo {
    pub id: Uuid,
    pub remote_id: Option<String>,
    pub swap_type: SwapType,
    pub input_asset: String,
    pub input_amount: String,
    pub output_asset: String,
    pub output_amount: Option<String>,
    pub deposit_address: Option<String>,
    pub deposit_memo: Option<String>,
    pub destination_address: Option<String>,
    pub refund_address: Option<String>,
    pub state: SwapState,
    pub deadline: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Quote information from the 1Click API.
///
/// This struct maps to the new API response format where amounts are provided
/// in both raw (smallest units) and formatted (human-readable) forms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapQuote {
    pub input_asset: String,
    /// Input amount in smallest units (e.g., wei for ETH, zatoshis for ZEC)
    pub input_amount: String,
    /// Human-readable input amount (from API's amountInFormatted)
    pub input_amount_formatted: String,
    pub output_asset: String,
    /// Output amount in smallest units
    pub output_amount: String,
    /// Human-readable output amount (from API's amountOutFormatted)
    pub output_amount_formatted: String,
    /// Minimum output amount in smallest units (accounting for slippage)
    pub min_output_amount: String,
    /// Deadline as milliseconds since epoch
    pub deadline: i64,
    /// Estimated time for swap completion in seconds
    pub time_estimate_secs: u64,
    /// Deposit address (present when dry=false)
    pub deposit_address: Option<String>,
    /// Deposit memo (present when deposit requires a memo)
    pub deposit_memo: Option<String>,
    /// Correlation ID for tracking the quote
    pub correlation_id: String,
}
