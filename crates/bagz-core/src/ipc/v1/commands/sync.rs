use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::SyncProgress;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartSyncRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopSyncRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetSyncProgressRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSyncResponse {
    pub schema_version: u32,
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopSyncResponse {
    pub schema_version: u32,
    pub stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetSyncProgressResponse {
    pub schema_version: u32,
    pub progress: SyncProgress,
}
