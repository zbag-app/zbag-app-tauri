use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wallet type.
///
/// NOTE: In v1 wallets are always `Software`. `WatchOnly` is reserved for future use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletType {
    Software,
    WatchOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Network {
    Mainnet,
    Testnet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletInfo {
    pub id: Uuid,
    pub name: String,
    pub wallet_type: WalletType,
    pub network: Network,
    pub remember_unlock_enabled: bool,
    pub created_at: i64,
    pub last_opened_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wallet {
    pub info: WalletInfo,
}
