use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use uuid::Uuid;

use zstash_core::domain::{AddressType, Network};
use zstash_core::errors;
use zstash_core::ipc::v1::commands::wallet::ReauthPurpose;
use zstash_engine::db::{
    OpenSqlcipherOptions, backup_meta, open_sqlcipher_db, wallet_encryption_meta,
};
use zstash_engine::encryption;
use zstash_engine::error::find_engine_ipc_error;
use zstash_engine::key_store::KeyStore;
use zstash_engine::reauth::SystemClock;
use zstash_engine::tx_service::{TxEventHandler, TxService};
use zstash_engine::wallet_manager::WalletManager;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

struct GrpcUrlOverrideGuard {
    prev: Option<String>,
}

impl GrpcUrlOverrideGuard {
    /// Sets `ZSTASH_GRPC_URL` for test isolation.
    /// Tests use `http://127.0.0.1:9999` - port 9999 is intentionally unreachable
    /// so network calls fail predictably without external dependencies.
    fn set(url: &str) -> Self {
        let prev = std::env::var("ZSTASH_GRPC_URL").ok();
        unsafe {
            std::env::set_var("ZSTASH_GRPC_URL", url);
        }
        Self { prev }
    }
}

impl Drop for GrpcUrlOverrideGuard {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(value) => unsafe { std::env::set_var("ZSTASH_GRPC_URL", value) },
            None => unsafe { std::env::remove_var("ZSTASH_GRPC_URL") },
        }
    }
}

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

fn shield_funds_for_test(
    mgr: &mut WalletManager,
    account_id: u32,
    consolidate: bool,
    reauth_token: &str,
    on_tx_changed: Option<TxEventHandler>,
) -> anyhow::Result<zstash_core::ipc::v1::commands::transaction::ShieldFundsResponse> {
    let tx_service = TxService::new(SystemClock);
    let task = mgr.prepare_shield_funds_task(account_id, consolidate, reauth_token, &tx_service)?;
    WalletManager::execute_prepared_shield_funds_task(task, on_tx_changed)
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

fn open_sqlcipher_conn(path: &Path, dek: &encryption::Dek) -> Connection {
    open_sqlcipher_db(
        path,
        dek,
        OpenSqlcipherOptions {
            create_if_missing: false,
            load_array_module: true,
        },
    )
    .expect("open wallet db")
}

fn consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}

fn decode_transparent_address(
    network: Network,
    encoded: &str,
) -> zcash_transparent::address::TransparentAddress {
    let params = consensus_network(network);
    let addr = zcash_client_backend::address::Address::decode(&params, encoded)
        .unwrap_or_else(|| panic!("decode transparent address: {encoded}"));

    match addr {
        zcash_client_backend::address::Address::Transparent(taddr) => taddr,
        zcash_client_backend::address::Address::Tex(taddr) => {
            zcash_transparent::address::TransparentAddress::PublicKeyHash(taddr)
        }
        other => panic!("expected transparent address, got {other:?}"),
    }
}

fn insert_transparent_utxos(
    mgr: &WalletManager,
    wallet_id: Uuid,
    network: Network,
    password: &str,
    recipient_encoded: &str,
    values_zat: impl IntoIterator<Item = u64>,
) {
    let meta = wallet_encryption_meta::get_wallet_encryption(mgr.app_db().conn(), wallet_id)
        .expect("load wallet encryption meta")
        .expect("wallet encryption meta missing");
    let dek = encryption::unwrap_dek(
        wallet_id,
        network,
        password,
        &meta.kdf,
        &meta.aead,
        &meta.wrapped_dek_b64,
    )
    .expect("unwrap DEK");

    let db_path = wallet_db_path(mgr.wallets_root(), network, wallet_id);
    assert!(db_path.exists(), "wallet db file should exist");

    let mut conn = open_sqlcipher_conn(&db_path, &dek);
    let params = consensus_network(network);

    use zcash_protocol::consensus::Parameters as _;
    let nu5_activation = params
        .activation_height(zcash_protocol::consensus::NetworkUpgrade::Nu5)
        .expect("NU5 activation height must be available");
    let tip_height =
        zcash_protocol::consensus::BlockHeight::from_u32(u32::from(nu5_activation) + 100);

    #[allow(deprecated)]
    use zcash_client_backend::data_api::WalletRead as _;
    #[allow(deprecated)]
    use zcash_client_backend::data_api::WalletWrite as _;

    let chain_tip = {
        let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
            &mut conn,
            params,
            zcash_client_sqlite::util::SystemClock,
            rand::rngs::OsRng,
        );

        wdb.update_chain_tip(tip_height).expect("update chain tip");

        wdb.chain_height()
            .expect("read chain height")
            .expect("chain height missing")
    };

    // `get_wallet_summary` returns `None` until there is at least some block data available for scan
    // progress estimation. Insert a minimal placeholder block row at the (post-NU5) chain tip; we
    // populate the numeric tree size fields so metadata parsing doesn't need the legacy Sapling tree
    // encoding.
    conn.execute(
        "INSERT OR IGNORE INTO blocks (
            height,
            hash,
            time,
            sapling_tree,
            sapling_commitment_tree_size,
            orchard_commitment_tree_size,
            sapling_output_count,
            orchard_action_count
        ) VALUES (?1, ?2, 0, x'00', 0, 0, 0, 0)",
        rusqlite::params![u32::from(chain_tip) as i64, vec![0u8; 32]],
    )
    .expect("insert placeholder block row");

    let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
        &mut conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );
    let mined_height = {
        let tip: u32 = u32::from(chain_tip);
        let mined = tip.saturating_sub(20);
        Some(zcash_protocol::consensus::BlockHeight::from_u32(mined))
    };

    let recipient = decode_transparent_address(network, recipient_encoded);

    for (idx, value) in values_zat.into_iter().enumerate() {
        let tag = (idx as u8).wrapping_add(1);
        let outpoint = zcash_transparent::bundle::OutPoint::new([tag; 32], idx as u32);
        let txout = zcash_transparent::bundle::TxOut::new(
            zcash_protocol::value::Zatoshis::from_u64(value).expect("valid zatoshis"),
            zcash_transparent::address::Script::from(&recipient.script()),
        );
        let output = zcash_client_backend::wallet::WalletTransparentOutput::from_parts(
            outpoint,
            txout,
            mined_height,
        )
        .expect("build transparent output");

        wdb.put_received_transparent_utxo(&output)
            .expect("insert transparent utxo");
    }
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
fn transparent_receive_address_and_spends_are_blocked_until_shielded() {
    let root = temp_root("us3_transparent_blocked");
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
    assert!(!transparent.encoded.trim().is_empty());

    mgr.lock_wallet(wallet.id).expect("lock wallet");
    insert_transparent_utxos(
        &mgr,
        wallet.id,
        wallet.network,
        "pw",
        &transparent.encoded,
        [100_000],
    );
    mgr.unlock_wallet_for_test(wallet.id, "pw", false)
        .expect("unlock");

    let balance = mgr.get_balance(0).expect("get balance");
    let transparent_total: u64 = balance
        .transparent_total
        .parse()
        .expect("transparent_total u64");
    assert!(transparent_total > 0);

    let shielded = testnet_shielded_address();
    let err = mgr
        .prepare_send_for_test(0, &shielded, "1", None, false)
        .expect_err("transparent funds must not be spendable via send flow");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::TRANSPARENT_SPEND_BLOCKED);
}

#[test]
fn shield_funds_sweeps_transparent_balance_and_deducts_fee() {
    let _guard = env_lock();
    let _env = GrpcUrlOverrideGuard::set("http://127.0.0.1:9999");

    let root = temp_root("us3_shield_sweep");
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

    mgr.lock_wallet(wallet.id).expect("lock wallet");
    insert_transparent_utxos(
        &mgr,
        wallet.id,
        wallet.network,
        "pw",
        &transparent.encoded,
        [250_000],
    );
    mgr.unlock_wallet_for_test(wallet.id, "pw", false)
        .expect("unlock");

    let before = mgr.get_balance(0).expect("get balance before");
    let before_total: u64 = before.total.parse().expect("before total u64");
    let before_transparent: u64 = before
        .transparent_total
        .parse()
        .expect("before transparent_total u64");
    assert_eq!(before_total, before_transparent);

    let (reauth_token, _expires_at) = mgr
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth wallet");

    let events: Arc<Mutex<Vec<zstash_core::ipc::v1::events::TransactionChangedEvent>>> =
        Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    let handler: TxEventHandler = Arc::new(move |evt| {
        events_clone.lock().expect("mutex poisoned").push(evt);
    });

    let resp = shield_funds_for_test(&mut mgr, 0, true, &reauth_token, Some(handler))
        .expect("shield funds");
    assert!(!resp.txid.trim().is_empty());

    let fee: u64 = resp.fee.parse().expect("fee u64");
    assert!(fee > 0);

    let after = mgr.get_balance(0).expect("get balance after");
    let after_total: u64 = after.total.parse().expect("after total u64");
    let after_transparent: u64 = after
        .transparent_total
        .parse()
        .expect("after transparent_total u64");
    assert_eq!(after_transparent, 0);
    assert_eq!(after_total, before_total.saturating_sub(fee));

    let emitted = events.lock().expect("mutex poisoned").clone();
    assert!(
        emitted.iter().any(|e| e.transaction.txid == resp.txid),
        "expected tx.changed event for shielding txid"
    );
}

#[test]
fn shield_funds_is_blocked_until_backup_complete() {
    let _guard = env_lock();
    let _env = GrpcUrlOverrideGuard::set("http://127.0.0.1:9999");

    let root = temp_root("us3_shield_backup_gate");
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

    let (reauth_token, _expires_at) = mgr
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth wallet");

    let err = shield_funds_for_test(&mut mgr, 0, true, &reauth_token, None)
        .expect_err("BACKUP_REQUIRED should block shield_funds");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::BACKUP_REQUIRED);
}

#[test]
fn shield_funds_insufficient_fee_includes_details() {
    let _guard = env_lock();
    let _env = GrpcUrlOverrideGuard::set("http://127.0.0.1:9999");

    let root = temp_root("us3_shield_fee_insufficient");
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

    mgr.lock_wallet(wallet.id).expect("lock wallet");
    insert_transparent_utxos(
        &mgr,
        wallet.id,
        wallet.network,
        "pw",
        &transparent.encoded,
        [6_000],
    );
    mgr.unlock_wallet_for_test(wallet.id, "pw", false)
        .expect("unlock");

    let (reauth_token, _expires_at) = mgr
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth wallet");

    let err = shield_funds_for_test(&mut mgr, 0, true, &reauth_token, None)
        .expect_err("fee should exceed available transparent balance");
    let ipc = find_engine_ipc_error(&err).expect("engine ipc error");
    assert_eq!(ipc.code, errors::INSUFFICIENT_FUNDS);

    let details = ipc.details.as_ref().expect("error details present");
    for key in [
        "required_minimum_zatoshis",
        "available_zatoshis",
        "estimated_fee_zatoshis",
    ] {
        assert!(
            details.get(key).and_then(|v| v.as_str()).is_some(),
            "expected details.{key} string"
        );
    }
}

#[test]
fn shield_funds_batches_large_input_sets() {
    let _guard = env_lock();
    let _env = GrpcUrlOverrideGuard::set("http://127.0.0.1:9999");

    let root = temp_root("us3_shield_batching");
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

    mgr.lock_wallet(wallet.id).expect("lock wallet");
    insert_transparent_utxos(
        &mgr,
        wallet.id,
        wallet.network,
        "pw",
        &transparent.encoded,
        std::iter::repeat_n(50_000u64, 201),
    );
    mgr.unlock_wallet_for_test(wallet.id, "pw", false)
        .expect("unlock");

    let (reauth_token, _expires_at) = mgr
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth wallet");
    let resp = shield_funds_for_test(&mut mgr, 0, true, &reauth_token, None).expect("shield funds");

    let txs = mgr
        .list_transactions_for_test(0, 500, 0)
        .expect("list transactions")
        .transactions;
    let shield_txs: Vec<_> = txs
        .iter()
        .filter(|t| t.tx_type == zstash_core::domain::TransactionType::Shield)
        .collect();
    assert!(
        shield_txs.len() >= 2,
        "expected >=2 Shield txs due to batching; got {} (first txid {})",
        shield_txs.len(),
        resp.txid
    );
}
