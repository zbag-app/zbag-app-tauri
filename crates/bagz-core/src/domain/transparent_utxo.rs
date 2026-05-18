use serde::{Deserialize, Serialize};

use super::balance::Zatoshis;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: String,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransparentUTXO {
    pub outpoint: OutPoint,
    pub account_id: u32,
    pub value: Zatoshis,
    pub address: String,
    pub mined_height: u32,
    pub is_spent: bool,
    pub shielding_txid: Option<String>,
}
