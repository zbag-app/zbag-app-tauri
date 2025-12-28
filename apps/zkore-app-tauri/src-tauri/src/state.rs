use std::path::PathBuf;
use std::sync::Mutex;

use zkore_engine::key_store_keychain::KeyStoreKeychain;
use zkore_engine::swap_service::SwapService;
use zkore_engine::sync_service::SyncService;
use zkore_engine::wallet_manager::WalletManager;

pub struct AppState {
    pub wallet_manager: Mutex<WalletManager>,
    pub sync_service: SyncService,
    pub swap_service: SwapService,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let app_db_path = default_app_db_path()?;
        let wallets_root = default_wallets_root()?;
        let key_store = Box::new(KeyStoreKeychain::new(wallets_root.clone()));
        let swap_service = SwapService::new(app_db_path.clone())?;
        let wallet_manager =
            WalletManager::new_with_wallets_root(app_db_path, wallets_root, key_store)?;
        Ok(Self {
            wallet_manager: Mutex::new(wallet_manager),
            sync_service: SyncService::new(),
            swap_service,
        })
    }
}

fn default_app_db_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".zkore").join("app.db"))
}

fn default_wallets_root() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".zkore").join("wallets"))
}
