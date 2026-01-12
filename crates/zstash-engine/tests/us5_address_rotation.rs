use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use zkore_core::domain::{AddressType, Network};
use zkore_engine::key_store::KeyStore;
use zkore_engine::wallet_manager::WalletManager;

type StoreKey = (Uuid, u8);
type Store = HashMap<StoreKey, Vec<u8>>;
type SharedStore = Arc<Mutex<Store>>;

#[derive(Debug, Default, Clone)]
struct TestKeyStore {
    encrypted_mnemonics: SharedStore,
}

impl KeyStore for TestKeyStore {
    fn store_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()> {
        self.encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .insert(
                (wallet_id, network_key(network)),
                encrypted_mnemonic.to_vec(),
            );
        Ok(())
    }

    fn load_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self
            .encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .get(&(wallet_id, network_key(network)))
            .cloned())
    }

    fn delete_encrypted_mnemonic(&self, wallet_id: Uuid, network: Network) -> anyhow::Result<()> {
        self.encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .remove(&(wallet_id, network_key(network)));
        Ok(())
    }

    fn store_keychain_unlock_material(
        &self,
        _wallet_id: Uuid,
        _network: Network,
        _unlock_material: &[u8],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn load_keychain_unlock_material(
        &self,
        _wallet_id: Uuid,
        _network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn delete_keychain_unlock_material(
        &self,
        _wallet_id: Uuid,
        _network: Network,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

fn network_key(network: Network) -> u8 {
    match network {
        Network::Mainnet => 0,
        Network::Testnet => 1,
    }
}

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("zkore_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn receive_shielded_addresses_rotate_and_are_shielded_only() {
    let root = temp_root("us5_address_rotation");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    mgr.create_wallet("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    let a1 = mgr
        .get_receive_address(0, AddressType::ShieldedOnly)
        .expect("get receive address #1");
    let a2 = mgr
        .get_receive_address(0, AddressType::ShieldedOnly)
        .expect("get receive address #2");

    assert_ne!(a1.encoded, a2.encoded, "shielded addresses must rotate");
    assert_ne!(
        a1.diversifier_index, a2.diversifier_index,
        "diversifier index must change when rotating"
    );

    let di1: u64 = a1
        .diversifier_index
        .parse()
        .expect("parse diversifier_index #1");
    let di2: u64 = a2
        .diversifier_index
        .parse()
        .expect("parse diversifier_index #2");
    assert!(di2 > di1, "diversifier index must increase");

    #[allow(deprecated)]
    let parsed = zcash_client_backend::address::Address::decode(
        &zcash_protocol::consensus::Network::TestNetwork,
        &a1.encoded,
    )
    .expect("decode unified address");

    #[allow(deprecated)]
    match parsed {
        zcash_client_backend::address::Address::Unified(ua) => {
            assert!(ua.transparent().is_none(), "must omit transparent receiver");
            assert!(ua.has_orchard(), "must include orchard receiver");
            assert!(ua.has_sapling(), "must allow sapling receiver");
        }
        other => panic!("expected unified address, got {other:?}"),
    }
}

#[test]
fn transparent_receive_address_is_stable_per_account() {
    let root = temp_root("us5_transparent_stable");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    mgr.create_wallet("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    let t1 = mgr
        .get_receive_address(0, AddressType::Transparent)
        .expect("get transparent receive address #1");
    let t2 = mgr
        .get_receive_address(0, AddressType::Transparent)
        .expect("get transparent receive address #2");

    assert_eq!(
        t1.encoded, t2.encoded,
        "transparent compatibility address must be stable"
    );

    #[allow(deprecated)]
    let parsed = zcash_client_backend::address::Address::decode(
        &zcash_protocol::consensus::Network::TestNetwork,
        &t1.encoded,
    )
    .expect("decode transparent address");

    #[allow(deprecated)]
    match parsed {
        zcash_client_backend::address::Address::Transparent(_) => {}
        other => panic!("expected transparent address, got {other:?}"),
    }
}
