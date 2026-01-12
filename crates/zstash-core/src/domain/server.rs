use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::wallet::Network;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub grpc_url: String,
    pub network: Network,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: Uuid,
    pub name: String,
    pub grpc_url: String,
    pub network: Network,
    pub is_default: bool,
    pub last_success_at: Option<i64>,
}
