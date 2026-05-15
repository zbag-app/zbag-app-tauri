use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use bagz_core::domain::{AddressType, Network};
use bagz_core::errors;
use bagz_core::ipc::v1::commands::wallet::ReauthPurpose;
use bagz_engine::db::{backup_meta, wallet_meta};
use bagz_engine::error::find_engine_ipc_error;
use bagz_engine::key_store::KeyStore;
use bagz_engine::tx_service::TxService;
use bagz_engine::wallet_manager::WalletManager;

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
    let root = std::env::temp_dir().join(format!("bagz_{prefix}_{}", Uuid::new_v4()));
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
fn build_signing_request_is_blocked_until_backup_complete() {
    let root = temp_root("us7_backup_gate");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let wallet = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    let err = mgr
        .build_signing_request_for_test(0, "not-an-address", "1", None, false)
        .expect_err("BACKUP_REQUIRED should block build_signing_request");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::BACKUP_REQUIRED);

    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("set backup required=false");

    let err = mgr
        .build_signing_request_for_test(0, "not-an-address", "1", None, false)
        .expect_err("after backup, invalid recipient should be surfaced");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::INVALID_RECIPIENT);
}

#[test]
fn build_signing_request_enforces_privacy_ack_and_memo_rules_for_transparent_recipients() {
    let root = temp_root("us7_transparent_rules");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let wallet = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet")
        .wallet;
    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("disable backup gate");

    let transparent = mgr
        .get_receive_address(0, AddressType::Transparent)
        .expect("get transparent address");

    let err = mgr
        .build_signing_request_for_test(0, &transparent.encoded, "1", None, false)
        .expect_err("transparent recipient should require privacy ack");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::PRIVACY_ACK_REQUIRED);

    let err = mgr
        .build_signing_request_for_test(0, &transparent.encoded, "1", Some("hi"), true)
        .expect_err("transparent recipient must reject memos");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::MEMO_NOT_ALLOWED);
}

#[test]
fn build_signing_request_rejects_memo_over_512_bytes() {
    let root = temp_root("us7_memo_too_long");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let wallet = mgr
        .create_wallet_for_test("Test Wallet", Network::Testnet, "pw", false, None)
        .expect("create wallet")
        .wallet;
    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("disable backup gate");

    let shielded = testnet_shielded_address();
    let memo = "a".repeat(513);
    let err = mgr
        .build_signing_request_for_test(0, &shielded, "1", Some(&memo), false)
        .expect_err("memo too long should be rejected");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::MEMO_TOO_LONG);
}

#[test]
fn prepare_finalize_signing_task_keeps_request_when_wallet_db_preflight_fails() {
    let root = temp_root("us7_finalize_preflight");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mut tx_service = TxService::new(bagz_engine::reauth::SystemClock);

    let wallet = mgr
        .create_wallet(
            "Test Wallet",
            Network::Testnet,
            "pw",
            false,
            None,
            &mut tx_service,
        )
        .expect("create wallet")
        .wallet;
    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("disable backup gate");

    let signing_request_id = Uuid::new_v4().to_string();
    tx_service.import_pending_signing_request_for_execution(
        signing_request_id.clone(),
        wallet.id,
        "pczt-test".to_string(),
        SystemTime::now() + Duration::from_secs(300),
    );
    let (reauth_token, _) = mgr
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth wallet");

    let (_wallet, wallet_dir_str) = wallet_meta::get_wallet(mgr.app_db().conn(), wallet.id)
        .expect("load wallet metadata")
        .expect("wallet exists");
    let wallet_db_path = PathBuf::from(wallet_dir_str).join("wallet.sqlite");
    std::fs::remove_file(&wallet_db_path).expect("remove wallet db before preflight");

    let prep =
        mgr.prepare_finalize_signing_task(&signing_request_id, "", &reauth_token, &mut tx_service);
    assert!(prep.is_err(), "wallet db preflight should fail");
    let err = prep.err().expect("expected preflight failure");
    assert!(
        err.to_string().contains("finalize signing preflight"),
        "expected finalize signing preflight context: {err:#}"
    );

    let (_pczt, _expires_at) = tx_service
        .take_pending_signing_request_for_execution(&signing_request_id, wallet.id)
        .expect("signing request should remain available after preflight failure");
}
