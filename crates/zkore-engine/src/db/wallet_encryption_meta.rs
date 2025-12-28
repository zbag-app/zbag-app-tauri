use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::encryption::{WalletAeadParams, WalletKdfParams};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletEncryptionMeta {
    pub kdf: WalletKdfParams,
    pub aead: WalletAeadParams,
    pub wrapped_dek_b64: String,
}

pub fn insert_wallet_encryption(
    conn: &Connection,
    wallet_id: Uuid,
    meta: &WalletEncryptionMeta,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO wallet_encryption (
            wallet_id,
            kdf_algorithm,
            kdf_version,
            kdf_memory_mib,
            kdf_iterations,
            kdf_parallelism,
            kdf_salt,
            wrapped_dek,
            aead_scheme,
            aead_version,
            aead_nonce
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            wallet_id.to_string(),
            meta.kdf.algorithm,
            meta.kdf.version as i64,
            meta.kdf.memory_mib as i64,
            meta.kdf.iterations as i64,
            meta.kdf.parallelism as i64,
            meta.kdf.salt_b64,
            meta.wrapped_dek_b64,
            meta.aead.scheme,
            meta.aead.version as i64,
            meta.aead.nonce_b64,
        ],
    )?;
    Ok(())
}

pub fn get_wallet_encryption(
    conn: &Connection,
    wallet_id: Uuid,
) -> rusqlite::Result<Option<WalletEncryptionMeta>> {
    let mut stmt = conn.prepare(
        "SELECT
            kdf_algorithm,
            kdf_version,
            kdf_memory_mib,
            kdf_iterations,
            kdf_parallelism,
            kdf_salt,
            wrapped_dek,
            aead_scheme,
            aead_version,
            aead_nonce
        FROM wallet_encryption
        WHERE wallet_id = ?1",
    )?;

    let mut rows = stmt.query([wallet_id.to_string()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let aead_nonce: Option<String> = row.get(9)?;
    let Some(aead_nonce) = aead_nonce else {
        return Ok(None);
    };

    Ok(Some(WalletEncryptionMeta {
        kdf: WalletKdfParams {
            algorithm: row.get(0)?,
            version: row.get::<_, i64>(1)? as u32,
            memory_mib: row.get::<_, i64>(2)? as u32,
            iterations: row.get::<_, i64>(3)? as u32,
            parallelism: row.get::<_, i64>(4)? as u32,
            salt_b64: row.get(5)?,
        },
        wrapped_dek_b64: row.get(6)?,
        aead: WalletAeadParams {
            scheme: row.get(7)?,
            version: row.get::<_, i64>(8)? as u32,
            nonce_b64: aead_nonce,
        },
    }))
}
