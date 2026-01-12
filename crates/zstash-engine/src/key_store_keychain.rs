use std::path::{Path, PathBuf};

use anyhow::Context as _;
use base64::Engine as _;
use uuid::Uuid;

use zstash_core::domain::Network;

use crate::key_store::KeyStore;

#[derive(Debug, Clone)]
pub struct KeyStoreKeychain {
    wallets_root: PathBuf,
    service: String,
}

impl KeyStoreKeychain {
    pub fn new(wallets_root: PathBuf) -> Self {
        Self {
            wallets_root,
            service: "zstash".to_string(),
        }
    }

    pub fn with_service_name(mut self, service: impl Into<String>) -> Self {
        self.service = service.into();
        self
    }

    fn wallet_dir(&self, wallet_id: Uuid, network: Network) -> PathBuf {
        self.wallets_root
            .join(network_dir_name(network))
            .join(wallet_id.to_string())
    }

    fn mnemonic_path(&self, wallet_id: Uuid, network: Network) -> PathBuf {
        self.wallet_dir(wallet_id, network).join("mnemonic.enc")
    }

    fn keychain_entry(&self, wallet_id: Uuid, network: Network) -> anyhow::Result<keyring::Entry> {
        let username = format!("wallet-dek:{wallet_id}:{}", network_dir_name(network));
        keyring::Entry::new(&self.service, &username).context("failed to construct keychain entry")
    }

    fn write_file_atomic(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create key store parent directory: {}",
                    parent.display()
                )
            })?;
        }

        let mut tmp = path.as_os_str().to_os_string();
        tmp.push(".tmp");
        let tmp_path = PathBuf::from(tmp);

        std::fs::write(&tmp_path, contents).with_context(|| {
            format!(
                "failed to write key store temp file: {}",
                tmp_path.display()
            )
        })?;
        std::fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "failed to move key store temp file into place: {} -> {}",
                tmp_path.display(),
                path.display()
            )
        })?;
        Ok(())
    }
}

impl KeyStore for KeyStoreKeychain {
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
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
        }
    }

    fn delete_encrypted_mnemonic(&self, wallet_id: Uuid, network: Network) -> anyhow::Result<()> {
        let path = self.mnemonic_path(wallet_id, network);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| format!("failed to delete {}", path.display())),
        }
    }

    fn store_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
        unlock_material: &[u8],
    ) -> anyhow::Result<()> {
        let entry = self.keychain_entry(wallet_id, network)?;
        let secret_b64 = base64::engine::general_purpose::STANDARD.encode(unlock_material);
        entry
            .set_password(&secret_b64)
            .context("failed to store keychain unlock material")?;
        Ok(())
    }

    fn load_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let entry = self.keychain_entry(wallet_id, network)?;
        match entry.get_password() {
            Ok(secret_b64) => {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(secret_b64)
                    .context("invalid keychain unlock material encoding")?;
                Ok(Some(decoded))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err).context("failed to read keychain unlock material"),
        }
    }

    fn delete_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<()> {
        let entry = self.keychain_entry(wallet_id, network)?;
        match entry.delete_password() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err).context("failed to delete keychain unlock material"),
        }
    }
}

fn network_dir_name(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
    }
}
