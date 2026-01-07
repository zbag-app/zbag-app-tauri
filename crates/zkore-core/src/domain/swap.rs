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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapQuote {
    pub input_asset: String,
    pub input_amount: String,
    pub output_asset: String,
    pub output_amount: String,
    pub fee_amount: String,
    pub fee_asset: String,
    pub deadline: i64,
    pub rate: String,
}
