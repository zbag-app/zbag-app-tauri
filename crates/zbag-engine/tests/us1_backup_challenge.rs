mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use zbag_core::domain::{BackupAction, Network, WalletLockStatus};
use zbag_core::errors;
use zbag_core::ipc::v1::commands::wallet::ReauthPurpose;
use zbag_core::sensitive::SensitiveString;
use zbag_engine::error::find_engine_ipc_error;
use zbag_engine::key_store::KeyStore;
use zbag_engine::wallet_manager::WalletManager;

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
    let root = std::env::temp_dir().join(format!("zbag_{prefix}_{}", Uuid::new_v4()));
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
fn create_wallet_issues_backup_challenge_and_requires_backup() {
    let root = temp_root("us1_backup_challenge");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    assert_eq!(created.seed_phrase.len(), 24);

    let challenge = created.backup_challenge;
    assert_eq!(challenge.indices.len(), 4);
    assert!(
        challenge.indices.iter().all(|i| (1..=24).contains(i)),
        "indices must be 1..=24: {:?}",
        challenge.indices
    );

    let mut distinct = challenge.indices.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 4, "indices must be distinct");

    let now_ms = chrono::Utc::now().timestamp_millis();
    assert!(challenge.expires_at > now_ms);
    assert!(challenge.expires_at <= now_ms + 10 * 60 * 1000 + 2_000);

    let status = mgr
        .compute_wallet_status(created.wallet.id)
        .expect("compute wallet status");
    assert_eq!(status.backup_status, BackupAction::Required);
}

#[test]
fn verify_backup_marks_backup_complete() {
    let root = temp_root("us1_verify_backup");
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

    let status = mgr
        .compute_wallet_status(created.wallet.id)
        .expect("compute wallet status");
    assert_eq!(status.backup_status, BackupAction::Complete);

    let err = mgr
        .verify_backup(
            created.wallet.id,
            &created.backup_challenge.challenge_id,
            &answers,
        )
        .expect_err("challenge should be invalidated after success");
    assert_ipc_err_code(&err, errors::BACKUP_CHALLENGE_INVALID);
}

#[test]
fn verify_backup_accepts_case_insensitive_words() {
    let root = temp_root("us1_verify_backup_case_insensitive");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    let mut answers =
        common::solve_backup_challenge(&created.seed_phrase, &created.backup_challenge.indices);
    let idx = *answers
        .keys()
        .next()
        .expect("challenge should have indices");
    let upper = answers
        .get(&idx)
        .expect("challenge answer exists")
        .as_ref()
        .to_ascii_uppercase();
    answers.insert(idx, upper.into());

    mgr.verify_backup(
        created.wallet.id,
        &created.backup_challenge.challenge_id,
        &answers,
    )
    .expect("verify backup (case-insensitive)");

    let status = mgr
        .compute_wallet_status(created.wallet.id)
        .expect("compute wallet status");
    assert_eq!(status.backup_status, BackupAction::Complete);
}

#[test]
fn backup_challenge_invalidates_after_five_failures_and_requires_new_challenge() {
    let root = temp_root("us1_backup_failures");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    let wrong_answers: HashMap<u8, SensitiveString> = created
        .backup_challenge
        .indices
        .iter()
        .map(|idx| (*idx, "wrong".to_string().into()))
        .collect();

    for _ in 0..4 {
        let err = mgr
            .verify_backup(
                created.wallet.id,
                &created.backup_challenge.challenge_id,
                &wrong_answers,
            )
            .expect_err("wrong answers must fail");
        assert_ipc_err_code(&err, errors::BACKUP_CHALLENGE_INVALID);
    }

    let err = mgr
        .verify_backup(
            created.wallet.id,
            &created.backup_challenge.challenge_id,
            &wrong_answers,
        )
        .expect_err("5th failure should invalidate the challenge");
    assert_ipc_err_code(&err, errors::BACKUP_CHALLENGE_TOO_MANY_ATTEMPTS);

    let new_challenge = mgr
        .get_backup_challenge(created.wallet.id)
        .expect("issue new challenge");
    assert_ne!(
        new_challenge.challenge_id,
        created.backup_challenge.challenge_id
    );

    let answers = common::solve_backup_challenge(&created.seed_phrase, &new_challenge.indices);
    mgr.verify_backup(created.wallet.id, &new_challenge.challenge_id, &answers)
        .expect("verify backup");

    let status = mgr
        .compute_wallet_status(created.wallet.id)
        .expect("compute wallet status");
    assert_eq!(status.backup_status, BackupAction::Complete);
}

#[test]
#[cfg(debug_assertions)]
fn expired_backup_challenge_returns_backup_challenge_expired() {
    let root = temp_root("us1_backup_expiry");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    let now_ms = chrono::Utc::now().timestamp_millis();
    assert!(
        mgr.__set_backup_challenge_expires_at(created.wallet.id, now_ms - 1),
        "test hook must find backup challenge"
    );

    let err = mgr
        .verify_backup(
            created.wallet.id,
            &created.backup_challenge.challenge_id,
            &HashMap::new(),
        )
        .expect_err("expired challenge must fail");
    assert_ipc_err_code(&err, errors::BACKUP_CHALLENGE_EXPIRED);

    let err = mgr
        .verify_backup(
            created.wallet.id,
            &created.backup_challenge.challenge_id,
            &HashMap::new(),
        )
        .expect_err("expired challenge should be removed");
    assert_ipc_err_code(&err, errors::BACKUP_CHALLENGE_INVALID);
}

#[test]
fn restart_invalidates_in_memory_backup_challenges() {
    let root = temp_root("us1_backup_restart");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let created = {
        let mut mgr = WalletManager::new_with_wallets_root(
            app_db_path.clone(),
            wallets_root.clone(),
            Box::new(key_store.clone()),
        )
        .expect("create wallet manager");

        mgr.create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
            .expect("create wallet")
    };

    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager (restarted)");

    let err = mgr
        .verify_backup(
            created.wallet.id,
            &created.backup_challenge.challenge_id,
            &HashMap::new(),
        )
        .expect_err("restarted manager should not accept old challenge_id");
    assert_ipc_err_code(&err, errors::BACKUP_CHALLENGE_INVALID);
}

#[test]
fn load_wallet_is_locked_then_unlocked_after_unlock_wallet() {
    let root = temp_root("us1_load_unlock");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    mgr.lock_wallet(created.wallet.id).expect("lock wallet");

    let (_wallet, lock_status) = mgr
        .load_wallet_for_test(created.wallet.id)
        .expect("load wallet");
    assert_eq!(lock_status, WalletLockStatus::Locked);

    mgr.unlock_wallet_for_test(created.wallet.id, "pw", false)
        .expect("unlock wallet");

    let (_wallet, lock_status) = mgr
        .load_wallet_for_test(created.wallet.id)
        .expect("load wallet after unlock");
    assert_eq!(lock_status, WalletLockStatus::Unlocked);

    let account_ids = mgr
        .list_wallet_db_account_ids(created.wallet.id)
        .expect("list wallet db account ids");
    assert!(
        !account_ids.is_empty(),
        "wallet should have at least one account once unlocked"
    );
}

#[test]
#[ignore]
#[cfg(not(debug_assertions))]
fn create_wallet_end_to_end_duration_is_under_sixty_seconds_in_release() {
    let root = temp_root("us1_create_wallet_timing");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let start = std::time::Instant::now();
    let _created = mgr
        .create_wallet_for_test("Timing Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");
    let elapsed = start.elapsed();
    eprintln!("CreateWallet duration (release): {elapsed:?}");
    assert!(
        elapsed.as_secs() < 60,
        "CreateWallet took too long: {elapsed:?}"
    );
}

#[test]
fn view_seed_phrase_returns_original_words() {
    // NOTE: This test validates functional correctness only; memory zeroization is verified via
    // code review (and is not practical to assert reliably in safe Rust tests).
    let root = temp_root("us1_view_seed_phrase");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let key_store = TestKeyStore::default();
    let mut mgr =
        WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
            .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet");

    let (token, _expires_at) = mgr
        .reauth_wallet(created.wallet.id, "pw", ReauthPurpose::ViewSeedPhrase)
        .expect("reauth wallet");

    let viewed = mgr
        .view_seed_phrase(created.wallet.id, &token)
        .expect("view seed phrase");

    assert_eq!(viewed.len(), 24, "seed phrase must have 24 words");
    assert_eq!(
        viewed, created.seed_phrase,
        "viewed words must match created words"
    );
}
