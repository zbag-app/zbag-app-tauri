use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPhase {
    Idle,
    Preparing,
    Downloading,
    Scanning,
    Enhancing,
    CatchingUp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncProgress {
    pub phase: SyncPhase,
    pub scan_frontier_height: u32,
    pub wallet_tip_height: u32,
    pub progress_percent: u8,
    pub eta_seconds: Option<u64>,
}
