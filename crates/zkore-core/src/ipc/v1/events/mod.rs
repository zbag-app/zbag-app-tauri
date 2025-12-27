use serde::{Deserialize, Serialize};

use crate::domain::{Balance, SyncProgress, TransactionInfo, WalletStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncProgressEvent {
    pub schema_version: u32,
    pub event: String,
    pub progress: SyncProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceChangedEvent {
    pub schema_version: u32,
    pub event: String,
    pub account_id: u32,
    pub balance: Balance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionChangedEvent {
    pub schema_version: u32,
    pub event: String,
    pub transaction: TransactionInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletStatusEvent {
    pub schema_version: u32,
    pub event: String,
    pub status: WalletStatus,
}
