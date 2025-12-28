use serde::{Deserialize, Serialize};

use crate::domain::SwapInfo;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapChangedEvent {
    pub schema_version: u32,
    pub event: String,
    pub swap: SwapInfo,
}

