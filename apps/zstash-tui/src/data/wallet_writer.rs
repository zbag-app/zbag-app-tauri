//! Write operations for wallet data.

#![allow(dead_code)] // Some functions are planned for future tasks

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use uuid::Uuid;
use zstash_engine::db::AppDb;

/// Update the birthday height for an account.
///
/// WARNING: This clears the scan queue, requiring a rescan from the new birthday.
/// Note: account_uuid should be the hex representation of the uuid (as returned by wallet_reader).
pub fn update_birthday_height(
    wallet_db: &Connection,
    account_uuid: &str,
    new_height: u32,
) -> Result<()> {
    // Start a transaction
    wallet_db.execute("BEGIN IMMEDIATE", [])?;

    let result = (|| -> Result<()> {
        // Update the birthday height in the accounts table
        // Note: uuid is stored as BLOB, so we compare against hex(uuid)
        let updated = wallet_db.execute(
            "UPDATE accounts SET birthday_height = ?1 WHERE hex(uuid) = ?2",
            params![new_height, account_uuid],
        )?;

        if updated == 0 {
            anyhow::bail!("account not found: {}", account_uuid);
        }

        // Clear the scan queue to force rescan from new birthday
        wallet_db.execute("DELETE FROM scan_queue", [])?;

        // Reset scan progress if it exists
        wallet_db
            .execute(
                "UPDATE scan_progress SET fully_scanned_height = NULL WHERE id = 1",
                [],
            )
            .ok(); // Ignore error if table doesn't exist

        Ok(())
    })();

    match result {
        Ok(()) => {
            wallet_db.execute("COMMIT", [])?;
            Ok(())
        }
        Err(e) => {
            wallet_db.execute("ROLLBACK", []).ok();
            Err(e)
        }
    }
}

/// Reset sync state to rescan from the account birthday.
pub fn reset_sync_state(wallet_db: &Connection) -> Result<()> {
    wallet_db.execute("BEGIN IMMEDIATE", [])?;

    let result = (|| -> Result<()> {
        // Clear scan queue
        wallet_db.execute("DELETE FROM scan_queue", [])?;

        // Reset scan progress
        wallet_db
            .execute(
                "UPDATE scan_progress SET fully_scanned_height = NULL WHERE id = 1",
                [],
            )
            .ok();

        // Clear blocks cache (optional - helps ensure fresh rescan)
        // wallet_db.execute("DELETE FROM blocks", [])?;

        Ok(())
    })();

    match result {
        Ok(()) => {
            wallet_db.execute("COMMIT", [])?;
            Ok(())
        }
        Err(e) => {
            wallet_db.execute("ROLLBACK", []).ok();
            Err(e)
        }
    }
}

/// Update wallet name in the app database.
pub fn update_wallet_name(app_db: &AppDb, wallet_id: Uuid, name: &str) -> Result<()> {
    app_db
        .conn()
        .execute(
            "UPDATE wallets SET name = ?1 WHERE id = ?2",
            params![name, wallet_id.to_string()],
        )
        .context("failed to update wallet name")?;
    Ok(())
}

/// Update remember_unlock setting in the app database.
pub fn set_remember_unlock(app_db: &AppDb, wallet_id: Uuid, enabled: bool) -> Result<()> {
    app_db
        .conn()
        .execute(
            "UPDATE wallets SET remember_unlock_enabled = ?1 WHERE id = ?2",
            params![enabled as i64, wallet_id.to_string()],
        )
        .context("failed to update remember_unlock setting")?;
    Ok(())
}
