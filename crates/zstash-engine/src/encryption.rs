use argon2::{
    Argon2, Params,
    password_hash::{Salt, SaltString},
};
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, Tag, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use zstash_core::domain::Network;

pub const KDF_ALGORITHM_ARGON2ID: &str = "argon2id";
pub const KDF_VERSION_V1: u32 = 1;
pub const AEAD_SCHEME_XCHACHA20POLY1305: &str = "xchacha20poly1305";
pub const AEAD_VERSION_V1: u32 = 1;

pub const KDF_ALGORITHM: &str = KDF_ALGORITHM_ARGON2ID;
pub const KDF_VERSION: u32 = KDF_VERSION_V1;
pub const KDF_MEMORY_MIB: u32 = 64;
pub const KDF_ITERATIONS: u32 = 3;
pub const KDF_PARALLELISM: u32 = 1;

pub const AEAD_SCHEME: &str = AEAD_SCHEME_XCHACHA20POLY1305;
pub const AEAD_VERSION: u32 = AEAD_VERSION_V1;

/// Supported KDF algorithms for parameter validation.
///
/// When introducing new versions/algorithms, append them to the supported lists;
/// do not remove old values, or existing wallets may become undecryptable.
const SUPPORTED_KDF_ALGORITHMS: &[&str] = &[KDF_ALGORITHM_ARGON2ID];
/// Supported KDF versions for parameter validation.
const SUPPORTED_KDF_VERSIONS: &[u32] = &[KDF_VERSION_V1];
/// Supported AEAD schemes for parameter validation.
const SUPPORTED_AEAD_SCHEMES: &[&str] = &[AEAD_SCHEME_XCHACHA20POLY1305];
/// Supported AEAD versions for parameter validation.
const SUPPORTED_AEAD_VERSIONS: &[u32] = &[AEAD_VERSION_V1];

const KDF_MEMORY_MIB_MIN: u32 = 1;
const KDF_MEMORY_MIB_MAX: u32 = 256;
const KDF_ITERATIONS_MIN: u32 = 1;
const KDF_ITERATIONS_MAX: u32 = 100;
const KDF_PARALLELISM_MIN: u32 = 1;
const KDF_PARALLELISM_MAX: u32 = 16;

// Argon2 best-practice guidance recommends salts are at least 8 bytes.
const KDF_SALT_LEN_MIN_BYTES: usize = 8;
// `password_hash::Salt` (PHC string) caps the *encoded* salt at 64 characters, which decodes to
// <= 48 bytes. A 64-byte buffer is intentionally generous and avoids heap allocation.
const KDF_SALT_LEN_MAX_BYTES: usize = 64;

const AEAD_NONCE_LEN_BYTES: usize = std::mem::size_of::<XNonce>();
const DEK_LEN_BYTES: usize = 32;
const AEAD_TAG_LEN_BYTES: usize = std::mem::size_of::<Tag>();
const WRAPPED_DEK_LEN_BYTES: usize = DEK_LEN_BYTES + AEAD_TAG_LEN_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletKdfParams {
    pub algorithm: String,
    /// Schema version for KDF parameters (not Argon2 version which is always V0x13).
    pub version: u32,
    pub memory_mib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt_b64: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletAeadParams {
    pub scheme: String,
    /// Schema version for AEAD parameters (not the underlying cipher implementation version).
    pub version: u32,
    pub nonce_b64: String,
}

#[derive(Zeroize)]
#[zeroize(drop)]
pub struct Dek(pub [u8; 32]);

impl std::fmt::Debug for Dek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Dek([REDACTED])")
    }
}

#[derive(Zeroize)]
#[zeroize(drop)]
struct Kek([u8; 32]);

pub fn generate_dek() -> Dek {
    let mut dek = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut dek);
    Dek(dek)
}

pub fn generate_kdf_salt_b64() -> String {
    SaltString::generate(&mut rand::thread_rng())
        .as_str()
        .to_string()
}

pub fn generate_nonce_b64() -> String {
    let mut nonce = [0u8; AEAD_NONCE_LEN_BYTES];
    rand::thread_rng().fill_bytes(&mut nonce);
    base64::engine::general_purpose::STANDARD.encode(nonce)
}

/// Validates that the KDF parameters are supported by this implementation (including decoded
/// salt length).
pub fn validate_kdf_params(kdf: &WalletKdfParams) -> anyhow::Result<()> {
    validate_and_decode_kdf_salt(kdf).map(|_| ())
}

/// Decoded KDF salt bytes.
///
/// Uses a fixed-size stack buffer to avoid heap allocation. The PHC-encoded salt is capped at 64
/// characters (<= 48 decoded bytes), but we keep a 64-byte buffer as a generous upper bound.
#[derive(Zeroize)]
#[zeroize(drop)]
struct KdfSaltBytes {
    buf: [u8; KDF_SALT_LEN_MAX_BYTES],
    len: usize,
}

impl KdfSaltBytes {
    fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

fn validate_and_decode_kdf_salt(kdf: &WalletKdfParams) -> anyhow::Result<KdfSaltBytes> {
    if !SUPPORTED_KDF_ALGORITHMS.contains(&kdf.algorithm.as_str()) {
        return Err(anyhow::anyhow!(
            "unsupported KDF algorithm: {}",
            kdf.algorithm
        ));
    }
    if !SUPPORTED_KDF_VERSIONS.contains(&kdf.version) {
        return Err(anyhow::anyhow!("unsupported KDF version: {}", kdf.version));
    }

    // Validate reasonable bounds for KDF parameters
    if !(KDF_MEMORY_MIB_MIN..=KDF_MEMORY_MIB_MAX).contains(&kdf.memory_mib) {
        return Err(anyhow::anyhow!(
            "KDF memory_mib out of bounds: {} (valid: {}-{})",
            kdf.memory_mib,
            KDF_MEMORY_MIB_MIN,
            KDF_MEMORY_MIB_MAX
        ));
    }
    if !(KDF_ITERATIONS_MIN..=KDF_ITERATIONS_MAX).contains(&kdf.iterations) {
        return Err(anyhow::anyhow!(
            "KDF iterations out of bounds: {} (valid: {}-{})",
            kdf.iterations,
            KDF_ITERATIONS_MIN,
            KDF_ITERATIONS_MAX
        ));
    }
    if !(KDF_PARALLELISM_MIN..=KDF_PARALLELISM_MAX).contains(&kdf.parallelism) {
        return Err(anyhow::anyhow!(
            "KDF parallelism out of bounds: {} (valid: {}-{})",
            kdf.parallelism,
            KDF_PARALLELISM_MIN,
            KDF_PARALLELISM_MAX
        ));
    }

    let salt =
        Salt::from_b64(&kdf.salt_b64).map_err(|e| anyhow::anyhow!("invalid KDF salt: {e}"))?;
    let mut salt_buf = [0u8; KDF_SALT_LEN_MAX_BYTES];
    let decoded_len = {
        let decoded_salt = salt
            .decode_b64(&mut salt_buf)
            .map_err(|e| anyhow::anyhow!("invalid KDF salt: {e}"))?;
        decoded_salt.len()
    };

    if decoded_len < KDF_SALT_LEN_MIN_BYTES {
        return Err(anyhow::anyhow!(
            "invalid KDF salt length: {} (min: {})",
            decoded_len,
            KDF_SALT_LEN_MIN_BYTES
        ));
    }

    Ok(KdfSaltBytes {
        buf: salt_buf,
        len: decoded_len,
    })
}

/// Validates that the AEAD parameters are supported by this implementation.
pub fn validate_aead_params(aead: &WalletAeadParams) -> anyhow::Result<()> {
    validate_and_decode_aead_nonce(aead).map(|_| ())
}

fn validate_and_decode_aead_nonce(
    aead: &WalletAeadParams,
) -> anyhow::Result<[u8; AEAD_NONCE_LEN_BYTES]> {
    if !SUPPORTED_AEAD_SCHEMES.contains(&aead.scheme.as_str()) {
        return Err(anyhow::anyhow!("unsupported AEAD scheme: {}", aead.scheme));
    }
    if !SUPPORTED_AEAD_VERSIONS.contains(&aead.version) {
        return Err(anyhow::anyhow!(
            "unsupported AEAD version: {}",
            aead.version
        ));
    }

    let mut nonce = [0u8; AEAD_NONCE_LEN_BYTES];
    let decoded_len = base64::engine::general_purpose::STANDARD
        .decode_slice(&aead.nonce_b64, &mut nonce)
        .map_err(|e| match e {
            base64::DecodeSliceError::OutputSliceTooSmall => {
                anyhow::anyhow!("invalid AEAD nonce length: > {}", AEAD_NONCE_LEN_BYTES)
            }
            _ => anyhow::anyhow!("invalid AEAD nonce base64: {e}"),
        })?;

    if decoded_len != AEAD_NONCE_LEN_BYTES {
        return Err(anyhow::anyhow!(
            "invalid AEAD nonce length: {}",
            decoded_len
        ));
    }

    Ok(nonce)
}

pub fn wrap_dek(
    wallet_id: Uuid,
    network: Network,
    password: &str,
    kdf: &WalletKdfParams,
    aead: &WalletAeadParams,
    dek: &Dek,
) -> anyhow::Result<String> {
    let salt = validate_and_decode_kdf_salt(kdf)?;
    let nonce_bytes = validate_and_decode_aead_nonce(aead)?;

    let mut kek = derive_kek(password, kdf, salt.as_slice())?;
    let aad = aead_aad(wallet_id, network, aead);

    let nonce: &XNonce = XNonce::from_slice(&nonce_bytes);

    let cipher = XChaCha20Poly1305::new_from_slice(&kek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: &dek.0,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to wrap DEK: {e}"))?;

    kek.0.zeroize();

    Ok(base64::engine::general_purpose::STANDARD.encode(ciphertext))
}

pub fn unwrap_dek(
    wallet_id: Uuid,
    network: Network,
    password: &str,
    kdf: &WalletKdfParams,
    aead: &WalletAeadParams,
    wrapped_dek_b64: &str,
) -> anyhow::Result<Dek> {
    let salt = validate_and_decode_kdf_salt(kdf)?;
    let nonce_bytes = validate_and_decode_aead_nonce(aead)?;

    let mut kek = derive_kek(password, kdf, salt.as_slice())?;
    let aad = aead_aad(wallet_id, network, aead);

    let nonce: &XNonce = XNonce::from_slice(&nonce_bytes);

    let mut ciphertext = [0u8; WRAPPED_DEK_LEN_BYTES];
    let decoded_len = base64::engine::general_purpose::STANDARD
        .decode_slice(wrapped_dek_b64, &mut ciphertext)
        .map_err(|e| match e {
            base64::DecodeSliceError::OutputSliceTooSmall => {
                anyhow::anyhow!("invalid wrapped DEK length: > {}", WRAPPED_DEK_LEN_BYTES)
            }
            _ => anyhow::anyhow!("invalid wrapped DEK base64: {e}"),
        })?;
    if decoded_len != WRAPPED_DEK_LEN_BYTES {
        return Err(anyhow::anyhow!(
            "invalid wrapped DEK length: {}",
            decoded_len
        ));
    }

    let cipher = XChaCha20Poly1305::new_from_slice(&kek.0)
        .map_err(|e| anyhow::anyhow!("failed to init AEAD: {e}"))?;

    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to unwrap DEK: {e}"))?;
    let plaintext = Zeroizing::new(plaintext);

    kek.0.zeroize();

    let mut dek = [0u8; DEK_LEN_BYTES];
    if plaintext.len() != DEK_LEN_BYTES {
        return Err(anyhow::anyhow!("invalid DEK length: {}", plaintext.len()));
    }
    dek.copy_from_slice(plaintext.as_slice());
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

/// Derives a KEK from the password using the stored KDF parameters.
/// Currently only Argon2id is supported; the algorithm field is validated upstream.
fn derive_kek(password: &str, kdf: &WalletKdfParams, salt: &[u8]) -> anyhow::Result<Kek> {
    let memory_kib = kdf
        .memory_mib
        .checked_mul(1024)
        .ok_or_else(|| anyhow::anyhow!("KDF memory overflow"))?;
    let params = Params::new(memory_kib, kdf.iterations, kdf.parallelism, Some(32))
        .map_err(|e| anyhow::anyhow!("invalid KDF params: {e}"))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    let mut kek = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut kek)
        .map_err(|e| anyhow::anyhow!("failed to derive KEK: {e}"))?;
    Ok(Kek(kek))
}

fn aead_aad(wallet_id: Uuid, network: Network, aead: &WalletAeadParams) -> String {
    format!(
        "wallet_id={wallet_id};network={network:?};aead_scheme={};aead_version={}",
        aead.scheme, aead.version
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_kdf_params_supported() {
        let kdf = default_kdf_params();
        assert!(validate_kdf_params(&kdf).is_ok());
    }

    #[test]
    fn test_validate_kdf_params_unsupported_algorithm() {
        let kdf = WalletKdfParams {
            algorithm: "bcrypt".to_string(),
            version: 1,
            memory_mib: 64,
            iterations: 3,
            parallelism: 1,
            salt_b64: generate_kdf_salt_b64(),
        };
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("unsupported KDF algorithm"));
    }

    #[test]
    fn test_validate_kdf_params_unsupported_version() {
        let kdf = WalletKdfParams {
            algorithm: "argon2id".to_string(),
            version: 99,
            memory_mib: 64,
            iterations: 3,
            parallelism: 1,
            salt_b64: generate_kdf_salt_b64(),
        };
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("unsupported KDF version"));
    }

    #[test]
    fn test_validate_kdf_params_memory_bounds() {
        let mut kdf = default_kdf_params();
        kdf.memory_mib = 0;
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("memory_mib out of bounds"));

        kdf.memory_mib = KDF_MEMORY_MIB_MIN;
        assert!(validate_kdf_params(&kdf).is_ok());

        kdf.memory_mib = KDF_MEMORY_MIB_MAX;
        assert!(validate_kdf_params(&kdf).is_ok());

        kdf.memory_mib = KDF_MEMORY_MIB_MAX + 1;
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("memory_mib out of bounds"));
    }

    #[test]
    fn test_validate_kdf_params_iterations_bounds() {
        let mut kdf = default_kdf_params();
        kdf.iterations = 0;
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("iterations out of bounds"));

        kdf.iterations = KDF_ITERATIONS_MIN;
        assert!(validate_kdf_params(&kdf).is_ok());

        kdf.iterations = KDF_ITERATIONS_MAX;
        assert!(validate_kdf_params(&kdf).is_ok());

        kdf.iterations = KDF_ITERATIONS_MAX + 1;
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("iterations out of bounds"));
    }

    #[test]
    fn test_validate_kdf_params_parallelism_bounds() {
        let mut kdf = default_kdf_params();
        kdf.parallelism = 0;
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("parallelism out of bounds"));

        kdf.parallelism = KDF_PARALLELISM_MIN;
        assert!(validate_kdf_params(&kdf).is_ok());

        kdf.parallelism = KDF_PARALLELISM_MAX;
        assert!(validate_kdf_params(&kdf).is_ok());

        kdf.parallelism = KDF_PARALLELISM_MAX + 1;
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("parallelism out of bounds"));
    }

    #[test]
    fn test_validate_kdf_params_rejects_invalid_salt_base64() {
        let mut kdf = default_kdf_params();
        kdf.salt_b64 = "!!!".to_string();
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("invalid KDF salt"));
    }

    #[test]
    fn test_validate_kdf_params_rejects_short_salt() {
        let mut kdf = default_kdf_params();
        kdf.salt_b64 = "AAAAAAAA".to_string();
        let err = validate_kdf_params(&kdf).unwrap_err();
        assert!(err.to_string().contains("invalid KDF salt length"));
    }

    #[test]
    fn test_validate_aead_params_supported() {
        let aead = default_aead_params();
        assert!(validate_aead_params(&aead).is_ok());
    }

    #[test]
    fn test_validate_aead_params_unsupported_scheme() {
        let aead = WalletAeadParams {
            scheme: "aes256gcm".to_string(),
            version: 1,
            nonce_b64: generate_nonce_b64(),
        };
        let err = validate_aead_params(&aead).unwrap_err();
        assert!(err.to_string().contains("unsupported AEAD scheme"));
    }

    #[test]
    fn test_validate_aead_params_unsupported_version() {
        let aead = WalletAeadParams {
            scheme: "xchacha20poly1305".to_string(),
            version: 99,
            nonce_b64: generate_nonce_b64(),
        };
        let err = validate_aead_params(&aead).unwrap_err();
        assert!(err.to_string().contains("unsupported AEAD version"));
    }

    #[test]
    fn test_validate_aead_params_rejects_invalid_nonce_base64() {
        let aead = WalletAeadParams {
            scheme: AEAD_SCHEME.to_string(),
            version: AEAD_VERSION,
            nonce_b64: "!!!".to_string(),
        };
        let err = validate_aead_params(&aead).unwrap_err();
        assert!(err.to_string().contains("invalid AEAD nonce base64"));
    }

    #[test]
    fn test_validate_aead_params_rejects_invalid_nonce_length() {
        let nonce_b64 =
            base64::engine::general_purpose::STANDARD.encode([0u8; AEAD_NONCE_LEN_BYTES - 1]);
        let aead = WalletAeadParams {
            scheme: AEAD_SCHEME.to_string(),
            version: AEAD_VERSION,
            nonce_b64,
        };
        let err = validate_aead_params(&aead).unwrap_err();
        assert!(err.to_string().contains("invalid AEAD nonce length"));
    }

    #[test]
    fn test_wrap_unwrap_dek_roundtrip() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();

        let wrapped =
            wrap_dek(wallet_id, network, password, &kdf, &aead, &dek).expect("wrap should succeed");

        let unwrapped = unwrap_dek(wallet_id, network, password, &kdf, &aead, &wrapped)
            .expect("unwrap should succeed");

        assert_eq!(dek.0, unwrapped.0);
    }

    #[test]
    fn test_wrap_unwrap_dek_roundtrip_with_non_default_kdf_params() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let dek = generate_dek();
        let mut kdf = default_kdf_params();
        kdf.memory_mib = 32;
        let aead = default_aead_params();

        let wrapped =
            wrap_dek(wallet_id, network, password, &kdf, &aead, &dek).expect("wrap should succeed");

        let unwrapped = unwrap_dek(wallet_id, network, password, &kdf, &aead, &wrapped)
            .expect("unwrap should succeed");

        assert_eq!(dek.0, unwrapped.0);
    }

    #[test]
    fn test_wrap_dek_rejects_unsupported_kdf() {
        let wallet_id = Uuid::new_v4();
        let dek = generate_dek();
        let mut kdf = default_kdf_params();
        kdf.algorithm = "unsupported".to_string();
        let aead = default_aead_params();

        let err = wrap_dek(wallet_id, Network::Testnet, "pw", &kdf, &aead, &dek).unwrap_err();
        assert!(err.to_string().contains("unsupported KDF algorithm"));
    }

    #[test]
    fn test_unwrap_dek_validates_aead_params() {
        let wallet_id = Uuid::new_v4();
        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();

        let wrapped = wrap_dek(wallet_id, Network::Testnet, "pw", &kdf, &aead, &dek)
            .expect("wrap should succeed");

        let mut bad_aead = aead.clone();
        bad_aead.scheme = "unsupported".to_string();

        let err =
            unwrap_dek(wallet_id, Network::Testnet, "pw", &kdf, &bad_aead, &wrapped).unwrap_err();
        assert!(err.to_string().contains("unsupported AEAD scheme"));
    }

    #[test]
    fn test_unwrap_dek_rejects_invalid_wrapped_dek_base64() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let kdf = default_kdf_params();
        let aead = default_aead_params();

        let err = unwrap_dek(wallet_id, network, password, &kdf, &aead, "!!!").unwrap_err();
        assert!(err.to_string().contains("invalid wrapped DEK base64"));
    }

    #[test]
    fn test_unwrap_dek_rejects_invalid_wrapped_dek_length() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let kdf = default_kdf_params();
        let aead = default_aead_params();

        let wrapped_too_short =
            base64::engine::general_purpose::STANDARD.encode([0u8; WRAPPED_DEK_LEN_BYTES - 1]);
        let err = unwrap_dek(
            wallet_id,
            network,
            password,
            &kdf,
            &aead,
            &wrapped_too_short,
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid wrapped DEK length"));

        let wrapped_too_long =
            base64::engine::general_purpose::STANDARD.encode([0u8; WRAPPED_DEK_LEN_BYTES + 1]);
        let err =
            unwrap_dek(wallet_id, network, password, &kdf, &aead, &wrapped_too_long).unwrap_err();
        assert!(err.to_string().contains("invalid wrapped DEK length"));
    }

    #[test]
    fn test_unwrap_dek_fails_with_wrong_password() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let correct_password = "correct_password";
        let wrong_password = "wrong_password";
        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();

        let wrapped = wrap_dek(wallet_id, network, correct_password, &kdf, &aead, &dek)
            .expect("wrap should succeed");

        let result = unwrap_dek(wallet_id, network, wrong_password, &kdf, &aead, &wrapped);
        assert!(result.is_err(), "unwrap should fail with wrong password");
    }

    #[test]
    fn test_unwrap_dek_fails_with_wrong_network() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();

        let wrapped =
            wrap_dek(wallet_id, network, password, &kdf, &aead, &dek).expect("wrap should succeed");

        let result = unwrap_dek(wallet_id, Network::Mainnet, password, &kdf, &aead, &wrapped);
        assert!(result.is_err(), "unwrap should fail with wrong network");
    }

    #[test]
    fn test_aead_aad_uses_persisted_params() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Mainnet;
        let aead = WalletAeadParams {
            scheme: "xchacha20poly1305".to_string(),
            version: 1,
            nonce_b64: generate_nonce_b64(),
        };

        let aad = aead_aad(wallet_id, network, &aead);
        assert!(aad.contains("aead_scheme=xchacha20poly1305"));
        assert!(aad.contains("aead_version=1"));
    }

    #[test]
    fn test_unwrap_dek_fails_with_mismatched_kdf_params() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let dek = generate_dek();

        // Wrap with memory_mib=32 (different from default 64)
        let mut kdf_wrap = default_kdf_params();
        kdf_wrap.memory_mib = 32;
        let aead = default_aead_params();

        let wrapped = wrap_dek(wallet_id, network, password, &kdf_wrap, &aead, &dek)
            .expect("wrap should succeed");

        // Attempt unwrap with memory_mib=128 (different KDF params)
        let mut kdf_unwrap = kdf_wrap.clone();
        kdf_unwrap.memory_mib = 128;

        let result = unwrap_dek(wallet_id, network, password, &kdf_unwrap, &aead, &wrapped);
        assert!(
            result.is_err(),
            "unwrap should fail with mismatched KDF params"
        );
    }

    #[test]
    fn test_unwrap_dek_fails_with_mismatched_aead_params() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let dek = generate_dek();

        let kdf = default_kdf_params();
        let aead_wrap = default_aead_params();

        let wrapped = wrap_dek(wallet_id, network, password, &kdf, &aead_wrap, &dek)
            .expect("wrap should succeed");

        // Attempt unwrap with a different nonce (different AEAD params)
        let mut aead_unwrap = aead_wrap.clone();
        aead_unwrap.nonce_b64 = generate_nonce_b64();

        let result = unwrap_dek(wallet_id, network, password, &kdf, &aead_unwrap, &wrapped);
        assert!(
            result.is_err(),
            "unwrap should fail with mismatched AEAD params"
        );
    }

    #[test]
    fn test_unwrap_dek_fails_with_wrong_wallet_id() {
        let wallet_id = Uuid::new_v4();
        let network = Network::Testnet;
        let password = "test_password";
        let dek = generate_dek();
        let kdf = default_kdf_params();
        let aead = default_aead_params();

        let wrapped =
            wrap_dek(wallet_id, network, password, &kdf, &aead, &dek).expect("wrap should succeed");

        // Attempt unwrap with a different wallet_id (AAD mismatch)
        let wrong_wallet_id = Uuid::new_v4();

        let result = unwrap_dek(wrong_wallet_id, network, password, &kdf, &aead, &wrapped);
        assert!(
            result.is_err(),
            "unwrap should fail with wrong wallet_id (AAD binding)"
        );
    }
}
