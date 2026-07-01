use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapType {
    ToZec,
    FromZec,
}

/// Swap mode determines whether the amount specified is the input or output.
///
/// - `ExactInput`: User specifies the input amount, receives whatever output the swap yields.
/// - `ExactOutput`: User specifies the desired output amount, pays whatever input is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SwapMode {
    #[default]
    ExactInput,
    ExactOutput,
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
    /// Swap mode: ExactInput or ExactOutput (CrossPay).
    #[serde(default)]
    pub swap_mode: SwapMode,
    pub input_asset: String,
    /// Input amount (used for ExactInput mode).
    pub input_amount: String,
    pub output_asset: String,
    /// Output amount (used for ExactOutput/CrossPay mode).
    pub output_amount: Option<String>,
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

/// Supported token information for swaps.
///
/// This struct is populated from the 1Click `/v0/tokens` endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportedToken {
    /// Asset ID (e.g., "nep141:wrap.near")
    pub asset_id: String,
    /// Token symbol (e.g., "NEAR")
    pub symbol: String,
    /// Chain identifier (e.g., "near", "eth", "sol")
    pub chain: String,
    /// Token decimals
    pub decimals: u8,
    /// USD price (may be null/zero for filtering)
    pub usd_price: Option<f64>,
    /// Token icon URL (optional)
    pub icon: Option<String>,
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
    /// App/affiliate fee in basis points (1 bps = 0.01%). 50 bps = 0.50%.
    pub app_fee_bps: Option<u32>,
}
