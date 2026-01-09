use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{AccountInfo, Network, RecipientKind, TransactionType, WalletInfo, Zatoshis};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigningSummary {
    pub recipient: String,
    pub recipient_kind: RecipientKind,
    pub amount: Zatoshis,
    pub fee: Zatoshis,
    pub memo_present: bool,
    pub tx_type: TransactionType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigningRequest {
    pub pczt_payload: String,
    pub qr_frames: Vec<String>,
    pub summary: SigningSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportUfvkRequest {
    pub schema_version: u32,
    pub wallet_id: Uuid,
    pub ufvk: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSigningRequestRequest {
    pub schema_version: u32,
    pub account_id: u32,
    pub recipient: String,
    pub amount: Zatoshis,
    pub memo: Option<String>,
    pub allow_transparent_recipient: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizeSigningRequest {
    pub schema_version: u32,
    pub signed_payload: String,
    pub reauth_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportUfvkResponse {
    pub schema_version: u32,
    pub account: AccountInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildSigningRequestResponse {
    pub schema_version: u32,
    pub signing_request: SigningRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizeSigningResponse {
    pub schema_version: u32,
    pub txid: String,
}

/// Create a standalone Keystone hardware wallet from a UFVK.
///
/// Unlike software wallets, this does NOT generate a mnemonic.
/// The UFVK provides view-only access; spending requires Keystone signing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateKeystoneWalletRequest {
    pub schema_version: u32,
    pub name: String,
    pub network: Network,
    pub password: String,
    pub remember_unlock: bool,
    pub ufvk: String,
    /// Optional birthday height for faster sync.
    /// If omitted, defaults to Sapling activation height (slower full-chain scan).
    pub birthday_height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateKeystoneWalletResponse {
    pub schema_version: u32,
    pub wallet: WalletInfo,
    pub account: AccountInfo,
}
