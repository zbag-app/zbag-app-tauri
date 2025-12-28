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
