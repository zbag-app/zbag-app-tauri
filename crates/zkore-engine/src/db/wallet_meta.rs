use rusqlite::{Connection, params};
use uuid::Uuid;

use zkore_core::domain::{Network, WalletInfo, WalletType};

pub fn insert_wallet(
    conn: &Connection,
    wallet: &WalletInfo,
    directory_path: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO wallets (id, name, directory_path, wallet_type, network, remember_unlock_enabled, created_at, last_opened_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            wallet.id.to_string(),
            wallet.name,
            directory_path,
            format!("{:?}", wallet.wallet_type),
            format!("{:?}", wallet.network),
            wallet.remember_unlock_enabled as i64,
            wallet.created_at,
            wallet.last_opened_at,
        ],
    )?;
    Ok(())
}

pub fn get_wallet(
    conn: &Connection,
    wallet_id: Uuid,
) -> rusqlite::Result<Option<(WalletInfo, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, directory_path, wallet_type, network, remember_unlock_enabled, created_at, last_opened_at
         FROM wallets WHERE id = ?1",
    )?;
    let mut rows = stmt.query([wallet_id.to_string()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let id_str: String = row.get(0)?;
    let directory_path: String = row.get(2)?;
    let wallet_type: String = row.get(3)?;
    let network: String = row.get(4)?;

    let wallet = WalletInfo {
        id: Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        wallet_type: match wallet_type.as_str() {
            "Software" => WalletType::Software,
            "WatchOnly" => WalletType::WatchOnly,
            _ => WalletType::Software,
        },
        network: match network.as_str() {
            "Mainnet" => Network::Mainnet,
            "Testnet" => Network::Testnet,
            _ => Network::Testnet,
        },
        remember_unlock_enabled: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        last_opened_at: row.get(7)?,
    };

    Ok(Some((wallet, directory_path)))
}

pub fn list_wallets(conn: &Connection) -> rusqlite::Result<Vec<WalletInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, wallet_type, network, remember_unlock_enabled, created_at, last_opened_at FROM wallets ORDER BY created_at DESC",
    )?;
    let wallets = stmt
        .query_map([], |row| {
            let id_str: String = row.get(0)?;
            let wallet_type: String = row.get(2)?;
            let network: String = row.get(3)?;
            Ok(WalletInfo {
                id: Uuid::parse_str(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                name: row.get(1)?,
                wallet_type: match wallet_type.as_str() {
                    "Software" => WalletType::Software,
                    "WatchOnly" => WalletType::WatchOnly,
                    _ => WalletType::Software,
                },
                network: match network.as_str() {
                    "Mainnet" => Network::Mainnet,
                    "Testnet" => Network::Testnet,
                    _ => Network::Testnet,
                },
                remember_unlock_enabled: row.get::<_, i64>(4)? != 0,
                created_at: row.get(5)?,
                last_opened_at: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(wallets)
}

pub fn update_last_opened_at(
    conn: &Connection,
    wallet_id: Uuid,
    last_opened_at: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE wallets SET last_opened_at = ?2 WHERE id = ?1",
        params![wallet_id.to_string(), last_opened_at],
    )?;
    Ok(())
}

pub fn set_remember_unlock_enabled(
    conn: &Connection,
    wallet_id: Uuid,
    enabled: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE wallets SET remember_unlock_enabled = ?2 WHERE id = ?1",
        params![wallet_id.to_string(), enabled as i64],
    )?;
    Ok(())
}
