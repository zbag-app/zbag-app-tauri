//! Read operations for wallet data.

#![allow(dead_code)] // Some functions are planned for future tasks

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;
use uuid::Uuid;
use zstash_core::domain::{Network, WalletInfo};
use zstash_engine::db::AppDb;
use zstash_engine::db::wallet_meta;

/// Account information from the wallet database.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub id: u32,
    pub uuid: String,
    pub name: String,
    pub birthday_height: u32,
}

/// Sync state information.
#[derive(Debug, Clone)]
pub struct SyncState {
    pub chain_tip: Option<u32>,
    pub fully_scanned_height: Option<u32>,
    pub scan_ranges: Vec<ScanRange>,
}

/// A range of blocks to scan.
#[derive(Debug, Clone)]
pub struct ScanRange {
    pub start: u32,
    pub end: u32,
    pub priority: i32,
}

/// Open the app database.
pub fn open_app_db(path: &Path) -> Result<AppDb> {
    AppDb::open(path).context("failed to open app database")
}

/// List all wallets from the app database.
pub fn list_wallets(app_db: &AppDb) -> Result<Vec<WalletInfo>> {
    wallet_meta::list_wallets(app_db.conn()).context("failed to list wallets")
}

/// Get wallet info and directory path.
pub fn get_wallet(app_db: &AppDb, wallet_id: Uuid) -> Result<Option<(WalletInfo, String)>> {
    wallet_meta::get_wallet(app_db.conn(), wallet_id).context("failed to get wallet")
}

/// Get the wallet database path for a wallet.
pub fn wallet_db_path(wallets_dir: &Path, network: Network, wallet_id: Uuid) -> PathBuf {
    let network_dir = match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
    };
    wallets_dir
        .join(network_dir)
        .join(wallet_id.to_string())
        .join("wallet.sqlite")
}

/// Get accounts from the wallet database.
pub fn get_accounts(wallet_db: &Connection, _network: Network) -> Result<Vec<AccountInfo>> {
    // Query the zcash_client_sqlite accounts table
    // Note: uuid is stored as BLOB, so we use hex() to convert it to text
    let mut stmt = wallet_db
        .prepare("SELECT id, hex(uuid), name, birthday_height FROM accounts ORDER BY id")?;

    let accounts = stmt
        .query_map([], |row| {
            let id: u32 = row.get(0)?;
            Ok(AccountInfo {
                id,
                uuid: row.get(1)?,
                name: row
                    .get::<_, Option<String>>(2)?
                    .unwrap_or_else(|| format!("Account {id}")),
                birthday_height: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read accounts")?;

    Ok(accounts)
}

/// Get the sync state from the wallet database.
pub fn get_sync_state(wallet_db: &Connection) -> Result<SyncState> {
    // Get chain tip from blocks table
    let chain_tip: Option<u32> = wallet_db
        .query_row("SELECT MAX(height) FROM blocks", [], |row| row.get(0))
        .ok();

    // Get fully scanned height
    let fully_scanned_height: Option<u32> = wallet_db
        .query_row(
            "SELECT fully_scanned_height FROM scan_progress WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    // Get scan ranges
    let scan_ranges = wallet_db
        .prepare(
            "SELECT block_range_start, block_range_end, priority \
             FROM scan_queue ORDER BY priority DESC, block_range_start",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(ScanRange {
                    start: row.get(0)?,
                    end: row.get(1)?,
                    priority: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default();

    Ok(SyncState {
        chain_tip,
        fully_scanned_height,
        scan_ranges,
    })
}

/// Get account balance (simplified - returns zatoshis as i64).
pub fn get_account_balance(wallet_db: &Connection, account_id: u32) -> Result<i64> {
    // Sum up received notes minus spent notes for the account.
    // This is a simplified version - the real balance calculation is more complex.
    let balance: i64 = wallet_db
        .query_row(
            r"
            SELECT COALESCE(SUM(value), 0) FROM (
                SELECT value FROM sapling_received_notes
                WHERE account_id = ?1 AND spent IS NULL
                UNION ALL
                SELECT value FROM orchard_received_notes
                WHERE account_id = ?1 AND spent IS NULL
            )
            ",
            [account_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(balance)
}
