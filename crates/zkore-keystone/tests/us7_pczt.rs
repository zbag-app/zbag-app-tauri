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

#[test]
fn pczt_encode_and_decode_strip_proprietary_fields() {
    use pczt::roles::creator::Creator;
    use pczt::roles::updater::Updater;

    let pczt = Creator::new(0, 0, 0, [0; 32], [0; 32]).build();
    let pczt = Updater::new(pczt)
        .update_global_with(|mut global| {
            global.set_proprietary("keystone.device_id".to_string(), vec![1, 2, 3]);
        })
        .finish();

    let raw_payload = base64::engine::general_purpose::STANDARD.encode(pczt.serialize());
    let decoded =
        zkore_keystone::pczt::decode_pczt_base64(&raw_payload).expect("decode should succeed");
    assert!(decoded.global().proprietary().is_empty());

    let stripped_payload = zkore_keystone::pczt::encode_pczt_base64(&pczt);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(stripped_payload)
        .expect("decode stripped base64");
    let reparsed = pczt::Pczt::parse(&bytes).expect("parse should succeed");
    assert!(reparsed.global().proprietary().is_empty());
}
