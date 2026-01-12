use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OpenFlags};
use uuid::Uuid;
use zeroize::Zeroize;

use zstash_core::domain::{Network, WalletLockStatus};
use zstash_core::ipc::v1::commands::wallet::ReauthPurpose;
use zstash_engine::db::wallet_encryption_meta;
use zstash_engine::encryption;
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

fn wallet_db_path(wallets_root: &Path, network: Network, wallet_id: Uuid) -> PathBuf {
    let network_dir = match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
    };
    wallets_root
        .join(network_dir)
        .join(wallet_id.to_string())
        .join("wallet.sqlite")
}

fn open_sqlcipher_conn(path: &Path, dek: &[u8; 32], create_if_missing: bool) -> Connection {
    let flags = if create_if_missing {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let conn = Connection::open_with_flags(path, flags).expect("open wallet db");

    let mut dek_hex = dek.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");
    conn.execute_batch(&pragma).expect("apply key");
    dek_hex.zeroize();
    pragma.zeroize();

    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
        .expect("validate db");

    conn
}

#[test]
fn wallet_db_is_encrypted_and_unlock_requires_correct_password() {
    let root = temp_root("wallet_db_enc");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root.clone(),
        Box::new(key_store),
    )
    .expect("create wallet manager");

    let password = "correct horse battery staple";
    let wallet = mgr
        .create_wallet("Test Wallet", Network::Testnet, password, false, None)
        .expect("create wallet")
        .wallet;

    let db_path = wallet_db_path(&wallets_root, Network::Testnet, wallet.id);
    assert!(db_path.exists(), "wallet db file should exist");

    // Verify the wallet DB is encrypted-at-rest.
    let conn_plain = Connection::open(&db_path).expect("open without key");
    let plaintext_query = conn_plain.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    });
    assert!(
        plaintext_query.is_err(),
        "wallet db should not be readable without key"
    );

    assert!(
        mgr.unlock_wallet(wallet.id, "wrong-password", false)
            .is_err(),
        "unlock with wrong password must fail"
    );

    mgr.unlock_wallet(wallet.id, password, false)
        .expect("unlock with correct password should succeed");

    let meta = wallet_encryption_meta::get_wallet_encryption(mgr.app_db().conn(), wallet.id)
        .expect("load wallet encryption meta")
        .expect("wallet encryption meta missing");
    let dek = encryption::unwrap_dek(
        wallet.id,
        Network::Testnet,
        password,
        &meta.kdf.salt_b64,
        &meta.aead.nonce_b64,
        &meta.wrapped_dek_b64,
    )
    .expect("unwrap DEK");

    let conn = open_sqlcipher_conn(&db_path, &dek.0, false);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))
        .expect("query sqlite_master");
    assert!(count > 0);
}

#[test]
#[cfg(debug_assertions)]
fn wallet_db_migration_snapshot_rolls_back_on_validation_failure() {
    let root = temp_root("wallet_db_migrate");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root.clone(),
        Box::new(key_store),
    )
    .expect("create wallet manager");

    let password = "pw";
    let wallet = mgr
        .create_wallet("Test Wallet", Network::Testnet, password, false, None)
        .expect("create wallet")
        .wallet;
    mgr.lock_wallet(wallet.id).expect("lock wallet");

    let meta = wallet_encryption_meta::get_wallet_encryption(mgr.app_db().conn(), wallet.id)
        .expect("load wallet encryption meta")
        .expect("wallet encryption meta missing");
    let dek = encryption::unwrap_dek(
        wallet.id,
        Network::Testnet,
        password,
        &meta.kdf.salt_b64,
        &meta.aead.nonce_b64,
        &meta.wrapped_dek_b64,
    )
    .expect("unwrap DEK");

    // Replace the wallet DB with a minimal encrypted DB to force migrations to make changes.
    let db_path = wallet_db_path(&wallets_root, Network::Testnet, wallet.id);
    std::fs::remove_file(&db_path).expect("remove migrated db");
    let conn = open_sqlcipher_conn(&db_path, &dek.0, true);
    conn.execute_batch("CREATE TABLE dummy(id INTEGER); INSERT INTO dummy(id) VALUES (1);")
        .expect("seed dummy table");
    drop(conn);

    mgr.__set_wallet_db_force_validate_fail(true);
    assert!(
        mgr.unlock_wallet(wallet.id, password, false).is_err(),
        "forced validation failure should bubble up"
    );
    mgr.__set_wallet_db_force_validate_fail(false);

    // Ensure the DB was restored to the pre-migration snapshot (dummy table exists, accounts does not).
    let conn = open_sqlcipher_conn(&db_path, &dek.0, false);
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert!(tables.iter().any(|t| t == "dummy"));
    assert!(
        !tables.iter().any(|t| t == "accounts"),
        "wallet migrations should have been rolled back"
    );
}

#[test]
fn keychain_auto_unlock_does_not_satisfy_reauth() {
    let root = temp_root("wallet_keychain_reauth");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let password = "pw";

    let wallet_id = {
        let mut mgr = WalletManager::new_with_wallets_root(
            app_db_path.clone(),
            wallets_root.clone(),
            Box::new(key_store.clone()),
        )
        .expect("create wallet manager");

        mgr.create_wallet("Test Wallet", Network::Testnet, password, true, None)
            .expect("create wallet")
            .wallet
            .id
    };

    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let (_wallet, lock_status) = mgr.load_wallet(wallet_id).expect("load wallet");
    assert_eq!(lock_status, WalletLockStatus::Unlocked);

    assert!(
        mgr.reauth_wallet(wallet_id, "", ReauthPurpose::Spend)
            .is_err(),
        "reauth must require password input even when auto-unlocked"
    );
    assert!(
        mgr.reauth_wallet(wallet_id, "wrong", ReauthPurpose::Spend)
            .is_err(),
        "reauth must reject incorrect password"
    );

    let (token, _expires_at) = mgr
        .reauth_wallet(wallet_id, password, ReauthPurpose::Spend)
        .expect("reauth with correct password should succeed");
    assert!(!token.is_empty());
}
