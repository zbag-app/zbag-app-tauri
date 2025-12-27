use serde::{Deserialize, Serialize};

pub type Zatoshis = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    pub shielded_spendable: Zatoshis,
    pub shielded_pending: Zatoshis,
    pub transparent_total: Zatoshis,
    pub total: Zatoshis,
}
