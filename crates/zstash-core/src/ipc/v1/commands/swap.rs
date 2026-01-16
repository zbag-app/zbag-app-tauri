use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{SupportedToken, SwapInfo, SwapQuote, SwapType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestSwapQuoteRequest {
    pub schema_version: u32,
    pub swap_type: SwapType,
    pub input_asset: String,
    pub input_amount: String,
    pub output_asset: String,
    pub destination_address: Option<String>,
    pub refund_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetSupportedTokensRequest {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartSwapRequest {
    pub schema_version: u32,
    pub quote_id: String,
    pub allow_transparent_interaction: bool,
    pub reauth_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetSwapStatusRequest {
    pub schema_version: u32,
    pub swap_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListSwapsRequest {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSwapQuoteResponse {
    pub schema_version: u32,
    pub quote_id: String,
    pub quote: SwapQuote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSwapResponse {
    pub schema_version: u32,
    pub swap: SwapInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetSwapStatusResponse {
    pub schema_version: u32,
    pub swap: SwapInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListSwapsResponse {
    pub schema_version: u32,
    pub swaps: Vec<SwapInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetSupportedTokensResponse {
    pub schema_version: u32,
    pub tokens: Vec<SupportedToken>,
}
