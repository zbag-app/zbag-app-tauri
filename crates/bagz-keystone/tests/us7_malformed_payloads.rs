use bagz_keystone::payload::decode_zcash_pczt_ur_cbor;

#[test]
fn zcash_pczt_ur_cbor_rejects_truncated_data() {
    let err = decode_zcash_pczt_ur_cbor(&[]).expect_err("empty cbor");
    assert_eq!(err.to_string(), "unexpected end of CBOR");
}

#[test]
fn zcash_pczt_ur_cbor_rejects_wrong_shape() {
    // map(0)
    let err = decode_zcash_pczt_ur_cbor(&[0xa0]).expect_err("map(0) should be rejected");
    assert_eq!(err.to_string(), "invalid zcash-pczt CBOR: expected map(1)");
}

#[test]
fn zcash_pczt_ur_cbor_rejects_trailing_bytes() {
    // map(1), key 1, bytes(0), plus extra trailing byte.
    let err = decode_zcash_pczt_ur_cbor(&[0xa1, 0x01, 0x40, 0x00])
        .expect_err("trailing bytes should be rejected");
    assert_eq!(err.to_string(), "invalid zcash-pczt CBOR: trailing bytes");
}
