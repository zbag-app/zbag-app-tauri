use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use zstash_core::domain::{AddressType, BackupAction, Network};
use zstash_core::errors;
use zstash_core::sensitive::SensitiveString;
use zstash_engine::error::find_engine_ipc_error;
use zstash_engine::key_store::KeyStore;
use zstash_engine::wallet_manager::WalletManager;

type StoreKey = (Uuid, u8);
type Store = HashMap<StoreKey, Vec<u8>>;
type SharedStore = Arc<Mutex<Store>>;

#[derive(Debug, Default, Clone)]
struct TestKeyStore {
    encrypted_mnemonics: SharedStore,
    keychain: SharedStore,
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
        wallet_id: Uuid,
        network: Network,
        unlock_material: &[u8],
    ) -> anyhow::Result<()> {
        self.keychain
            .lock()
            .expect("mutex poisoned")
            .insert((wallet_id, network_key(network)), unlock_material.to_vec());
        Ok(())
    }

    fn load_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self
            .keychain
            .lock()
            .expect("mutex poisoned")
            .get(&(wallet_id, network_key(network)))
            .cloned())
    }

    fn delete_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<()> {
        self.keychain
            .lock()
            .expect("mutex poisoned")
            .remove(&(wallet_id, network_key(network)));
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
    let root = std::env::temp_dir().join(format!("zstash_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn assert_ipc_err_code(err: &anyhow::Error, code: &str) {
    let Some(ipc) = find_engine_ipc_error(err) else {
        panic!("expected EngineIpcError, got: {err:?}");
    };
    assert_eq!(ipc.code, code, "unexpected error: {ipc:?}");
}

#[test]
fn restore_wallet_rejects_invalid_seed_phrase() {
    let root = temp_root("us4_restore_invalid_seed");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let err = mgr
        .restore_wallet(
            "Restored",
            Network::Testnet,
            "pw",
            false,
            SensitiveString::from("this is not a valid 24 word seed phrase"),
            None,
        )
        .expect_err("restore must fail");

    assert_ipc_err_code(&err, errors::INVALID_SEED_PHRASE);
}

#[test]
fn restore_wallet_marks_backup_complete_and_spend_is_not_blocked_by_backup_required() {
    let root = temp_root("us4_restore_backup_complete");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet("Seed Source", Network::Testnet, "pw", false, None)
        .expect("create wallet");
    let seed_phrase = created
        .seed_phrase
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(" ");
    let seed_phrase: SensitiveString = seed_phrase.into();

    let restored = mgr
        .restore_wallet(
            "Restored",
            Network::Testnet,
            "pw2",
            false,
            seed_phrase,
            None,
        )
        .expect("restore wallet");

    let status = mgr
        .compute_wallet_status(restored.wallet.id)
        .expect("compute wallet status");
    assert_eq!(status.backup_status, BackupAction::Complete);

    let recipient = mgr
        .get_receive_address(0, AddressType::ShieldedOnly)
        .expect("get receive address");

    let err = mgr
        .prepare_send(0, &recipient.encoded, "1", None, false)
        .expect_err(
            "send should fail (no funds / not yet scanned), but must not be BACKUP_REQUIRED",
        );

    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_ne!(ipc.code, errors::BACKUP_REQUIRED);
}

#[test]
fn restore_wallet_returns_birthday_height_estimate() {
    let root = temp_root("us4_restore_birthday_height");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet("Seed Source", Network::Mainnet, "pw", false, None)
        .expect("create wallet");
    let seed_phrase = created
        .seed_phrase
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(" ");
    let seed_phrase: SensitiveString = seed_phrase.into();

    let birthday_date_ms: i64 = 1_704_067_200_000; // 2024-01-01T00:00:00Z
    let expected =
        zstash_engine::birthday::estimate_birthday_height(Network::Mainnet, birthday_date_ms);

    let restored = mgr
        .restore_wallet(
            "Restored",
            Network::Mainnet,
            "pw2",
            false,
            seed_phrase,
            Some(birthday_date_ms),
        )
        .expect("restore wallet");

    assert_eq!(restored.birthday_height, expected);
}
