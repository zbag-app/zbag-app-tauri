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
    /// The logical content size in bytes (0-512).
    ///
    /// For Text/Binary memos, this is the actual byte length of the content.
    /// For Empty memos, this is 0 (representing no logical content), even though
    /// the wire format uses a single 0xF6 marker byte. This semantic distinction
    /// is intentional: `size_bytes` represents displayable/usable content size,
    /// not raw storage size.
    pub size_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub account_id: u32,
    pub tx_type: TransactionType,
    pub value: Zatoshis,
    pub fee: Zatoshis,
    /// Number of unique memos for this transaction (deduplicated across sent/received notes).
    /// Includes empty and binary memos. Self-send transactions with identical memos in both
    /// sent and received notes will have a count of 1.
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
