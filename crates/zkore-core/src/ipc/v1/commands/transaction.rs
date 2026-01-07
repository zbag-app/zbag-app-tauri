use serde::{Deserialize, Serialize};

use crate::domain::{RecipientKind, TransactionInfo, Zatoshis};

use super::super::common::UnixTimestampMs;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListTransactionsRequest {
    pub schema_version: u32,
    pub account_id: u32,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareSendRequest {
    pub schema_version: u32,
    pub account_id: u32,
    pub recipient: String,
    pub amount: Zatoshis,
    pub memo: Option<String>,
    pub allow_transparent_recipient: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmSendRequest {
    pub schema_version: u32,
    pub proposal_id: String,
    pub reauth_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelSendRequest {
    pub schema_version: u32,
    pub proposal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryBroadcastRequest {
    pub schema_version: u32,
    pub txid: String,
    pub reauth_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShieldFundsRequest {
    pub schema_version: u32,
    pub account_id: u32,
    pub consolidate: bool,
    pub reauth_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListTransactionsResponse {
    pub schema_version: u32,
    pub transactions: Vec<TransactionInfo>,
    pub total_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub recipient: String,
    pub recipient_kind: RecipientKind,
    pub amount: Zatoshis,
    pub fee: Zatoshis,
    pub memo_present: bool,
    pub total_spend: Zatoshis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareSendResponse {
    pub schema_version: u32,
    pub proposal_id: String,
    pub fee: Zatoshis,
    pub summary: TransactionSummary,
    pub expires_at: UnixTimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmSendResponse {
    pub schema_version: u32,
    pub txid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelSendResponse {
    pub schema_version: u32,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryBroadcastResponse {
    pub schema_version: u32,
    pub txid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldFundsResponse {
    pub schema_version: u32,
    pub txid: String,
    pub fee: Zatoshis,
}
