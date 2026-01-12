//! Encryption utilities for accessing wallet databases.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use uuid::Uuid;
use zeroize::Zeroize;
use zstash_core::domain::Network;
use zstash_engine::db::wallet_encryption_meta::get_wallet_encryption;
use zstash_engine::encryption::{Dek, unwrap_dek};

/// Derive the DEK (Data Encryption Key) for a wallet using the password.
pub fn derive_wallet_dek(
    app_db: &Connection,
    wallet_id: Uuid,
    network: Network,
    password: &str,
) -> Result<Dek> {
    let meta = get_wallet_encryption(app_db, wallet_id)
        .context("failed to query wallet encryption metadata")?
        .ok_or_else(|| anyhow::anyhow!("wallet encryption metadata not found"))?;

    unwrap_dek(
        wallet_id,
        network,
        password,
        &meta.kdf.salt_b64,
        &meta.aead.nonce_b64,
        &meta.wrapped_dek_b64,
    )
    .context("failed to unwrap DEK (wrong password?)")
}

/// Open an encrypted wallet database using the DEK.
pub fn open_wallet_db(wallet_db_path: &Path, dek: &Dek) -> Result<Connection> {
    let conn = Connection::open_with_flags(wallet_db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .with_context(|| format!("failed to open wallet db: {}", wallet_db_path.display()))?;

    // Format DEK as hex and apply via PRAGMA key
    let mut dek_hex: String = dek.0.iter().map(|b| format!("{b:02x}")).collect();
    let mut pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");

    conn.execute_batch(&pragma)
        .context("failed to apply wallet db encryption key")?;

    // Zeroize sensitive strings
    dek_hex.zeroize();
    pragma.zeroize();

    // Validate the key by reading from sqlite_master
    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
        .context("failed to verify wallet db key (wrong password?)")?;

    // Load array extension for zcash_client_sqlite compatibility
    rusqlite::vtab::array::load_module(&conn).context("failed to load array module")?;

    Ok(conn)
}
