use rusqlite::{Connection, params, types::Type};
use uuid::Uuid;

pub fn get_next_diversifier_index(
    conn: &Connection,
    wallet_id: Uuid,
    account_id: u32,
) -> rusqlite::Result<Option<u64>> {
    let mut stmt = conn.prepare(
        "SELECT diversifier_index FROM receive_rotation WHERE wallet_id = ?1 AND account_id = ?2",
    )?;
    let mut rows = stmt.query(params![wallet_id.to_string(), account_id as i64])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let stored: i64 = row.get(0)?;
    let value = u64::try_from(stored).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, Type::Integer, Box::new(err))
    })?;
    Ok(Some(value))
}

pub fn set_next_diversifier_index(
    conn: &Connection,
    wallet_id: Uuid,
    account_id: u32,
    next_diversifier_index: u64,
    updated_at: i64,
) -> rusqlite::Result<()> {
    let di_i64 = i64::try_from(next_diversifier_index)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;

    conn.execute(
        "INSERT INTO receive_rotation (wallet_id, account_id, diversifier_index, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(wallet_id, account_id) DO UPDATE SET diversifier_index=excluded.diversifier_index, created_at=excluded.created_at",
        params![wallet_id.to_string(), account_id as i64, di_i64, updated_at],
    )?;
    Ok(())
}
