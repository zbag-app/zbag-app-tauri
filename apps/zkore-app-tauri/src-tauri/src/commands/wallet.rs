use std::collections::HashMap;

use anyhow::Context as _;
use tauri::State;
use tracing::warn;

use zkore_core::domain::{AccountInfo, AccountType, WalletLockStatus};
use zkore_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, GetWalletStatusRequest, GetWalletStatusResponse,
    ListWalletsRequest, ListWalletsResponse, LoadWalletRequest, LoadWalletResponse,
    LockWalletRequest, LockWalletResponse, LogoutWalletRequest, LogoutWalletResponse,
    ReauthPurpose, ReauthWalletRequest, ReauthWalletResponse, UnlockWalletRequest,
    UnlockWalletResponse, ViewSeedPhraseRequest, ViewSeedPhraseResponse,
};
use zkore_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};

use crate::state::AppState;

use super::util::{map_anyhow, system_time_to_unix_ms};

#[tauri::command(rename = "zkore_create_wallet")]
pub fn zkore_create_wallet(
    state: State<'_, AppState>,
    request: CreateWalletRequest,
) -> IpcResult<CreateWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    // Resolve gRPC URL and fetch chain tip for birthday height
    let (grpc_url, tor_manager) = {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let grpc_url =
            zkore_engine::server_resolver::resolve_grpc_url(mgr.app_db(), request.network);
        (grpc_url, Some(state.tor_manager.clone()))
    };

    // Fetch birthday height near chain tip for new wallet
    // This avoids scanning the entire blockchain for a brand new wallet
    let birthday_height = match grpc_url {
        Ok(url) => tauri::async_runtime::block_on(
            zkore_engine::wallet_manager::fetch_birthday_height_for_new_wallet(&url, tor_manager),
        ),
        Err(err) => {
            warn!(error = ?err, "failed to resolve gRPC URL for birthday fetch");
            None
        }
    };

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let created = mgr.create_wallet(
            &request.name,
            request.network,
            &request.password,
            request.remember_unlock,
            birthday_height,
        )?;

        Ok(CreateWalletResponse {
            schema_version: SCHEMA_VERSION,
            wallet: created.wallet,
            seed_phrase: created.seed_phrase,
            backup_challenge: created.backup_challenge,
        })
    })
}

#[tauri::command(rename = "zkore_list_wallets")]
pub fn zkore_list_wallets(
    state: State<'_, AppState>,
    request: ListWalletsRequest,
) -> IpcResult<ListWalletsResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let wallets = mgr.list_wallets()?;
        Ok(ListWalletsResponse {
            schema_version: SCHEMA_VERSION,
            wallets,
        })
    })
}

#[tauri::command(rename = "zkore_load_wallet")]
pub fn zkore_load_wallet(
    state: State<'_, AppState>,
    request: LoadWalletRequest,
) -> IpcResult<LoadWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        build_load_wallet_response(&mut mgr, request.wallet_id)
    })
}

#[tauri::command(rename = "zkore_unlock_wallet")]
pub fn zkore_unlock_wallet(
    state: State<'_, AppState>,
    request: UnlockWalletRequest,
) -> IpcResult<UnlockWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.unlock_wallet(
            request.wallet_id,
            &request.password,
            request.remember_unlock,
        )?;
        Ok(UnlockWalletResponse {
            schema_version: SCHEMA_VERSION,
            unlocked: status == WalletLockStatus::Unlocked,
        })
    })
}

#[tauri::command(rename = "zkore_lock_wallet")]
pub fn zkore_lock_wallet(
    state: State<'_, AppState>,
    request: LockWalletRequest,
) -> IpcResult<LockWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.lock_wallet(request.wallet_id)?;
        Ok(LockWalletResponse {
            schema_version: SCHEMA_VERSION,
            locked: status == WalletLockStatus::Locked,
        })
    })
}

#[tauri::command(rename = "zkore_reauth_wallet")]
pub fn zkore_reauth_wallet(
    state: State<'_, AppState>,
    request: ReauthWalletRequest,
) -> IpcResult<ReauthWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let (token, expires_at) =
            mgr.reauth_wallet(request.wallet_id, &request.password, request.purpose)?;
        Ok(ReauthWalletResponse {
            schema_version: SCHEMA_VERSION,
            reauth_token: token,
            expires_at: system_time_to_unix_ms(expires_at)?,
        })
    })
}

#[tauri::command(rename = "zkore_view_seed_phrase")]
pub fn zkore_view_seed_phrase(
    state: State<'_, AppState>,
    request: ViewSeedPhraseRequest,
) -> IpcResult<ViewSeedPhraseResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let seed_phrase = mgr.view_seed_phrase(request.wallet_id, &request.reauth_token)?;
        Ok(ViewSeedPhraseResponse {
            schema_version: SCHEMA_VERSION,
            seed_phrase,
        })
    })
}

#[tauri::command(rename = "zkore_get_wallet_status")]
pub fn zkore_get_wallet_status(
    state: State<'_, AppState>,
    request: GetWalletStatusRequest,
) -> IpcResult<GetWalletStatusResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");
        let status = mgr.compute_wallet_status(request.wallet_id)?;
        Ok(GetWalletStatusResponse {
            schema_version: SCHEMA_VERSION,
            status,
        })
    })
}

#[tauri::command(rename = "zkore_logout_wallet")]
pub fn zkore_logout_wallet(
    state: State<'_, AppState>,
    request: LogoutWalletRequest,
) -> IpcResult<LogoutWalletResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    map_anyhow(|| {
        // Stop sync first (best effort)
        let _ = state.sync_service.stop_sync(request.wallet_id, None);

        let mut mgr = state.wallet_manager.lock().expect("mutex poisoned");

        // Validate and consume reauth token
        mgr.consume_reauth_token(
            request.wallet_id,
            &request.reauth_token,
            ReauthPurpose::Logout,
        )?;

        // Perform logout
        mgr.logout_wallet(request.wallet_id)?;

        Ok(LogoutWalletResponse {
            schema_version: SCHEMA_VERSION,
            success: true,
        })
    })
}

fn load_accounts_for_wallet(
    mgr: &mut zkore_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<Vec<AccountInfo>> {
    let wallet_db_accounts = mgr.list_wallet_db_account_ids(wallet_id)?;
    let meta_accounts =
        zkore_engine::db::account_meta::list_accounts(mgr.app_db().conn(), wallet_id)
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
    mgr: &mut zkore_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<LoadWalletResponse> {
    let (wallet, lock_status) = mgr.load_wallet(wallet_id)?;

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

    use zkore_core::domain::Network;
    use zkore_engine::key_store::KeyStore;
    use zkore_engine::wallet_manager::WalletManager;

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
        let root = std::env::temp_dir().join(format!("zkore_{prefix}_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn load_wallet_returns_empty_accounts_when_locked_then_accounts_after_unlock() {
        let root = temp_root("us1_load_wallet_accounts");
        let app_db_path = root.join("app.db");
        let wallets_root = root.join("wallets");

        let key_store = TestKeyStore::default();
        let mut mgr =
            WalletManager::new_with_wallets_root(app_db_path, wallets_root, Box::new(key_store))
                .expect("create wallet manager");

        let created = mgr
            .create_wallet("Test Wallet", Network::Testnet, "pw", false, None)
            .expect("create wallet");
        mgr.lock_wallet(created.wallet.id).expect("lock wallet");

        let resp = build_load_wallet_response(&mut mgr, created.wallet.id).expect("load wallet");
        assert_eq!(resp.lock_status, WalletLockStatus::Locked);
        assert_eq!(resp.accounts.len(), 0);

        mgr.unlock_wallet(created.wallet.id, "pw", false)
            .expect("unlock wallet");
        let resp = build_load_wallet_response(&mut mgr, created.wallet.id)
            .expect("load wallet after unlock");
        assert_eq!(resp.lock_status, WalletLockStatus::Unlocked);
        assert!(
            !resp.accounts.is_empty(),
            "expected at least one account after unlock"
        );
    }
}
