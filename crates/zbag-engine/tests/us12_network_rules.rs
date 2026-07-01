use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use zbag_core::domain::{AddressType, Network};
use zbag_core::errors;
use zbag_engine::error::find_engine_ipc_error;
use zbag_engine::key_store::KeyStore;
use zbag_engine::wallet_manager::WalletManager;

#[derive(Debug, Default, Clone)]
struct TestKeyStore {
    encrypted_mnemonics: Arc<Mutex<Vec<u8>>>,
}

impl KeyStore for TestKeyStore {
    fn store_encrypted_mnemonic(
        &self,
        _wallet_id: Uuid,
        _network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()> {
        *self.encrypted_mnemonics.lock().expect("mutex poisoned") = encrypted_mnemonic.to_vec();
        Ok(())
    }

    fn load_encrypted_mnemonic(
        &self,
        _wallet_id: Uuid,
        _network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let bytes = self
            .encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .clone();
        if bytes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(bytes))
        }
    }

    fn delete_encrypted_mnemonic(&self, _wallet_id: Uuid, _network: Network) -> anyhow::Result<()> {
        self.encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .clear();
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

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("zbag_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn wallet_directories_are_network_scoped_and_network_is_immutable() {
    let root = temp_root("us12_network_immutability");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root.clone(),
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let wallet = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    let expected_dir = wallets_root.join("testnet").join(wallet.id.to_string());
    assert!(
        expected_dir.exists(),
        "expected testnet wallet dir to exist: {}",
        expected_dir.display()
    );

    // Tamper the app DB network field to simulate an illegal network change.
    mgr.app_db()
        .conn()
        .execute(
            "UPDATE wallets SET network = 'Mainnet' WHERE id = ?1",
            [wallet.id.to_string()],
        )
        .expect("update network field");

    let err = mgr
        .load_wallet_for_test(wallet.id)
        .expect_err("network change should be rejected");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::INVALID_REQUEST);
}

#[test]
fn address_prefixes_match_network() {
    let root = temp_root("us12_address_prefixes");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let mainnet = mgr
        .create_wallet_for_test("Main", Network::Mainnet, "pw", false, None)
        .expect("create mainnet wallet")
        .wallet;
    let main_shielded = mgr
        .get_receive_address(0, AddressType::ShieldedOnly)
        .expect("mainnet shielded address");
    assert!(
        main_shielded.encoded.starts_with("u1"),
        "expected mainnet UA prefix u1..., got {}",
        main_shielded.encoded
    );
    let main_transparent = mgr
        .get_receive_address(0, AddressType::Transparent)
        .expect("mainnet transparent address");
    assert!(
        main_transparent.encoded.starts_with('t'),
        "expected mainnet t-addr prefix, got {}",
        main_transparent.encoded
    );

    // Switch to testnet wallet and assert prefixes.
    let testnet = mgr
        .create_wallet_for_test("Test", Network::Testnet, "pw2", false, None)
        .expect("create testnet wallet")
        .wallet;
    mgr.load_wallet_for_test(testnet.id)
        .expect("load testnet wallet");

    let test_shielded = mgr
        .get_receive_address(0, AddressType::ShieldedOnly)
        .expect("testnet shielded address");
    assert!(
        test_shielded.encoded.starts_with("utest1"),
        "expected testnet UA prefix utest1..., got {}",
        test_shielded.encoded
    );
    let test_transparent = mgr
        .get_receive_address(0, AddressType::Transparent)
        .expect("testnet transparent address");
    assert!(
        test_transparent.encoded.starts_with("tm"),
        "expected testnet t-addr prefix tm..., got {}",
        test_transparent.encoded
    );

    // Silence unused wallet variables, but keep them visible for debugging.
    let _ = mainnet;
}
