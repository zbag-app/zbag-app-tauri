use serde::{Deserialize, Serialize};

use crate::domain::TorState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TorStatusEvent {
    pub schema_version: u32,
    pub event: String,
    pub state: TorState,
}

