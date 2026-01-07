use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{AccountInfo, Network, WalletInfo, WalletLockStatus};

use super::super::common::UnixTimestampMs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReauthPurpose {
    Spend,
    ViewSeedPhrase,
    Logout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWalletRequest {
    pub schema_version: u32,
    pub name: String,
    pub network: Network,
    pub password: String,
    pub remember_unlock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadWalletRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListWalletsRequest {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetWalletStatusRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnlockWalletRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
    pub password: String,
    pub remember_unlock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockWalletRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReauthWalletRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
    pub password: String,
    pub purpose: ReauthPurpose,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSeedPhraseRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
    pub reauth_token: String,
}

/// Request to logout from the active wallet.
/// Requires a valid reauth token with Logout purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogoutWalletRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
    pub reauth_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupChallenge {
    pub challenge_id: String,
    pub indices: Vec<u8>,
    pub expires_at: UnixTimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateWalletResponse {
    pub schema_version: u32,
    pub wallet: WalletInfo,
    pub seed_phrase: Vec<String>,
    pub backup_challenge: BackupChallenge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockWalletResponse {
    pub schema_version: u32,
    pub unlocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockWalletResponse {
    pub schema_version: u32,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReauthWalletResponse {
    pub schema_version: u32,
    pub reauth_token: String,
    pub expires_at: UnixTimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewSeedPhraseResponse {
    pub schema_version: u32,
    pub seed_phrase: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogoutWalletResponse {
    pub schema_version: u32,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadWalletResponse {
    pub schema_version: u32,
    pub wallet: WalletInfo,
    pub lock_status: WalletLockStatus,
    pub accounts: Vec<AccountInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListWalletsResponse {
    pub schema_version: u32,
    pub wallets: Vec<WalletInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetWalletStatusResponse {
    pub schema_version: u32,
    pub status: crate::domain::WalletStatus,
}
