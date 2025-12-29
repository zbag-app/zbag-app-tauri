use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use zkore_core::domain::{Network, ServerInfo};
use zkore_core::errors;
use zkore_engine::error::find_engine_ipc_error;
use zkore_engine::key_store::KeyStore;
use zkore_engine::wallet_manager::WalletManager;

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
    let root = std::env::temp_dir().join(format!("zkore_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn set_default_server_is_scoped_to_server_network() {
    let root = temp_root("us15_set_default_server");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let before =
        zkore_engine::db::server_meta::list_servers(mgr.app_db().conn()).expect("list servers");
    let mainnet_default_before = before
        .iter()
        .find(|s| s.network == Network::Mainnet && s.is_default)
        .expect("mainnet default seeded")
        .id;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let new_testnet = ServerInfo {
        id: Uuid::new_v4(),
        name: "Testnet Custom".to_string(),
        grpc_url: "https://example.testnet.invalid".to_string(),
        network: Network::Testnet,
        is_default: false,
        last_success_at: None,
    };
    zkore_engine::db::server_meta::insert_server(mgr.app_db().conn(), &new_testnet, now_ms)
        .expect("insert server");

    zkore_engine::db::server_meta::set_default_server(mgr.app_db_mut().conn_mut(), new_testnet.id)
        .expect("set default");

    let after =
        zkore_engine::db::server_meta::list_servers(mgr.app_db().conn()).expect("list servers");
    let mainnet_default_after = after
        .iter()
        .find(|s| s.network == Network::Mainnet && s.is_default)
        .expect("mainnet default")
        .id;
    assert_eq!(
        mainnet_default_before, mainnet_default_after,
        "setting testnet default must not affect mainnet default"
    );

    let testnet_defaults: Vec<_> = after
        .iter()
        .filter(|s| s.network == Network::Testnet && s.is_default)
        .collect();
    assert_eq!(testnet_defaults.len(), 1, "exactly one testnet default");
    assert_eq!(testnet_defaults[0].id, new_testnet.id);
}

#[test]
fn server_network_mismatch_is_rejected_for_active_wallet() {
    let root = temp_root("us15_server_network_mismatch");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let wallet = mgr
        .create_wallet("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    let err = mgr
        .ensure_server_network_matches_active_wallet(Network::Mainnet)
        .expect_err("mismatch should be rejected");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::INVALID_REQUEST);

    mgr.lock_wallet(wallet.id).expect("lock wallet");
}
