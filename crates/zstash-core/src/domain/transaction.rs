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
///
/// This struct represents memo data in a display-friendly format. The `content` field
/// contains human-readable strings, not raw binary data. For Binary memos, the actual
/// payload is not exposed; only a placeholder description is provided. Use `size_bytes`
/// to determine the true payload size for all memo kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoInfo {
    /// The kind of memo content.
    pub kind: MemoKind,
    /// The memo content as a displayable string.
    ///
    /// - **Text memos:** The actual UTF-8 text content, ready for display.
    /// - **Binary memos:** A placeholder description (e.g., `"[binary: 512 bytes]"`),
    ///   NOT the raw binary payload. The actual binary data is not exposed through
    ///   this field to avoid display issues and potential security concerns.
    /// - **Empty memos:** `None` (no displayable content).
    pub content: Option<String>,
    /// The true payload size in bytes (0-512).
    ///
    /// This represents the actual byte length of the memo payload, regardless of
    /// what `content` displays:
    ///
    /// - **Text memos:** The UTF-8 byte length of the text.
    /// - **Binary memos:** The actual binary payload size (not the length of the
    ///   placeholder string in `content`).
    /// - **Empty memos:** 0 (representing no logical content), even though the wire
    ///   format uses a single 0xF6 marker byte. This semantic distinction is
    ///   intentional: `size_bytes` represents usable content size, not raw storage size.
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
