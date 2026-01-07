//! US7 malformed signing input regression tests (scaffold).
//!
//! Note: The workspace root is a virtual Cargo manifest (no root package), so this file is not
//! currently compiled by `cargo test --workspace`. It is kept here to match the spec-kit
//! task paths and document the checks required by T141b.
//!
//! Coverage intent:
//! - Truncated/corrupted/oversized animated-QR frame sets (UI ingestion)
//! - Invalid `.pczt` file imports (non-PCZT bytes)
//! - Malformed PCZT payloads returned from hardware signer
//! - Assert stable error codes and no panics across IPC boundaries

#[test]
fn placeholder() {
    // Intentionally empty.
}
