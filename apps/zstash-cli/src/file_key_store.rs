//! File-based key store for CLI usage.
//!
//! Stores encrypted mnemonics in wallet directories and unlock materials
//! in a local JSON file instead of the OS keychain.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use zkore_core::domain::Network;
use zkore_engine::key_store::KeyStore;

/// File-based key store for CLI usage.
///
/// Structure:
/// - `~/.zkore/keystore.json` - Contains unlock material entries
/// - `~/.zkore/wallets/{network}/{wallet_id}/mnemonic.enc` - Encrypted mnemonic
#[derive(Debug, Clone)]
pub struct FileKeyStore {
    wallets_root: PathBuf,
    keystore_path: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct KeyStoreData {
    /// Map of "wallet_id:network" -> base64-encoded unlock material
    unlock_materials: HashMap<String, String>,
}

impl FileKeyStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            wallets_root: data_dir.join("wallets"),
            keystore_path: data_dir.join("keystore.json"),
        }
    }

    fn wallet_dir(&self, wallet_id: Uuid, network: Network) -> PathBuf {
        self.wallets_root
            .join(network_dir_name(network))
            .join(wallet_id.to_string())
    }

    fn mnemonic_path(&self, wallet_id: Uuid, network: Network) -> PathBuf {
        self.wallet_dir(wallet_id, network).join("mnemonic.enc")
    }

    fn unlock_key(&self, wallet_id: Uuid, network: Network) -> String {
        format!("{}:{}", wallet_id, network_dir_name(network))
    }

    fn load_keystore(&self) -> anyhow::Result<KeyStoreData> {
        match fs::read_to_string(&self.keystore_path) {
            Ok(content) => serde_json::from_str(&content).context("invalid keystore.json format"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(KeyStoreData::default()),
            Err(e) => Err(e).context("failed to read keystore.json"),
        }
    }

    fn save_keystore(&self, data: &KeyStoreData) -> anyhow::Result<()> {
        if let Some(parent) = self.keystore_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(data)?;

        // Atomic write via temp file
        let tmp_path = self.keystore_path.with_extension("json.tmp");
        fs::write(&tmp_path, &content)?;
        fs::rename(&tmp_path, &self.keystore_path)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.keystore_path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    fn write_file_atomic(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("enc.tmp");
        fs::write(&tmp_path, contents)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

impl KeyStore for FileKeyStore {
    fn store_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()> {
        let path = self.mnemonic_path(wallet_id, network);
        Self::write_file_atomic(&path, encrypted_mnemonic)
    }

    fn load_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let path = self.mnemonic_path(wallet_id, network);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context(format!("failed to read {}", path.display())),
        }
    }

    fn delete_encrypted_mnemonic(&self, wallet_id: Uuid, network: Network) -> anyhow::Result<()> {
        let path = self.mnemonic_path(wallet_id, network);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context(format!("failed to delete {}", path.display())),
        }
    }

    fn store_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
        unlock_material: &[u8],
    ) -> anyhow::Result<()> {
        let mut data = self.load_keystore()?;
        let key = self.unlock_key(wallet_id, network);
        let encoded = base64::engine::general_purpose::STANDARD.encode(unlock_material);
        data.unlock_materials.insert(key, encoded);
        self.save_keystore(&data)
    }

    fn load_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let data = self.load_keystore()?;
        let key = self.unlock_key(wallet_id, network);
        match data.unlock_materials.get(&key) {
            Some(encoded) => {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .context("invalid unlock material encoding")?;
                Ok(Some(decoded))
            }
            None => Ok(None),
        }
    }

    fn delete_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<()> {
        let mut data = self.load_keystore()?;
        let key = self.unlock_key(wallet_id, network);
        data.unlock_materials.remove(&key);
        self.save_keystore(&data)
    }
}

fn network_dir_name(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
    }
}
