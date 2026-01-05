//! CLI application state, adapted from Tauri's AppState.
//!
//! Key differences from Tauri:
//! - Uses FileKeyStore instead of KeyStoreKeychain
//! - TorManager is created only if --tor flag is passed
//! - No event handlers (uses callbacks or polling instead)

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use uuid::Uuid;

use zkore_core::domain::{Network, WalletInfo, WalletLockStatus};
use zkore_engine::sync_service::SyncService;
use zkore_engine::wallet_manager::WalletManager;

use crate::file_key_store::FileKeyStore;

/// CLI application state.
pub struct CliAppState {
    pub wallet_manager: Arc<Mutex<WalletManager>>,
    pub sync_service: SyncService,
    pub tor_manager: Option<Arc<zkore_tor::TorManager>>,
    #[allow(dead_code)]
    data_dir: PathBuf,
}

impl CliAppState {
    /// Create new CLI app state.
    ///
    /// If `enable_tor` is true, Tor will be initialized (may take time to bootstrap).
    pub fn new(data_dir: &Path, enable_tor: bool) -> Result<Self> {
        let app_db_path = data_dir.join("app.db");
        let wallets_root = data_dir.join("wallets");

        // Ensure directories exist
        std::fs::create_dir_all(&wallets_root)?;

        // Create file-based key store
        let key_store = Box::new(FileKeyStore::new(data_dir));

        // Initialize WalletManager
        let mut wallet_manager =
            WalletManager::new_with_wallets_root(app_db_path, wallets_root, key_store)?;

        // Initialize TorManager if requested
        let tor_manager = if enable_tor {
            let tor_dir = data_dir.join("tor");
            let tor_state =
                zkore_engine::db::tor_meta::get_tor_state(wallet_manager.app_db().conn())
                    .map_err(|e| anyhow::anyhow!(e))?;

            let manager = Arc::new(zkore_tor::TorManager::new(
                zkore_tor::TorManagerConfig::new(tor_dir),
                tor_state,
            ));
            wallet_manager.set_tor_manager(Arc::clone(&manager));
            Some(manager)
        } else {
            None
        };

        let wallet_manager = Arc::new(Mutex::new(wallet_manager));

        Ok(Self {
            wallet_manager,
            sync_service: SyncService::new(),
            tor_manager,
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Get the data directory path.
    #[allow(dead_code)]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// List all wallets.
    pub fn list_wallets(&self) -> Result<Vec<WalletInfo>> {
        let wm = self.wallet_manager.lock().expect("mutex poisoned");
        wm.list_wallets()
    }

    /// Load a wallet by ID.
    pub fn load_wallet(&self, wallet_id: Uuid) -> Result<(WalletInfo, bool)> {
        let mut wm = self.wallet_manager.lock().expect("mutex poisoned");
        let (info, lock_status) = wm.load_wallet(wallet_id)?;
        let unlocked = lock_status == WalletLockStatus::Unlocked;
        Ok((info, unlocked))
    }

    /// Unlock a wallet with password.
    pub fn unlock_wallet(&self, wallet_id: Uuid, password: &str, remember: bool) -> Result<()> {
        let mut wm = self.wallet_manager.lock().expect("mutex poisoned");
        wm.unlock_wallet(wallet_id, password, remember)?;
        Ok(())
    }

    /// Lock a wallet.
    pub fn lock_wallet(&self, wallet_id: Uuid) -> Result<()> {
        let mut wm = self.wallet_manager.lock().expect("mutex poisoned");
        wm.lock_wallet(wallet_id)?;
        Ok(())
    }

    /// Find wallet by ID prefix (short ID matching).
    ///
    /// Returns an error if the prefix is ambiguous (matches multiple wallets).
    pub fn find_wallet_by_prefix(&self, prefix: &str) -> Result<Option<WalletInfo>> {
        let wallets = self.list_wallets()?;
        let prefix_lower = prefix.to_lowercase();

        let matches: Vec<_> = wallets
            .into_iter()
            .filter(|w| w.id.to_string().to_lowercase().starts_with(&prefix_lower))
            .collect();

        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches.into_iter().next().unwrap())),
            n => anyhow::bail!(
                "ambiguous wallet ID prefix '{}' matches {} wallets",
                prefix,
                n
            ),
        }
    }

    /// Get wallet by ID prefix, returning an error if not found.
    pub fn get_wallet_by_prefix(&self, prefix: &str) -> Result<WalletInfo> {
        self.find_wallet_by_prefix(prefix)?
            .ok_or_else(|| anyhow::anyhow!("wallet not found: {}", prefix))
    }
}

/// Helper to get the network directory name.
pub fn network_dir_name(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
    }
}
