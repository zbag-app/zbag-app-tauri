use std::path::PathBuf;

use anyhow::Context as _;
use uuid::Uuid;

use bagz_core::domain::Network;

use crate::key_store::KeyStore;
use bagz_core::permissions::{create_dir_all_secure, write_file_secure};

#[derive(Debug, Clone)]
pub struct KeyStoreKeychain {
    wallets_root: PathBuf,
    service: String,
}

impl KeyStoreKeychain {
    pub fn new(wallets_root: PathBuf) -> Self {
        Self {
            wallets_root,
            service: "bagz".to_string(),
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
}

impl KeyStore for KeyStoreKeychain {
    fn store_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()> {
        let path = self.mnemonic_path(wallet_id, network);
        if let Some(parent) = path.parent() {
            create_dir_all_secure(parent).with_context(|| {
                format!(
                    "failed to create key store parent directory: {}",
                    parent.display()
                )
            })?;
        }
        write_file_secure(&path, encrypted_mnemonic).with_context(|| {
            format!(
                "failed to write encrypted mnemonic file: {}",
                path.display()
            )
        })?;
        Ok(())
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

    /// DISABLED: Keychain-based auto-unlock is disabled due to security concerns.
    ///
    /// macOS Keychain with biometric access (`kSecAccessControlBiometryAny`) allows ANY enrolled
    /// fingerprint on the device to authenticate - it does not distinguish between different users'
    /// fingerprints. This is problematic for shared devices or situations where someone else's
    /// fingerprint was added to the device.
    ///
    /// See: https://github.com/bagzapp/bagz/issues/45
    fn store_keychain_unlock_material(
        &self,
        _wallet_id: Uuid,
        _network: Network,
        _unlock_material: &[u8],
    ) -> anyhow::Result<()> {
        // No-op: Do not store DEK in keychain to prevent false sense of biometric security
        Ok(())
    }

    /// DISABLED: Keychain-based auto-unlock is disabled due to security concerns.
    ///
    /// Always returns None to require password entry on every unlock.
    /// See `store_keychain_unlock_material` for rationale.
    fn load_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        // Clean up any previously stored keychain entries to prevent stale data
        match self.delete_keychain_unlock_material(wallet_id, network) {
            Ok(()) => {
                tracing::debug!(
                    wallet_id = %wallet_id,
                    "cleaned up stale keychain entry during disabled auto-unlock load"
                );
            }
            Err(e) => {
                tracing::warn!(
                    wallet_id = %wallet_id,
                    error = ?e,
                    "failed to clean up stale keychain entry during disabled auto-unlock load"
                );
            }
        }
        // Always return None to require password entry
        Ok(None)
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
