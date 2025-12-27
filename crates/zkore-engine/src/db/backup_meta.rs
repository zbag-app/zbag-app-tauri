use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn set_backup_required(conn: &Connection, wallet_id: Uuid, required: bool) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO backup_status (wallet_id, backup_required) VALUES (?1, ?2)
         ON CONFLICT(wallet_id) DO UPDATE SET backup_required=excluded.backup_required",
        params![wallet_id.to_string(), required as i64],
    )?;
    Ok(())
}

pub fn mark_backup_complete(
    conn: &Connection,
    wallet_id: Uuid,
    completed_at: i64,
    verification_method: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO backup_status (wallet_id, backup_required, backup_completed_at, verification_method)
         VALUES (?1, 0, ?2, ?3)
         ON CONFLICT(wallet_id) DO UPDATE SET backup_required=0, backup_completed_at=excluded.backup_completed_at, verification_method=excluded.verification_method",
        params![wallet_id.to_string(), completed_at, verification_method],
    )?;
    Ok(())
}

pub fn get_backup_required(conn: &Connection, wallet_id: Uuid) -> rusqlite::Result<Option<bool>> {
    let mut stmt = conn.prepare("SELECT backup_required FROM backup_status WHERE wallet_id = ?1")?;
    let mut rows = stmt.query([wallet_id.to_string()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let required: i64 = row.get(0)?;
    Ok(Some(required != 0))
}
