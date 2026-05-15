use zcash_protocol::consensus::{Network, Parameters as _};
use zip32::AccountId;

#[allow(deprecated)]
use zcash_client_backend::keys::UnifiedSpendingKey;

#[test]
fn parses_mainnet_and_testnet_ufvk_and_extracts_network() {
    let seed = [7u8; 32];
    let account = AccountId::ZERO;

    #[allow(deprecated)]
    let usk_main =
        UnifiedSpendingKey::from_seed(&Network::MainNetwork, &seed, account).expect("mainnet usk");
    let ufvk_main = usk_main.to_unified_full_viewing_key();
    let encoded_main = ufvk_main.encode(&Network::MainNetwork);

    let parsed_main = bagz_keystone::ufvk::parse_ufvk(&encoded_main).expect("parse mainnet ufvk");
    assert_eq!(parsed_main.network, Network::MainNetwork.network_type());
    assert_eq!(parsed_main.ufvk.encode(&Network::MainNetwork), encoded_main);

    #[allow(deprecated)]
    let usk_test =
        UnifiedSpendingKey::from_seed(&Network::TestNetwork, &seed, account).expect("testnet usk");
    let ufvk_test = usk_test.to_unified_full_viewing_key();
    let encoded_test = ufvk_test.encode(&Network::TestNetwork);

    let parsed_test = bagz_keystone::ufvk::parse_ufvk(&encoded_test).expect("parse testnet ufvk");
    assert_eq!(parsed_test.network, Network::TestNetwork.network_type());
    assert_eq!(parsed_test.ufvk.encode(&Network::TestNetwork), encoded_test);
}

#[test]
fn rejects_invalid_ufvk() {
    let err = bagz_keystone::ufvk::parse_ufvk("not-a-ufvk").expect_err("invalid ufvk");
    let msg = err.to_string();
    assert!(!msg.trim().is_empty());
}
