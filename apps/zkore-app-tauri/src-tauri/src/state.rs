use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use uuid::Uuid;

use zkore_core::domain::Network;
use zkore_engine::key_store::KeyStore;
use zkore_engine::wallet_manager::WalletManager;

pub struct AppState {
    pub wallet_manager: Mutex<WalletManager>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let key_store = Box::new(InMemoryKeyStore::default());
        let app_db_path = default_app_db_path()?;
        let wallet_manager = WalletManager::new(app_db_path, key_store)?;
        Ok(Self {
            wallet_manager: Mutex::new(wallet_manager),
        })
    }
}

fn default_app_db_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".zkore").join("app.db"))
}

#[derive(Debug, Default)]
struct InMemoryKeyStore {
    encrypted_mnemonics: Mutex<HashMap<(Uuid, u8), Vec<u8>>>,
    keychain: Mutex<HashMap<(Uuid, u8), Vec<u8>>>,
}

impl KeyStore for InMemoryKeyStore {
    fn store_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()> {
        self.encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .insert((wallet_id, network_key(network)), encrypted_mnemonic.to_vec());
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

    fn delete_encrypted_mnemonic(&self, wallet_id: Uuid, network: Network) -> anyhow::Result<()> {
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
