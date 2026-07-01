use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Network, WalletInfo};
use crate::sensitive::SensitiveString;

use super::super::common::UnixTimestampMs;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupChallenge {
    pub challenge_id: String,
    pub indices: Vec<u8>,
    pub expires_at: UnixTimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetBackupChallengeRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyBackupRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
    pub challenge_id: String,
    /// The challenged words entered by the user.
    ///
    /// SECURITY: `SensitiveString` helps limit retention on the Rust side, but this data still
    /// crosses the IPC boundary and exists as plaintext strings on the frontend/JS side.
    pub word_challenges: std::collections::BTreeMap<u8, SensitiveString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreWalletRequest {
    pub schema_version: u32,
    pub name: String,
    pub network: Network,
    /// SECURITY: `SensitiveString` helps limit retention on the Rust side, but this data still
    /// crosses the IPC boundary and exists as plaintext strings on the frontend/JS side.
    pub password: SensitiveString,
    /// DISABLED: Keychain biometric auto-unlock is disabled. Always pass `false`.
    /// See SECURITY.md for rationale
    pub remember_unlock: bool,
    /// The 24-word seed phrase entered by the user.
    ///
    /// SECURITY: `SensitiveString` helps limit retention on the Rust side, but this data still
    /// crosses the IPC boundary and exists as plaintext strings on the frontend/JS side.
    pub seed_phrase: SensitiveString,
    pub birthday_date: Option<UnixTimestampMs>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetBackupChallengeResponse {
    pub schema_version: u32,
    pub challenge: BackupChallenge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyBackupResponse {
    pub schema_version: u32,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreWalletResponse {
    pub schema_version: u32,
    pub wallet: WalletInfo,
    pub birthday_height: u32,
}
