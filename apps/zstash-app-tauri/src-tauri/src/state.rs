use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use zstash_engine::key_store_keychain::KeyStoreKeychain;
use zstash_engine::logging::LoggingGuard;
use zstash_engine::swap_service::SwapService;
use zstash_engine::sync_service::SyncService;
use zstash_engine::wallet_manager::WalletManager;

pub struct AppState {
    pub wallet_manager: Arc<Mutex<WalletManager>>,
    pub sync_service: SyncService,
    pub swap_service: SwapService,
    pub tor_manager: Arc<zstash_tor::TorManager>,
    pub logging_guard: Mutex<LoggingGuard>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let logging_guard = zstash_engine::logging::init_logging()?;
        let app_db_path = default_app_db_path()?;
        let wallets_root = default_wallets_root()?;
        let key_store = Box::new(KeyStoreKeychain::new(wallets_root.clone()));
        let mut wallet_manager =
            WalletManager::new_with_wallets_root(app_db_path.clone(), wallets_root, key_store)?;

        let tor_state = zstash_engine::db::tor_meta::get_tor_state(wallet_manager.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?;

        let tor_dir = default_tor_dir()?;
        let tor_manager = Arc::new(zstash_tor::TorManager::new(
            zstash_tor::TorManagerConfig::new(tor_dir),
            tor_state,
        ));

        wallet_manager.set_tor_manager(Arc::clone(&tor_manager));

        let wallet_manager = Arc::new(Mutex::new(wallet_manager));
        let near = zstash_network::near_intents::NearIntentsClient::new_with_tor(Arc::clone(
            &tor_manager,
        ))?;
        let swap_service =
            SwapService::new_with_near_client(app_db_path, Arc::clone(&wallet_manager), near)?;

        Ok(Self {
            wallet_manager,
            sync_service: SyncService::new(),
            swap_service,
            tor_manager,
            logging_guard: Mutex::new(logging_guard),
        })
    }
}

fn default_app_db_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".zstash").join("app.db"))
}

fn default_wallets_root() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".zstash").join("wallets"))
}

fn default_tor_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".zstash").join("tor"))
}
