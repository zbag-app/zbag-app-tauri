use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPhase {
    Idle,
    Preparing,
    Downloading,
    Scanning,
    Enhancing,
    CatchingUp,
    /// Network is unreachable; sync is retrying with exponential backoff.
    Offline,
    /// A local error occurred (DB, scan, etc.) - not a network issue.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncProgress {
    pub phase: SyncPhase,
    pub scan_frontier_height: u32,
    pub wallet_tip_height: u32,
    pub progress_percent: u8,
    pub eta_seconds: Option<u64>,
    /// Seconds until the next retry attempt (populated when phase is Offline or Error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_in_seconds: Option<u64>,
    /// User-safe, high-level error message (populated when phase is Error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
