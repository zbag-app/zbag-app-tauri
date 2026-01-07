//! US7 Keystone signing flow integration test (scaffold).
//!
//! Note: The workspace root is a virtual Cargo manifest (no root package), so this file is not
//! currently compiled by `cargo test --workspace`. It is kept here to match the spec-kit
//! task paths and document the checks required by T141a.
//!
//! Coverage intent:
//! - Create a wallet + import UFVK, then BuildSigningRequest for a HardwareSigner account.
//! - Sign the PCZT (test harness) and call FinalizeSigning.
//! - Assert broadcast attempt is made and failures are queued with stable error codes.
//! - Verify memo and transparent-recipient rules, and BACKUP_REQUIRED gating.

#[test]
fn placeholder() {
    // Intentionally empty.
}
