use serde::{Deserialize, Serialize};

use super::balance::Zatoshis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletLockStatus {
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupAction {
    Required,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    Synced,
    Syncing { progress_percent: u8 },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShieldAction {
    None,
    Available { amount: Zatoshis },
    InProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyPosture {
    Optimal,
    NeedsAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletStatus {
    pub lock_status: WalletLockStatus,
    pub backup_status: BackupAction,
    pub sync_status: SyncStatus,
    pub shield_status: ShieldAction,
    pub privacy_posture: PrivacyPosture,
}
