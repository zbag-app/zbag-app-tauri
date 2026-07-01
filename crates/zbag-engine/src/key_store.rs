use uuid::Uuid;

use zbag_core::domain::Network;

pub trait KeyStore: Send + Sync {
    fn store_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()>;

    fn load_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>>;

    fn delete_encrypted_mnemonic(&self, wallet_id: Uuid, network: Network) -> anyhow::Result<()>;

    /// VESTIGIAL: Keychain auto-unlock is disabled. This method is a no-op.
    /// See SECURITY.md for rationale
    fn store_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
        unlock_material: &[u8],
    ) -> anyhow::Result<()>;

    /// VESTIGIAL: Keychain auto-unlock is disabled. Always returns `Ok(None)`.
    /// See SECURITY.md for rationale
    fn load_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>>;

    /// Deletes any existing keychain entry. Still functional for cleanup.
    fn delete_keychain_unlock_material(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<()>;
}
