use rusqlite::{Connection, params};
use uuid::Uuid;

use zkore_core::domain::{AccountInfo, AccountType};

pub fn upsert_account(
    conn: &Connection,
    wallet_id: Uuid,
    account: &AccountInfo,
    created_at: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO accounts (wallet_id, account_id, name, account_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(wallet_id, account_id) DO UPDATE SET name=excluded.name, account_type=excluded.account_type",
        params![
            wallet_id.to_string(),
            account.id as i64,
            account.name,
            format!("{:?}", account.account_type),
            created_at,
        ],
    )?;
    Ok(())
}

pub fn list_accounts(conn: &Connection, wallet_id: Uuid) -> rusqlite::Result<Vec<AccountInfo>> {
    let mut stmt = conn.prepare(
        "SELECT account_id, name, account_type FROM accounts WHERE wallet_id = ?1 ORDER BY account_id ASC",
    )?;
    let accounts = stmt
        .query_map([wallet_id.to_string()], |row| {
            let account_type: String = row.get(2)?;
            Ok(AccountInfo {
                id: row.get::<_, i64>(0)? as u32,
                name: row.get(1)?,
                account_type: match account_type.as_str() {
                    "Software" => AccountType::Software,
                    "WatchOnly" => AccountType::WatchOnly,
                    "HardwareSigner" => AccountType::HardwareSigner,
                    _ => AccountType::Software,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(accounts)
}
