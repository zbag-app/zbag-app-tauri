use serde::{Deserialize, Serialize};

use crate::domain::Balance;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetBalanceRequest {
    pub schema_version: u32,
    pub account_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetBalanceResponse {
    pub schema_version: u32,
    pub balance: Balance,
}
