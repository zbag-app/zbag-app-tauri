//! Optional long-running integration test for real sync cancellation behavior.
//!
//! This test is intentionally ignored by default because it requires network access
//! and an explicit lightwalletd endpoint.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use uuid::Uuid;

use zstash_core::domain::{Network, SyncPhase};
use zstash_core::errors;
use zstash_core::ipc::v1::events::SyncProgressEvent;
use zstash_engine::db::wallet_encryption_meta;
use zstash_engine::encryption;
use zstash_engine::error::find_engine_ipc_error;
use zstash_engine::key_store::KeyStore;
use zstash_engine::sync_service::SyncService;
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

type SyncEventHandler = Arc<dyn Fn(SyncProgressEvent) + Send + Sync>;

fn wait_for_scan_phase(service: &SyncService, wallet_id: Uuid, timeout: Duration) -> SyncPhase {
    let start = Instant::now();
    loop {
        let progress = service.get_progress(wallet_id);
        if matches!(progress.phase, SyncPhase::Downloading | SyncPhase::Scanning) {
            return progress.phase;
        }

        assert!(
            start.elapsed() < timeout,
            "timed out waiting for Downloading/Scanning phase, last phase: {:?}",
            progress.phase
        );

        std::thread::sleep(Duration::from_millis(200));
    }
}

fn wait_for_progress_events_to_settle(
    phases: &Arc<Mutex<Vec<SyncPhase>>>,
    settle_window: Duration,
    timeout: Duration,
) -> Vec<SyncPhase> {
    let start = Instant::now();
    let mut last_len = phases.lock().expect("mutex poisoned").len();
    let mut last_change_at = Instant::now();

    loop {
        std::thread::sleep(Duration::from_millis(100));
        let current_len = phases.lock().expect("mutex poisoned").len();
        if current_len != last_len {
            last_len = current_len;
            last_change_at = Instant::now();
        }

        if last_change_at.elapsed() >= settle_window {
            return phases.lock().expect("mutex poisoned").clone();
        }

        assert!(
            start.elapsed() < timeout,
            "timed out waiting for sync progress events to settle"
        );
    }
}

#[test]
#[ignore = "manual long-running test; requires ZSTASH_GRPC_URL testnet endpoint and network access"]
fn stop_sync_during_real_scan_workload_stops_progress_and_allows_restart() {
    let _endpoint = std::env::var("ZSTASH_GRPC_URL")
        .expect("set ZSTASH_GRPC_URL to a reachable testnet lightwalletd endpoint");

    let root = temp_root("sync_real_scan_cancellation");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");
    let password = "pw";

    let mut mgr = WalletManager::new_with_wallets_root(
        app_db_path,
        wallets_root.clone(),
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");

    let created = mgr
        .create_wallet_for_test(
            "Sync Stress Wallet",
            Network::Testnet,
            password,
            false,
            None,
        )
        .expect("create wallet");
    let wallet_id = created.wallet.id;
    let network = created.wallet.network;

    let account_ids = mgr
        .list_wallet_db_account_ids(wallet_id)
        .expect("list wallet accounts");

    let meta = wallet_encryption_meta::get_wallet_encryption(mgr.app_db().conn(), wallet_id)
        .expect("load wallet encryption metadata")
        .expect("wallet encryption metadata missing");
    let dek = encryption::unwrap_dek(
        wallet_id,
        network,
        password,
        &meta.kdf,
        &meta.aead,
        &meta.wrapped_dek_b64,
    )
    .expect("unwrap DEK");

    let wallet_db_path = wallet_db_path(&wallets_root, network, wallet_id);
    assert!(wallet_db_path.exists(), "wallet db should exist");

    let service = SyncService::new();
    let phases = Arc::new(Mutex::new(Vec::<SyncPhase>::new()));
    let phases_for_handler = Arc::clone(&phases);
    let on_progress: SyncEventHandler = Arc::new(move |event| {
        phases_for_handler
            .lock()
            .expect("mutex poisoned")
            .push(event.progress.phase);
    });

    service
        .start_sync(
            mgr.app_db(),
            wallet_id,
            network,
            wallet_db_path.clone(),
            dek.clone_key_material(),
            account_ids.clone(),
            None,
            Some(Arc::clone(&on_progress)),
            None,
        )
        .expect("start sync");

    let observed_phase = wait_for_scan_phase(&service, wallet_id, Duration::from_secs(90));
    assert!(
        matches!(observed_phase, SyncPhase::Downloading | SyncPhase::Scanning),
        "expected active scan phase, got {observed_phase:?}"
    );

    service
        .stop_sync(wallet_id, Some(Arc::clone(&on_progress)))
        .expect("stop sync");
    assert_eq!(service.get_progress(wallet_id).phase, SyncPhase::Idle);
    assert!(
        !service.running_wallet_ids().contains(&wallet_id),
        "wallet should not be running after stop"
    );

    let final_phases = wait_for_progress_events_to_settle(
        &phases,
        Duration::from_secs(1),
        Duration::from_secs(10),
    );
    assert_eq!(
        final_phases.last().copied(),
        Some(SyncPhase::Idle),
        "expected final progress phase to be Idle after stop"
    );

    let mut restarted = false;
    for _ in 0..40 {
        match service.start_sync(
            mgr.app_db(),
            wallet_id,
            network,
            wallet_db_path.clone(),
            dek.clone_key_material(),
            account_ids.clone(),
            None,
            None,
            None,
        ) {
            Ok(()) => {
                restarted = true;
                break;
            }
            Err(err) => {
                let ipc = find_engine_ipc_error(&err)
                    .unwrap_or_else(|| panic!("restart failed with non-IPC error: {err:#}"));
                assert_eq!(
                    ipc.code,
                    errors::SYNC_IN_PROGRESS,
                    "unexpected restart error while waiting for unwind: {ipc:?}"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    assert!(
        restarted,
        "sync should become restartable after cancellation unwind"
    );
    service
        .stop_sync(wallet_id, None)
        .expect("stop restarted sync");
}
