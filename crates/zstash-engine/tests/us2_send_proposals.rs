use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use zstash_core::domain::{AddressType, Network};
use zstash_core::errors;
use zstash_core::ipc::v1::commands::wallet::ReauthPurpose;
use zstash_engine::db::{backup_meta, wallet_meta};
use zstash_engine::error::find_engine_ipc_error;
use zstash_engine::key_store::KeyStore;
use zstash_engine::reauth::Clock;
use zstash_engine::tx_service::TxService;
use zstash_engine::wallet_manager::WalletManager;

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
    let root = std::env::temp_dir().join(format!("zstash_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn testnet_shielded_address() -> String {
    let params = zcash_protocol::consensus::Network::TestNetwork;
    let seed = [0u8; 32];
    let usk = zcash_client_backend::keys::UnifiedSpendingKey::from_seed(
        &params,
        &seed,
        zip32::AccountId::ZERO,
    )
    .expect("derive usk");
    let ufvk = usk.to_unified_full_viewing_key();
    let (ua, _) = ufvk
        .default_address(zcash_client_backend::keys::UnifiedAddressRequest::SHIELDED)
        .expect("derive ua");
    zcash_client_backend::address::Address::Unified(ua).encode(&params)
}

#[test]
fn prepare_send_is_blocked_until_backup_complete() {
    let root = temp_root("us2_backup_gate");
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
        .prepare_send(0, "not-an-address", "1", None, false)
        .expect_err("BACKUP_REQUIRED should block prepare_send");

    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::BACKUP_REQUIRED);

    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("set backup required=false");

    let err = mgr
        .prepare_send(0, "not-an-address", "1", None, false)
        .expect_err("after backup, invalid recipient should be surfaced");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::INVALID_RECIPIENT);
}

#[test]
fn prepare_send_enforces_privacy_ack_and_memo_rules_for_transparent_recipients() {
    let root = temp_root("us2_transparent_rules");
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
    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("disable backup gate");

    let transparent = mgr
        .get_receive_address(0, AddressType::Transparent)
        .expect("get transparent address");

    let err = mgr
        .prepare_send(0, &transparent.encoded, "1", None, false)
        .expect_err("transparent recipient should require privacy ack");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::PRIVACY_ACK_REQUIRED);

    let err = mgr
        .prepare_send(0, &transparent.encoded, "1", Some("hi"), true)
        .expect_err("transparent recipient must reject memos");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::MEMO_NOT_ALLOWED);
}

#[test]
fn prepare_send_rejects_memo_over_512_bytes() {
    let root = temp_root("us2_memo_too_long");
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
    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("disable backup gate");

    let shielded = testnet_shielded_address();

    let memo = "a".repeat(513);
    let err = mgr
        .prepare_send(0, &shielded, "1", Some(&memo), false)
        .expect_err("memo too long should be rejected");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::MEMO_TOO_LONG);
}

#[derive(Debug, Clone)]
struct TestClock(Arc<Mutex<SystemTime>>);

impl Clock for TestClock {
    fn now(&self) -> SystemTime {
        *self.0.lock().expect("mutex poisoned")
    }
}

impl TestClock {
    fn new(now: SystemTime) -> Self {
        Self(Arc::new(Mutex::new(now)))
    }
}

fn to_unix_ms(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[test]
fn queued_broadcasts_are_cleaned_up_after_seven_days() {
    let root = temp_root("us2_queue_retention");
    let wallet_dir = root.join("wallet");
    let queue_dir = wallet_dir.join("queued_broadcasts");
    std::fs::create_dir_all(&queue_dir).expect("create queued_broadcasts dir");

    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(8 * 24 * 60 * 60);
    let clock = TestClock::new(now);
    let mut service = TxService::new(clock);
    let wallet_id = Uuid::new_v4();

    let txid_old = "oldtx";
    let old_created_at = now - Duration::from_secs(7 * 24 * 60 * 60 + 1);
    std::fs::write(queue_dir.join(format!("{txid_old}.bin")), b"test").expect("write bin");
    std::fs::write(
        queue_dir.join(format!("{txid_old}.json")),
        serde_json::to_vec(&serde_json::json!({
            "created_at_ms": to_unix_ms(old_created_at),
            "last_error": "failed"
        }))
        .unwrap(),
    )
    .expect("write meta");

    service
        .scan_queued_broadcasts(wallet_id, &wallet_dir)
        .expect("scan should succeed");

    assert!(
        !queue_dir.join(format!("{txid_old}.bin")).exists(),
        "old queued tx bytes should be deleted"
    );
    assert!(
        !queue_dir.join(format!("{txid_old}.json")).exists(),
        "old queued tx metadata should be deleted"
    );

    let txid_new = "newtx";
    std::fs::write(queue_dir.join(format!("{txid_new}.bin")), b"test").expect("write bin");
    std::fs::write(
        queue_dir.join(format!("{txid_new}.json")),
        serde_json::to_vec(&serde_json::json!({
            "created_at_ms": to_unix_ms(now - Duration::from_secs(60)),
            "last_error": null
        }))
        .unwrap(),
    )
    .expect("write meta");

    service
        .scan_queued_broadcasts(wallet_id, &wallet_dir)
        .expect("scan should succeed");

    assert!(
        queue_dir.join(format!("{txid_new}.bin")).exists(),
        "fresh queued tx bytes should be retained"
    );
    assert!(
        queue_dir.join(format!("{txid_new}.json")).exists(),
        "fresh queued tx metadata should be retained"
    );
}

#[test]
fn queued_broadcasts_with_missing_bin_are_dropped() {
    let root = temp_root("us2_queue_missing_bin");
    let wallet_dir = root.join("wallet");
    let queue_dir = wallet_dir.join("queued_broadcasts");
    std::fs::create_dir_all(&queue_dir).expect("create queued_broadcasts dir");

    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
    let clock = TestClock::new(now);
    let mut service = TxService::new(clock);
    let wallet_id = Uuid::new_v4();

    let txid = "missingbin";
    std::fs::write(
        queue_dir.join(format!("{txid}.json")),
        serde_json::to_vec(&serde_json::json!({
            "created_at_ms": to_unix_ms(now),
            "last_error": "oops"
        }))
        .unwrap(),
    )
    .expect("write meta");

    service
        .scan_queued_broadcasts(wallet_id, &wallet_dir)
        .expect("scan should succeed");

    assert!(
        !queue_dir.join(format!("{txid}.json")).exists(),
        "orphaned metadata should be deleted"
    );
}

#[test]
fn queued_broadcasts_with_invalid_metadata_are_dropped() {
    let root = temp_root("us2_queue_invalid_meta");
    let wallet_dir = root.join("wallet");
    let queue_dir = wallet_dir.join("queued_broadcasts");
    std::fs::create_dir_all(&queue_dir).expect("create queued_broadcasts dir");

    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
    let clock = TestClock::new(now);
    let mut service = TxService::new(clock);
    let wallet_id = Uuid::new_v4();

    let txid = "invalidmeta";
    std::fs::write(queue_dir.join(format!("{txid}.bin")), b"test").expect("write bin");
    std::fs::write(queue_dir.join(format!("{txid}.json")), b"{not json")
        .expect("write invalid json");

    service
        .scan_queued_broadcasts(wallet_id, &wallet_dir)
        .expect("scan should succeed");

    assert!(
        !queue_dir.join(format!("{txid}.json")).exists(),
        "invalid metadata should be deleted"
    );
}

#[test]
fn prepared_retry_task_revalidates_wallet_unlocked_state_before_lock_free_execution() {
    let root = temp_root("us2_retry_revalidation");
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

    let (reauth_token, _expires_at) = mgr
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth wallet");
    let task = mgr
        .prepare_retry_broadcast_task(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &reauth_token,
        )
        .expect("prepare retry task");

    mgr.lock_wallet(wallet.id).expect("lock wallet");

    let err = mgr
        .validate_retry_broadcast_task(&task)
        .expect_err("locked wallet must block prepared retry task execution");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::WALLET_LOCKED);
}

#[test]
fn prepared_retry_task_can_execute_without_wallet_manager_guard() {
    let root = temp_root("us2_retry_lock_free_execute");
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

    let (reauth_token, _expires_at) = mgr
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth wallet");
    let task = mgr
        .prepare_retry_broadcast_task(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &reauth_token,
        )
        .expect("prepare retry task");

    let err = WalletManager::execute_prepared_retry_broadcast_task(task, None, None)
        .expect_err("retry should fail because tx is not queued");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::QUEUED_BROADCAST_NOT_FOUND);
}

#[test]
fn process_queued_broadcast_retries_returns_zero_when_retry_attempt_fails() {
    let root = temp_root("us2_retry_failure_count");
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

    let (_wallet, wallet_dir_str) = wallet_meta::get_wallet(mgr.app_db().conn(), wallet.id)
        .expect("load wallet metadata")
        .expect("wallet exists");
    let wallet_dir = PathBuf::from(wallet_dir_str);
    let queue_dir = wallet_dir.join("queued_broadcasts");
    std::fs::create_dir_all(&queue_dir).expect("create queued_broadcasts dir");

    let txid = "invalid-txid";
    std::fs::write(queue_dir.join(format!("{txid}.bin")), b"queued-bytes")
        .expect("write queued tx bytes");
    std::fs::write(
        queue_dir.join(format!("{txid}.json")),
        serde_json::to_vec(&serde_json::json!({
            "created_at_ms": to_unix_ms(SystemTime::now()),
            "last_error": "seed queued retry"
        }))
        .expect("serialize queued metadata"),
    )
    .expect("write queued metadata");

    let processed = mgr
        .process_queued_broadcast_retries(None, None)
        .expect("process queued broadcast retries");

    assert_eq!(
        processed, 0,
        "failed retry attempts should not be counted as successful processing"
    );
}
