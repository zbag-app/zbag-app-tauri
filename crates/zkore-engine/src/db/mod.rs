use std::path::{Path, PathBuf};

use anyhow::Context as _;
use rusqlite::{Connection, OpenFlags};

pub mod account_meta;
pub mod backup_meta;
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
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create app metadata db parent directory: {}",
                    parent.display()
                )
            })?;
        }

        migrations::migrate_with_rollback(&path)?;

        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .with_context(|| format!("failed to open app metadata db: {}", path.display()))?;

        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .context("failed to enable foreign_keys")?;

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
