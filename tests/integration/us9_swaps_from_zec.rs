//! US9 swap-from-ZEC integration test (scaffold).
//!
//! Note: The workspace root is a virtual Cargo manifest (no root package), so this file is not
//! currently compiled by `cargo test --workspace`. It is kept here to match the spec-kit
//! task paths and document the checks required by T165b.
//!
//! Coverage intent:
//! - Mock the 1Click API endpoints (quote/deposit/status) and verify state transitions/events.
//! - Assert shielded-only spend enforcement (TRANSPARENT_SPEND_BLOCKED when only transparent funds available).
//! - Assert fail-closed privacy acknowledgement (PRIVACY_ACK_REQUIRED unless allow_transparent_interaction=true when required).

#[test]
fn placeholder() {
    // Intentionally empty.
}

