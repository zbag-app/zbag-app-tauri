use rusqlite::{Connection, params};

use zkore_core::domain::{TorState, TorStatus};

pub fn get_tor_state(conn: &Connection) -> rusqlite::Result<TorState> {
    let mut stmt =
        conn.prepare("SELECT enabled, status, last_error FROM tor_settings WHERE id = 1")?;
    let mut rows = stmt.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(TorState {
            enabled: false,
            status: TorStatus::Off,
            last_error: None,
        });
    };

    let enabled: i64 = row.get(0)?;
    let status: String = row.get(1)?;
    let last_error: Option<String> = row.get(2)?;

    Ok(TorState {
        enabled: enabled != 0,
        status: match status.as_str() {
            "Off" => TorStatus::Off,
            "Connecting" => TorStatus::Connecting,
            "On" => TorStatus::On,
            "Error" => TorStatus::Error,
            _ => TorStatus::Off,
        },
        last_error,
    })
}

pub fn upsert_tor_state(
    conn: &Connection,
    state: &TorState,
    updated_at_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO tor_settings (id, enabled, status, last_error, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
           enabled = excluded.enabled,
           status = excluded.status,
           last_error = excluded.last_error,
           updated_at = excluded.updated_at",
        params![
            state.enabled as i64,
            format!("{:?}", state.status),
            state.last_error,
            updated_at_ms,
        ],
    )?;
    Ok(())
}
