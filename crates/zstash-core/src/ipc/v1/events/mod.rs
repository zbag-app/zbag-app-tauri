use serde::{Deserialize, Serialize};

use crate::domain::{Balance, JobProgress, Network, SyncProgress, TransactionInfo, WalletStatus};

pub mod swap;
pub use swap::SwapChangedEvent;

pub mod tor;
pub use tor::TorStatusEvent;

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

/// Event emitted when a background job's progress changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobProgressEvent {
    pub schema_version: u32,
    pub event: String,
    pub progress: JobProgress,
}

/// Event emitted when send/broadcast transport failures trigger server failover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerFailoverEvent {
    pub schema_version: u32,
    pub event: String,
    pub network: Network,
    pub from_server_id: String,
    pub from_server_name: String,
    pub from_grpc_url: String,
    pub to_server_id: String,
    pub to_server_name: String,
    pub to_grpc_url: String,
    pub reason: String,
}
