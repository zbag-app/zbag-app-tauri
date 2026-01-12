//! US5 receive addresses integration test (scaffold).
//!
//! Note: The workspace root is a virtual Cargo manifest (no root package), so this file is not
//! currently compiled by `cargo test --workspace`. It is kept here to match the spec-kit
//! task paths and can be moved into a dedicated integration-test crate if needed.
//!
//! Coverage intent (mirrors T120a):
//! - Each Receive open yields a fresh ShieldedOnly UA (diversifier rotation)
//! - Transparent compatibility address is stable per account (no rotation)
//! - Returned AddressInfo.address_type matches request
//! - Labeling/UX in Receive page reflects shielded vs transparent
//!
//! See `crates/zstash-engine/tests/us5_address_rotation.rs` for the executable unit coverage.

#[test]
fn placeholder() {
    // Intentionally empty.
}

