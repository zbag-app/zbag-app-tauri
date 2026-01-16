use serde::{Deserialize, Serialize};

use super::balance::Zatoshis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionType {
    Send,
    Receive,
    Shield,
    Consolidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Expired,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipientKind {
    Orchard,
    Sapling,
    Transparent,
}

/// The kind of memo content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoKind {
    /// UTF-8 text memo (displayable).
    Text,
    /// Binary/arbitrary data (not directly displayable).
    Binary,
    /// Empty memo marker (0xF6).
    Empty,
}

/// Structured memo information for transaction display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoInfo {
    /// The kind of memo content.
    pub kind: MemoKind,
    /// The memo content as a string. For Text memos, this is the UTF-8 text.
    /// For Binary memos, this is a description like "[binary: 512 bytes]".
    /// For Empty memos, this is None.
    pub content: Option<String>,
    /// The size of the memo in bytes (0-512).
    pub size_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub account_id: u32,
    pub tx_type: TransactionType,
    pub value: Zatoshis,
    pub fee: Zatoshis,
    /// Total number of memos attached to this transaction (including empty/binary).
    pub memo_count: u32,
    /// Structured memo information for display.
    pub memos: Vec<MemoInfo>,
    pub status: TransactionStatus,
    pub last_error: Option<String>,
    pub can_retry_broadcast: bool,
    pub mined_height: Option<u32>,
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub info: TransactionInfo,
}
