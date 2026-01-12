//! Quick tool to decrypt wallet and print DEK or query accounts.
//!
//! Usage: cargo run -p zstash-tui --example decrypt_wallet

use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::Connection;
use zstash_core::domain::Network;
use zstash_engine::db::AppDb;
use zstash_engine::db::wallet_encryption_meta::get_wallet_encryption;
use zstash_engine::encryption::unwrap_dek;

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .expect("HOME env var")
}

fn main() -> Result<()> {
    let wallet_name = std::env::args().nth(1).unwrap_or_else(|| "3".to_string());
    let password = std::env::args().nth(2).unwrap_or_else(|| "pw".to_string());

    let app_db_path = home_dir().join(".zstash/app.db");

    println!("Opening app.db: {}", app_db_path.display());
    let app_db = AppDb::open(&app_db_path)?;

    // Find wallet by name
    let wallet: (String, String, String) = app_db
        .conn()
        .query_row(
            "SELECT id, name, network FROM wallets WHERE name = ?1",
            [&wallet_name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .context("wallet not found")?;

    let wallet_id: uuid::Uuid = wallet.0.parse()?;
    let network = if wallet.2 == "Testnet" {
        Network::Testnet
    } else {
        Network::Mainnet
    };

    println!("Wallet: {} ({})", wallet.1, wallet_id);
    println!("Network: {:?}", network);

    // Get encryption metadata
    let meta =
        get_wallet_encryption(app_db.conn(), wallet_id)?.context("no encryption metadata")?;

    println!("KDF salt: {}", meta.kdf.salt_b64);
    println!("Deriving DEK with password '{}'...", password);

    // Derive DEK
    let dek = unwrap_dek(
        wallet_id,
        network,
        &password,
        &meta.kdf.salt_b64,
        &meta.aead.nonce_b64,
        &meta.wrapped_dek_b64,
    )
    .context("failed to unwrap DEK - wrong password?")?;

    let dek_hex: String = dek.0.iter().map(|b| format!("{b:02x}")).collect();
    println!("\nDEK (hex): {}", dek_hex);
    println!("\nFor sqlcipher, use:");
    println!("  PRAGMA key = \"x'{}'\";\n", dek_hex);

    // Now open the wallet DB and query accounts
    let wallet_db_path: PathBuf = home_dir()
        .join(".zstash/wallets")
        .join(if network == Network::Testnet {
            "testnet"
        } else {
            "mainnet"
        })
        .join(wallet_id.to_string())
        .join("wallet.sqlite");

    println!("Opening wallet DB: {}", wallet_db_path.display());

    let conn = Connection::open(&wallet_db_path)?;
    let pragma = format!("PRAGMA key = \"x'{dek_hex}'\";");
    conn.execute_batch(&pragma)?;

    // Verify key works
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))?;
    println!("sqlite_master tables: {}", count);

    // Query accounts
    println!("\n=== ACCOUNTS TABLE ===");
    let mut stmt =
        conn.prepare("SELECT id, hex(uuid), name, birthday_height FROM accounts ORDER BY id")?;
    let accounts: Vec<(u32, String, Option<String>, u32)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if accounts.is_empty() {
        println!("(empty - no accounts found!)");
    } else {
        for (id, uuid, name, birthday) in &accounts {
            println!(
                "  id={}, uuid={}, name={:?}, birthday_height={}",
                id, uuid, name, birthday
            );
        }
    }

    // Also show raw row count
    let row_count: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
    println!("\nTotal rows in accounts: {}", row_count);

    // Check if accounts table exists and its schema
    println!("\n=== ACCOUNTS SCHEMA ===");
    let schema: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='accounts'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "(table not found)".to_string());
    println!("{}", schema);

    // Check all tables
    println!("\n=== ALL TABLES ===");
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for t in &tables {
        println!("  {}", t);
    }

    Ok(())
}
