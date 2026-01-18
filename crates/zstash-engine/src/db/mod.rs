use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use rusqlite::{Connection, OpenFlags};
use zeroize::Zeroize;

use crate::encryption::Dek;
use zstash_core::permissions::{create_dir_all_secure, set_file_permissions};

/// SQLite busy timeout for concurrent operations (sync + tx).
///
/// 30 seconds allows long-running sync to complete before giving up.
///
/// Note: WAL mode (`PRAGMA journal_mode=WAL`) was considered but not added.
/// busy_timeout alone resolves SQLITE_BUSY errors for the current access patterns
/// (single writer, occasional concurrent readers). WAL mode could be added later
/// if concurrent read performance during writes becomes a concern.
pub const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// Options for opening an SQLCipher-encrypted wallet database.
#[derive(Default)]
pub struct OpenSqlcipherOptions {
    /// Create the database file if it doesn't exist.
    pub create_if_missing: bool,
    /// Load the SQLite array module (needed for certain queries).
    pub load_array_module: bool,
}

/// Opens an SQLCipher-encrypted wallet database with the given options.
///
/// This is the shared helper used by both sync_service and wallet_manager
/// to ensure consistent connection setup (busy_timeout, encryption key, etc.).
pub fn open_sqlcipher_db(
    path: &Path,
    dek: &Dek,
    options: OpenSqlcipherOptions,
) -> anyhow::Result<Connection> {
    let flags = if options.create_if_missing {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };

    let conn = Connection::open_with_flags(path, flags)
        .with_context(|| format!("failed to open wallet db: {}", path.display()))?;

    // Configure busy timeout early so subsequent statements retry instead of failing with
    // SQLITE_BUSY under contention.
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
        .context("failed to set busy_timeout")?;

    // `hex::encode` uses lowercase hex, matching the previous per-byte `format!("{byte:02x}")`
    // implementation. Use as_slice() to avoid copying the 32-byte key (arrays are Copy).
    let mut dek_hex = hex::encode(dek.0.as_slice());
    let mut pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");
    conn.execute_batch(&pragma)
        .context("failed to apply wallet db encryption key")?;

    dek_hex.zeroize();
    pragma.zeroize();

    if options.load_array_module {
        rusqlite::vtab::array::load_module(&conn).context("failed to load sqlite array module")?;
    }

    // Force an early read to detect an incorrect key.
    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
        .context("wallet db is not readable (incorrect key or corrupted db)")?;

    Ok(conn)
}

/// Opens the app metadata database with the standard connection configuration.
///
/// This is a shared helper to ensure consistent settings (busy_timeout, foreign_keys) across
/// all app-db connection points.
pub fn open_app_db_connection(path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .with_context(|| format!("failed to open app metadata db: {}", path.display()))?;

    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
        .context("failed to set busy_timeout")?;

    // PRAGMA statements return 0 rows; we only care about success/failure.
    conn.execute("PRAGMA foreign_keys = ON;", [])
        .context("failed to enable foreign_keys")?;

    Ok(conn)
}

pub mod account_meta;
pub mod backup_meta;
pub mod fiat_meta;
pub mod migrations;
pub mod rotation_meta;
pub mod schema;
pub mod server_meta;
pub mod swap_meta;
pub mod tor_meta;
pub mod wallet_encryption_meta;
pub mod wallet_meta;

#[derive(Debug)]
pub struct AppDb {
    path: PathBuf,
    conn: Connection,
}

impl AppDb {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            create_dir_all_secure(parent).with_context(|| {
                format!(
                    "failed to create app metadata db parent directory: {}",
                    parent.display()
                )
            })?;
        }

        migrations::migrate_with_rollback(&path)?;

        let conn = open_app_db_connection(&path)?;

        // Set secure file permissions on the database file (0600 on Unix)
        set_file_permissions(&path).with_context(|| {
            format!(
                "failed to set permissions on app metadata db: {}",
                path.display()
            )
        })?;

        Ok(Self { path, conn })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
