use std::path::PathBuf;

use uuid::Uuid;

use zkore_core::domain::{Network, WalletInfo, WalletLockStatus, WalletType};

use crate::db::AppDb;
use crate::key_store::KeyStore;

pub struct WalletManager {
    app_db: AppDb,
    key_store: Box<dyn KeyStore>,
}

impl WalletManager {
    pub fn new(app_db_path: PathBuf, key_store: Box<dyn KeyStore>) -> anyhow::Result<Self> {
        Ok(Self {
            app_db: AppDb::open(app_db_path)?,
            key_store,
        })
    }

    pub fn list_wallets(&self) -> anyhow::Result<Vec<WalletInfo>> {
        let wallets = crate::db::wallet_meta::list_wallets(self.app_db.conn())?;
        Ok(wallets)
    }

    pub fn load_wallet(&self, _wallet_id: Uuid) -> anyhow::Result<(WalletInfo, WalletLockStatus)> {
        // Filled in by US1.
        anyhow::bail!("not implemented")
    }

    pub fn create_wallet(&self, _name: &str, _network: Network) -> anyhow::Result<WalletInfo> {
        // Filled in by US1.
        let now_ms = chrono::Utc::now().timestamp_millis();
        Ok(WalletInfo {
            id: Uuid::new_v4(),
            name: "placeholder".to_string(),
            wallet_type: WalletType::Software,
            network: Network::Testnet,
            remember_unlock_enabled: false,
            created_at: now_ms,
            last_opened_at: None,
        })
    }

    pub fn key_store(&self) -> &dyn KeyStore {
        self.key_store.as_ref()
    }

    pub fn app_db(&self) -> &AppDb {
        &self.app_db
    }
}
