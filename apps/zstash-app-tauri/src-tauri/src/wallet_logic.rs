use std::collections::HashMap;

use anyhow::Context as _;
use tracing::warn;

use zstash_core::domain::{AccountInfo, AccountType, SyncPhase, SyncProgress, WalletLockStatus};
use zstash_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusResponse, ListWalletsResponse,
    LoadWalletResponse, LockWalletResponse, LogoutWalletResponse, ReauthWalletRequest,
    ReauthWalletResponse, UnlockWalletRequest, UnlockWalletResponse, ViewSeedPhraseRequest,
    ViewSeedPhraseResponse,
};
use zstash_core::ipc::v1::common::SCHEMA_VERSION;

use crate::state::AppState;
use crate::time_utils::system_time_to_unix_ms;

fn idle_sync_progress() -> SyncProgress {
    SyncProgress {
        phase: SyncPhase::Idle,
        scan_frontier_height: 0,
        wallet_tip_height: 0,
        progress_percent: 0,
        eta_seconds: None,
        retry_in_seconds: None,
        error_message: None,
    }
}

fn stop_sync_for_wallet(
    state: &AppState,
    mgr: &mut zstash_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) {
    mgr.observe_sync_stop_requested(wallet_id);
    let _ = state.sync_service.stop_sync(wallet_id, None);
    mgr.observe_sync_progress(wallet_id, idle_sync_progress());
}

fn stop_previous_wallet_sync(
    state: &AppState,
    mgr: &mut zstash_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) {
    if let Some(prev_wallet_id) = mgr.active_wallet_info().map(|w| w.id)
        && prev_wallet_id != wallet_id
    {
        stop_sync_for_wallet(state, mgr, prev_wallet_id);
    }
}

pub fn list_wallets(state: &AppState) -> anyhow::Result<ListWalletsResponse> {
    let mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let wallets = mgr.list_wallets()?;
    Ok(ListWalletsResponse {
        schema_version: SCHEMA_VERSION,
        wallets,
    })
}

pub fn create_wallet(
    state: &AppState,
    request: CreateWalletRequest,
    birthday_height: Option<u32>,
) -> anyhow::Result<CreateWalletResponse> {
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
    let created = mgr.create_wallet(
        &request.name,
        request.network,
        &request.password,
        request.remember_unlock,
        birthday_height,
        &mut tx_svc,
    )?;

    Ok(CreateWalletResponse {
        schema_version: SCHEMA_VERSION,
        wallet: created.wallet,
        seed_phrase: created.seed_phrase,
        backup_challenge: created.backup_challenge,
    })
}

pub fn load_wallet(state: &AppState, wallet_id: uuid::Uuid) -> anyhow::Result<LoadWalletResponse> {
    // Loads the wallet state but does not auto-start sync; callers can decide
    // whether to start sync based on their execution context.
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
    stop_previous_wallet_sync(state, &mut mgr, wallet_id);
    build_load_wallet_response(&mut mgr, wallet_id, &mut tx_svc)
}

pub fn get_wallet_status(
    state: &AppState,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<GetWalletStatusResponse> {
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let status = mgr.compute_wallet_status(wallet_id)?;
    Ok(GetWalletStatusResponse {
        schema_version: SCHEMA_VERSION,
        status,
    })
}

pub fn unlock_wallet(
    state: &AppState,
    request: UnlockWalletRequest,
) -> anyhow::Result<UnlockWalletResponse> {
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
    let status = mgr.unlock_wallet(
        request.wallet_id,
        &request.password,
        request.remember_unlock,
        &mut tx_svc,
    )?;
    Ok(UnlockWalletResponse {
        schema_version: SCHEMA_VERSION,
        unlocked: status == WalletLockStatus::Unlocked,
    })
}

pub fn lock_wallet(state: &AppState, wallet_id: uuid::Uuid) -> anyhow::Result<LockWalletResponse> {
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    stop_sync_for_wallet(state, &mut mgr, wallet_id);
    let status = mgr.lock_wallet(wallet_id)?;
    Ok(LockWalletResponse {
        schema_version: SCHEMA_VERSION,
        locked: status == WalletLockStatus::Locked,
    })
}

pub fn logout_wallet(
    state: &AppState,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<LogoutWalletResponse> {
    let _ = state.sync_service.stop_sync(wallet_id, None);
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let mut tx_svc = state.tx_service.lock().expect("mutex poisoned");
    mgr.logout_wallet(wallet_id, &mut tx_svc)?;
    Ok(LogoutWalletResponse {
        schema_version: SCHEMA_VERSION,
        success: true,
    })
}

pub fn reauth_wallet(
    state: &AppState,
    request: ReauthWalletRequest,
) -> anyhow::Result<ReauthWalletResponse> {
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let (token, expires_at) =
        mgr.reauth_wallet(request.wallet_id, &request.password, request.purpose)?;
    Ok(ReauthWalletResponse {
        schema_version: SCHEMA_VERSION,
        reauth_token: token,
        expires_at: system_time_to_unix_ms(expires_at)?,
    })
}

pub fn view_seed_phrase(
    state: &AppState,
    request: ViewSeedPhraseRequest,
) -> anyhow::Result<ViewSeedPhraseResponse> {
    let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
    let seed_phrase = mgr.view_seed_phrase(request.wallet_id, &request.reauth_token)?;
    Ok(ViewSeedPhraseResponse {
        schema_version: SCHEMA_VERSION,
        seed_phrase,
    })
}

fn load_accounts_for_wallet(
    mgr: &mut zstash_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<Vec<AccountInfo>> {
    let wallet_db_accounts = mgr.list_wallet_db_account_ids(wallet_id)?;
    let meta_accounts =
        zstash_engine::db::account_meta::list_accounts(mgr.app_db().conn(), wallet_id)
            .map_err(|e| anyhow::anyhow!(e))
            .context("failed to load account metadata")?;

    let meta_by_id: HashMap<u32, AccountInfo> =
        meta_accounts.into_iter().map(|a| (a.id, a)).collect();

    let mut out = Vec::with_capacity(wallet_db_accounts.len());
    for account_id in wallet_db_accounts {
        if let Some(meta) = meta_by_id.get(&account_id) {
            out.push(meta.clone());
            continue;
        }

        warn!(account_id, "Account metadata missing; applying defaults");
        out.push(AccountInfo {
            id: account_id,
            name: format!("Account {}", account_id + 1),
            account_type: if account_id == 0 {
                AccountType::Software
            } else {
                AccountType::HardwareSigner
            },
        });
    }

    Ok(out)
}

fn build_load_wallet_response(
    mgr: &mut zstash_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
    tx_service: &mut zstash_engine::tx_service::TxService<zstash_engine::reauth::SystemClock>,
) -> anyhow::Result<LoadWalletResponse> {
    let (wallet, lock_status) = mgr.load_wallet(wallet_id, tx_service)?;

    let accounts = if lock_status == WalletLockStatus::Locked {
        vec![]
    } else {
        load_accounts_for_wallet(mgr, wallet.id)?
    };

    Ok(LoadWalletResponse {
        schema_version: SCHEMA_VERSION,
        wallet,
        lock_status,
        accounts,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use uuid::Uuid;

    use zstash_core::domain::{Network, WalletLockStatus};
    use zstash_engine::key_store::KeyStore;
    use zstash_engine::wallet_manager::WalletManager;

    use super::*;

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

        fn delete_encrypted_mnemonic(
            &self,
            wallet_id: Uuid,
            network: Network,
        ) -> anyhow::Result<()> {
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

    #[test]
    fn load_wallet_returns_empty_accounts_when_locked_then_accounts_after_unlock() {
        use zstash_engine::reauth::SystemClock;
        use zstash_engine::tx_service::TxService;

        let root = temp_root("us1_load_wallet_accounts");
        let app_db_path = root.join("app.db");
        let wallets_root = root.join("wallets");

        let key_store = TestKeyStore::default();
        let mut mgr =
            WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
                .expect("create wallet manager");
        let mut tx_service = TxService::new(SystemClock);

        let created = mgr
            .create_wallet(
                "Test Wallet",
                Network::Testnet,
                "pw",
                false,
                None,
                &mut tx_service,
            )
            .expect("create wallet");
        mgr.lock_wallet(created.wallet.id).expect("lock wallet");

        let resp = super::build_load_wallet_response(&mut mgr, created.wallet.id, &mut tx_service)
            .expect("load wallet");
        assert_eq!(resp.lock_status, WalletLockStatus::Locked);
        assert_eq!(resp.accounts.len(), 0);

        mgr.unlock_wallet(created.wallet.id, "pw", false, &mut tx_service)
            .expect("unlock wallet");
        let resp = super::build_load_wallet_response(&mut mgr, created.wallet.id, &mut tx_service)
            .expect("load wallet after unlock");
        assert_eq!(resp.lock_status, WalletLockStatus::Unlocked);
        assert!(
            !resp.accounts.is_empty(),
            "expected at least one account after unlock"
        );
    }
}
