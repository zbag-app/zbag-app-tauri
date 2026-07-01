//! US4 restore integration test (scaffold).
//!
//! Note: The workspace root is a virtual Cargo manifest (no root package), so this file is not
//! currently compiled by `cargo test --workspace`. It is kept here to match the spec-kit
//! task paths and can be moved into a dedicated integration-test crate if needed.
//!
//! Coverage intent (mirrors T115a):
//! - RestoreWallet seed validation (24 words, English)
//! - Birthday height estimation
//! - Restore progress/sync progress emission
//! - Spend-before-sync semantics (spendable vs pending)
//! - Restored wallets are not blocked by BACKUP_REQUIRED
//!
//! See `crates/zbag-engine/tests/us4_restore.rs` for the executable unit coverage.

#[test]
fn placeholder() {
    // Intentionally empty.
}

