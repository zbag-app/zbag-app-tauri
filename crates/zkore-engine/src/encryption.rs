use argon2::{password_hash::SaltString, Argon2, Params, PasswordHasher};
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use uuid::Uuid;
use zeroize::Zeroize;

use zkore_core::domain::Network;

pub const KDF_ALGORITHM: &str = "argon2id";
pub const KDF_VERSION: u32 = 1;
pub const KDF_MEMORY_MIB: u32 = 64;
pub const KDF_ITERATIONS: u32 = 3;
pub const KDF_PARALLELISM: u32 = 1;

pub const AEAD_SCHEME: &str = "xchacha20poly1305";
pub const AEAD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletKdfParams {
    pub algorithm: String,
    pub version: u32,
    pub memory_mib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt_b64: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletAeadParams {
    pub scheme: String,
    pub version: u32,
    pub nonce_b64: String,
}

#[derive(Zeroize)]
#[zeroize(drop)]
pub struct Dek(pub [u8; 32]);

#[derive(Zeroize)]
#[zeroize(drop)]
struct Kek([u8; 32]);

pub fn generate_dek() -> Dek {
    let mut dek = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut dek);
    Dek(dek)
}

pub fn generate_kdf_salt_b64() -> String {
    SaltString::generate(&mut rand::thread_rng()).as_str().to_string()
}

pub fn generate_nonce_b64() -> String {
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    base64::engine::general_purpose::STANDARD.encode(nonce)
}

pub fn wrap_dek(
    wallet_id: Uuid,
    network: Network,
    password: &str,
    kdf_salt_b64: &str,
    aead_nonce_b64: &str,
    dek: &Dek,
) -> anyhow::Result<String> {
    let mut kek = derive_kek(password, kdf_salt_b64)?;
    let aad = aead_aad(wallet_id, network);

    let nonce_bytes = base64::engine::general_purpose::STANDARD
        .decode(aead_nonce_b64)
        .map_err(|e| anyhow::anyhow!("invalid AEAD nonce base64: {e}"))?;
    if nonce_bytes.len() != 24 {
        return Err(anyhow::anyhow!("invalid AEAD nonce length: {}", nonce_bytes.len()));
    }
    let nonce: &XNonce = XNonce::from_slice(&nonce_bytes);

    let cipher = XChaCha20Poly1305::new_from_slice(&kek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let ciphertext = cipher
        .encrypt(nonce, Payload { msg: &dek.0, aad: aad.as_bytes() })
        .map_err(|e| anyhow::anyhow!("failed to wrap DEK: {e}"))?;

    kek.0.zeroize();

    Ok(base64::engine::general_purpose::STANDARD.encode(ciphertext))
}

pub fn unwrap_dek(
    wallet_id: Uuid,
    network: Network,
    password: &str,
    kdf_salt_b64: &str,
    aead_nonce_b64: &str,
    wrapped_dek_b64: &str,
) -> anyhow::Result<Dek> {
    let mut kek = derive_kek(password, kdf_salt_b64)?;
    let aad = aead_aad(wallet_id, network);

    let nonce_bytes = base64::engine::general_purpose::STANDARD
        .decode(aead_nonce_b64)
        .map_err(|e| anyhow::anyhow!("invalid AEAD nonce base64: {e}"))?;
    if nonce_bytes.len() != 24 {
        return Err(anyhow::anyhow!("invalid AEAD nonce length: {}", nonce_bytes.len()));
    }
    let nonce: &XNonce = XNonce::from_slice(&nonce_bytes);

    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(wrapped_dek_b64)
        .map_err(|e| anyhow::anyhow!("invalid wrapped DEK base64: {e}"))?;

    let cipher = XChaCha20Poly1305::new_from_slice(&kek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let plaintext = cipher
        .decrypt(nonce, Payload { msg: &ciphertext, aad: aad.as_bytes() })
        .map_err(|e| anyhow::anyhow!("failed to unwrap DEK: {e}"))?;

    kek.0.zeroize();

    let mut dek = [0u8; 32];
    if plaintext.len() != 32 {
        return Err(anyhow::anyhow!("invalid DEK length: {}", plaintext.len()));
    }
    dek.copy_from_slice(&plaintext);
    Ok(Dek(dek))
}

pub fn default_kdf_params() -> WalletKdfParams {
    WalletKdfParams {
        algorithm: KDF_ALGORITHM.to_string(),
        version: KDF_VERSION,
        memory_mib: KDF_MEMORY_MIB,
        iterations: KDF_ITERATIONS,
        parallelism: KDF_PARALLELISM,
        salt_b64: generate_kdf_salt_b64(),
    }
}

pub fn default_aead_params() -> WalletAeadParams {
    WalletAeadParams {
        scheme: AEAD_SCHEME.to_string(),
        version: AEAD_VERSION,
        nonce_b64: generate_nonce_b64(),
    }
}

fn derive_kek(password: &str, salt_b64: &str) -> anyhow::Result<Kek> {
    let salt = SaltString::from_b64(salt_b64)
        .map_err(|e| anyhow::anyhow!("invalid KDF salt: {e}"))?;
    let params = Params::new(
        KDF_MEMORY_MIB * 1024,
        KDF_ITERATIONS,
        KDF_PARALLELISM,
        Some(32),
    )
    .map_err(|e| anyhow::anyhow!("invalid KDF params: {e}"))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to derive KEK: {e}"))?;

    let Some(output) = hash.hash else {
        return Err(anyhow::anyhow!("argon2 output missing"));
    };

    let bytes = output.as_bytes();
    let mut kek = [0u8; 32];
    kek.copy_from_slice(bytes);
    Ok(Kek(kek))
}

fn aead_aad(wallet_id: Uuid, network: Network) -> String {
    format!(
        "wallet_id={wallet_id};network={network:?};aead_scheme={AEAD_SCHEME};aead_version={AEAD_VERSION}"
    )
}
