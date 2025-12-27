use serde::{Deserialize, Serialize};

use super::balance::Zatoshis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionType {
    Send,
    Receive,
    Shield,
    Consolidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Expired,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipientKind {
    Orchard,
    Sapling,
    Transparent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub account_id: u32,
    pub tx_type: TransactionType,
    pub value: Zatoshis,
    pub fee: Zatoshis,
    pub memo_present: bool,
    pub memo: Option<String>,
    pub status: TransactionStatus,
    pub last_error: Option<String>,
    pub can_retry_broadcast: bool,
    pub mined_height: Option<u32>,
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub info: TransactionInfo,
}
