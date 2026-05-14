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
    /// Validation error message if the URL is invalid, `None` if valid.
    /// Populated by `list_servers` to inform the frontend which servers have invalid URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_error: Option<String>,
}
