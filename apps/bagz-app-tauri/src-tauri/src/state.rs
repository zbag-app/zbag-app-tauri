use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use bagz_engine::key_store_keychain::KeyStoreKeychain;
use bagz_engine::logging::LoggingGuard;
use bagz_engine::reauth::SystemClock;
use bagz_engine::swap_service::SwapService;
use bagz_engine::sync_service::SyncService;
use bagz_engine::tx_service::TxService;
use bagz_engine::wallet_manager::WalletManager;
use bagz_network::exchange_rate::ExchangeRateService;
use bagz_network::near_intents::NearIntentsClient;

pub struct AppState {
    /// Global lock-order rule: if both mutexes are needed, always acquire
    /// `wallet_manager` first and `tx_service` second.
    pub wallet_manager: Arc<Mutex<WalletManager>>,
    /// TxService is separate from WalletManager to allow releasing the wallet_manager
    /// mutex during expensive proving/signing/broadcast operations.
    /// Must be locked only after `wallet_manager` when both are required.
    pub tx_service: Arc<Mutex<TxService<SystemClock>>>,
    pub sync_service: SyncService,
    pub swap_service: SwapService,
    pub tor_manager: Arc<bagz_tor::TorManager>,
    pub exchange_rate_service: ExchangeRateService,
    pub near_client: NearIntentsClient,
    pub logging_guard: Mutex<LoggingGuard>,
}

impl AppState {
    pub fn lock_wallet_then_tx_service(
        &self,
    ) -> (
        MutexGuard<'_, WalletManager>,
        MutexGuard<'_, TxService<SystemClock>>,
    ) {
        let wallet_manager = self.wallet_manager.lock().expect("mutex poisoned");
        let tx_service = self.tx_service.lock().expect("mutex poisoned");
        (wallet_manager, tx_service)
    }

    pub fn new() -> anyhow::Result<Self> {
        let logging_guard = bagz_engine::logging::init_logging()?;
        let app_db_path = default_app_db_path()?;
        let wallets_root = default_wallets_root()?;
        let key_store = Box::new(KeyStoreKeychain::new(wallets_root.clone()));
        let wallet_manager =
            WalletManager::new_with_wallets_root(app_db_path.clone(), wallets_root, key_store)?;

        let tor_state = bagz_engine::db::tor_meta::get_tor_state(wallet_manager.app_db().conn())
            .map_err(|e| anyhow::anyhow!(e))?;

        let tor_dir = default_tor_dir()?;
        let tor_manager = Arc::new(bagz_tor::TorManager::new(
            bagz_tor::TorManagerConfig::new(tor_dir),
            tor_state,
        ));

        // Create TxService separately - it will be used outside the wallet_manager mutex
        // for expensive proving/signing/broadcast operations.
        let mut tx_service = TxService::new(SystemClock);
        tx_service.set_tor_manager(Arc::clone(&tor_manager));
        let tx_service = Arc::new(Mutex::new(tx_service));

        let wallet_manager = Arc::new(Mutex::new(wallet_manager));
        let near_client = NearIntentsClient::new_with_tor(Arc::clone(&tor_manager))?;
        let swap_service = SwapService::new_with_near_client_and_tx(
            app_db_path,
            Arc::clone(&wallet_manager),
            Arc::clone(&tx_service),
            near_client.clone(),
        )?;
        let exchange_rate_service = ExchangeRateService::new_with_tor(Arc::clone(&tor_manager))?;

        Ok(Self {
            wallet_manager,
            tx_service,
            sync_service: SyncService::new(),
            swap_service,
            tor_manager,
            exchange_rate_service,
            near_client,
            logging_guard: Mutex::new(logging_guard),
        })
    }
}

fn default_app_db_path() -> anyhow::Result<PathBuf> {
    let root = bagz_data_root()?;
    Ok(root.join("app.db"))
}

fn default_wallets_root() -> anyhow::Result<PathBuf> {
    let root = bagz_data_root()?;
    Ok(root.join("wallets"))
}

fn default_tor_dir() -> anyhow::Result<PathBuf> {
    let root = bagz_data_root()?;
    Ok(root.join("tor"))
}

fn bagz_data_root() -> anyhow::Result<PathBuf> {
    #[cfg(feature = "test-bridge")]
    {
        match std::env::var("BAGZ_TEST_HOME") {
            Ok(root) => {
                let trimmed = root.trim();
                if trimmed.is_empty() {
                    return Err(anyhow::anyhow!(
                        "BAGZ_TEST_HOME is set but empty or whitespace; test-bridge requires a non-empty path"
                    ));
                }
                return Ok(PathBuf::from(trimmed));
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "BAGZ_TEST_HOME must be set to a non-empty path when test-bridge is enabled"
                ));
            }
        }
    }

    #[cfg(not(feature = "test-bridge"))]
    {
        let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
        Ok(PathBuf::from(home).join(".bagz"))
    }
}
