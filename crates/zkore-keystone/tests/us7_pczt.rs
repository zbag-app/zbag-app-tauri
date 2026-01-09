use base64::Engine as _;

use zkore_keystone::payload::{decode_zcash_pczt_ur_cbor, encode_zcash_pczt_ur_cbor};

#[test]
fn zcash_pczt_ur_cbor_roundtrips() {
    let pczt_bytes = (0u8..=255).collect::<Vec<u8>>();
    let cbor = encode_zcash_pczt_ur_cbor(&pczt_bytes);
    let decoded = decode_zcash_pczt_ur_cbor(&cbor).expect("decode should succeed");
    assert_eq!(decoded, pczt_bytes);
}

#[test]
fn decode_pczt_base64_rejects_invalid_base64() {
    let err = zkore_keystone::pczt::decode_pczt_base64("not base64")
        .expect_err("invalid base64 should be rejected");
    assert_eq!(err.to_string(), "invalid base64 payload");
}

#[test]
fn decode_pczt_base64_rejects_invalid_pczt_bytes() {
    let payload = base64::engine::general_purpose::STANDARD.encode(b"definitely-not-a-pczt");
    let err = zkore_keystone::pczt::decode_pczt_base64(&payload)
        .expect_err("invalid pczt should be rejected");
    assert!(err.to_string().starts_with("invalid PCZT payload:"));
}

/// Test that encode_pczt_for_signer selectively redacts proprietary fields.
///
/// The two-PCZT flow (matching Zashi):
/// 1. decode_pczt_base64 preserves all fields (for combining signed PCZTs)
/// 2. encode_pczt_for_signer redacts `zcash_client_backend:proposal_info` from global
///    but keeps other proprietary fields
#[test]
fn pczt_encode_for_signer_selectively_redacts_proprietary_fields() {
    use pczt::roles::creator::Creator;
    use pczt::roles::updater::Updater;

    let pczt = Creator::new(0, 0, 0, [0; 32], [0; 32]).build();
    let pczt = Updater::new(pczt)
        .update_global_with(|mut global| {
            // This should be redacted (Zashi pattern)
            global.set_proprietary(
                "zcash_client_backend:proposal_info".to_string(),
                vec![1, 2, 3],
            );
            // This should NOT be redacted (device metadata)
            global.set_proprietary("keystone.device_id".to_string(), vec![4, 5, 6]);
        })
        .finish();

    // decode_pczt_base64 should preserve all proprietary fields
    let raw_payload = base64::engine::general_purpose::STANDARD.encode(pczt.serialize());
    let decoded =
        zkore_keystone::pczt::decode_pczt_base64(&raw_payload).expect("decode should succeed");
    assert_eq!(decoded.global().proprietary().len(), 2);

    // encode_pczt_for_signer should selectively redact
    let redacted_payload = zkore_keystone::pczt::encode_pczt_for_signer(&pczt);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(redacted_payload)
        .expect("decode redacted base64");
    let reparsed = pczt::Pczt::parse(&bytes).expect("parse should succeed");

    // proposal_info should be redacted, keystone.device_id should remain
    assert!(
        !reparsed
            .global()
            .proprietary()
            .contains_key("zcash_client_backend:proposal_info")
    );
    assert!(
        reparsed
            .global()
            .proprietary()
            .contains_key("keystone.device_id")
    );
}
