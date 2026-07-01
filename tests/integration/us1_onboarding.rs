//! US1 onboarding integration test (scaffold).
//!
//! Note: The workspace root is a virtual Cargo manifest (no root package), so this file is not
//! currently compiled by `cargo test --workspace`. It is kept here to match the spec-kit
//! task paths and can be moved into a dedicated integration-test crate if needed.
//!
//! Coverage intent (mirrors T085a):
//! - Create wallet (IPC/command-level)
//! - Backup challenge issuance/expiry/retry limits
//! - Verify backup and GetWalletStatus Required -> Complete
//! - Locked LoadWalletResponse returns accounts=[]
//! - Unlock + re-LoadWallet returns accounts.length >= 1
//!
//! See `crates/zbag-engine/tests/us1_backup_challenge.rs` for the executable unit coverage.

#[test]
fn placeholder() {
    // Intentionally empty. See module docs above.
}

