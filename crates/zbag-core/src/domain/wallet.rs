use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wallet type.
///
/// - `Software`: Wallet with mnemonic seed stored locally. Supports full spend capability.
/// - `WatchOnly`: Wallet created from a UFVK (e.g., Keystone hardware wallet). No local seed;
///   spending requires external signing device.
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
