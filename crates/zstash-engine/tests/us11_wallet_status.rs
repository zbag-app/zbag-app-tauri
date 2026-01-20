use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OpenFlags};
use uuid::Uuid;
use zeroize::Zeroize;

use zstash_core::domain::{
    AddressType, Network, ShieldAction, SyncPhase, SyncProgress, SyncStatus,
};
use zstash_engine::db::{backup_meta, wallet_encryption_meta};
use zstash_engine::encryption;
use zstash_engine::key_store::KeyStore;
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

fn open_sqlcipher_conn(path: &Path, dek: &[u8; 32]) -> Connection {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .expect("open wallet db");

    let mut dek_hex = dek.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");
    conn.execute_batch(&pragma).expect("apply key");
    dek_hex.zeroize();
    pragma.zeroize();

    rusqlite::vtab::array::load_module(&conn).expect("load sqlite array module");

    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
        .expect("validate db");

    conn
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

    let mut conn = open_sqlcipher_conn(&db_path, &dek.0);
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

#[test]
fn wallet_status_tracks_backup_and_shielding_needs() {
    let root = temp_root("us11_wallet_status_basic");
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

    let status = mgr.compute_wallet_status(wallet.id).expect("wallet status");
    assert_eq!(
        status.backup_status,
        zstash_core::domain::BackupAction::Required
    );
    assert_eq!(
        status.privacy_posture,
        zstash_core::domain::PrivacyPosture::NeedsAction
    );

    backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
        .expect("disable backup gate");
    let status = mgr.compute_wallet_status(wallet.id).expect("wallet status");
    assert_eq!(
        status.backup_status,
        zstash_core::domain::BackupAction::Complete
    );
    assert_eq!(
        status.privacy_posture,
        zstash_core::domain::PrivacyPosture::Optimal
    );

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
        [10_000],
    );
    mgr.unlock_wallet(wallet.id, "pw", false).expect("unlock");

    let status = mgr.compute_wallet_status(wallet.id).expect("wallet status");
    match status.shield_status {
        ShieldAction::Available { amount } => assert_eq!(amount, "10000"),
        other => panic!("expected ShieldAction::Available, got {other:?}"),
    }
    assert_eq!(
        status.privacy_posture,
        zstash_core::domain::PrivacyPosture::NeedsAction
    );
}

#[test]
fn wallet_status_tracks_sync_progress_and_errors() {
    let root = temp_root("us11_wallet_status_sync");
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

    mgr.observe_sync_progress(
        wallet.id,
        SyncProgress {
            phase: SyncPhase::Preparing,
            scan_frontier_height: 0,
            wallet_tip_height: 0,
            progress_percent: 0,
            eta_seconds: None,
        },
    );

    let status = mgr.compute_wallet_status(wallet.id).expect("wallet status");
    assert_eq!(
        status.sync_status,
        SyncStatus::Syncing {
            progress_percent: 0
        }
    );

    mgr.observe_sync_progress(
        wallet.id,
        SyncProgress {
            phase: SyncPhase::Idle,
            scan_frontier_height: 0,
            wallet_tip_height: 0,
            progress_percent: 100,
            eta_seconds: None,
        },
    );
    let status = mgr.compute_wallet_status(wallet.id).expect("wallet status");
    assert_eq!(status.sync_status, SyncStatus::Synced);

    mgr.observe_sync_progress(
        wallet.id,
        SyncProgress {
            phase: SyncPhase::Downloading,
            scan_frontier_height: 0,
            wallet_tip_height: 0,
            progress_percent: 5,
            eta_seconds: None,
        },
    );
    mgr.observe_sync_progress(
        wallet.id,
        SyncProgress {
            phase: SyncPhase::Idle,
            scan_frontier_height: 0,
            wallet_tip_height: 0,
            progress_percent: 0,
            eta_seconds: None,
        },
    );
    let status = mgr.compute_wallet_status(wallet.id).expect("wallet status");
    assert!(matches!(status.sync_status, SyncStatus::Error { .. }));
}
