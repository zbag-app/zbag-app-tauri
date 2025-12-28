use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TorStatus {
    Off,
    Connecting,
    On,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TorState {
    pub enabled: bool,
    pub status: TorStatus,
    pub last_error: Option<String>,
}

