mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;
use zip32::AccountId;

use zcash_protocol::consensus::Network as ConsensusNetwork;
use bagz_core::domain::{AccountType, AddressType, Network};
use bagz_core::errors;
use bagz_engine::error::find_engine_ipc_error;
use bagz_engine::key_store::KeyStore;
use bagz_engine::wallet_manager::WalletManager;

type StoreKey = (Uuid, u8);
type Store = HashMap<StoreKey, Vec<u8>>;
type SharedStore = Arc<Mutex<Store>>;

#[allow(deprecated)]
use zcash_client_backend::keys::UnifiedSpendingKey;

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
    let root = std::env::temp_dir().join(format!("bagz_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn import_ufvk_creates_hardware_signer_account_and_blocks_spend() {
    let root = temp_root("us6_import_ufvk");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");
    let answers =
        common::solve_backup_challenge(&created.seed_phrase, &created.backup_challenge.indices);
    mgr.verify_backup(
        created.wallet.id,
        &created.backup_challenge.challenge_id,
        &answers,
    )
    .expect("verify backup");

    let seed = [9u8; 32];
    let usk = UnifiedSpendingKey::from_seed(&ConsensusNetwork::TestNetwork, &seed, AccountId::ZERO)
        .expect("derive usk");
    let ufvk = usk.to_unified_full_viewing_key();
    let ufvk_str = ufvk.encode(&ConsensusNetwork::TestNetwork);

    let imported = mgr
        .import_ufvk(created.wallet.id, &ufvk_str, "Keystone", None, None)
        .expect("import ufvk");

    assert_eq!(imported.id, 1);
    assert_eq!(imported.name, "Keystone");
    assert_eq!(imported.account_type, AccountType::HardwareSigner);

    let ids = mgr
        .list_wallet_db_account_ids(created.wallet.id)
        .expect("list wallet db account ids");
    assert_eq!(ids, vec![0, 1]);

    let _ = mgr
        .get_receive_address(imported.id, AddressType::ShieldedOnly)
        .expect("get receive address for imported account");
    let _ = mgr
        .get_balance(imported.id)
        .expect("get balance for imported account");

    let err = mgr
        .prepare_send_for_test(imported.id, "anything", "1", None, false)
        .expect_err("watch-only account cannot spend");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::WATCH_ONLY_CANNOT_SPEND);
}

#[test]
fn import_ufvk_rejects_network_mismatch() {
    let root = temp_root("us6_import_ufvk_mismatch");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    let seed = [11u8; 32];
    let usk = UnifiedSpendingKey::from_seed(&ConsensusNetwork::MainNetwork, &seed, AccountId::ZERO)
        .expect("derive usk");
    let ufvk = usk.to_unified_full_viewing_key();
    let ufvk_str = ufvk.encode(&ConsensusNetwork::MainNetwork);

    let err = mgr
        .import_ufvk(created.wallet.id, &ufvk_str, "Wrong net", None, None)
        .expect_err("network mismatch must fail");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::INVALID_UFVK);
}
