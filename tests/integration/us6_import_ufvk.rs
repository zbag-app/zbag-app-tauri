//! US6 UFVK import integration test (scaffold).
//!
//! Note: The workspace root is a virtual Cargo manifest (no root package), so this file is not
//! currently compiled by `cargo test --workspace`. It is kept here to match the spec-kit
//! task paths and can be moved into a dedicated integration-test crate if needed.
//!
//! Coverage intent (mirrors T127a):
//! - ImportUfvk creates a HardwareSigner account and it appears in LoadWallet accounts
//! - UFVK network mismatch returns INVALID_UFVK
//! - Active-account switching between Software and HardwareSigner accounts updates balance/address views
//! - Watch-only spend attempts trigger signing flow (US7)
//!
//! See `crates/bagz-keystone/tests/us6_ufvk.rs` and `crates/bagz-engine/tests/us6_import_ufvk.rs`
//! for executable unit coverage.

#[test]
fn placeholder() {
    // Intentionally empty.
}

