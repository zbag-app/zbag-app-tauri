use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{AccountInfo, RecipientKind, TransactionType, Zatoshis};

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
