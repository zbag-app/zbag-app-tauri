use rusqlite::{Connection, params};
use uuid::Uuid;

use zstash_core::domain::{Network, ServerInfo};

pub fn insert_server(
    conn: &Connection,
    server: &ServerInfo,
    created_at: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO servers (id, name, grpc_url, network, is_default, last_success_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            server.id.to_string(),
            server.name,
            server.grpc_url,
            format!("{:?}", server.network),
            server.is_default as i64,
            server.last_success_at,
            created_at,
        ],
    )?;
    Ok(())
}

pub fn get_server(conn: &Connection, server_id: Uuid) -> rusqlite::Result<Option<ServerInfo>> {
    let mut stmt =
        conn.prepare("SELECT id, name, grpc_url, network, is_default, last_success_at FROM servers WHERE id = ?1")?;
    let mut rows = stmt.query([server_id.to_string()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let id_str: String = row.get(0)?;
    let network: String = row.get(3)?;
    Ok(Some(ServerInfo {
        id: Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        grpc_url: row.get(2)?,
        network: match network.as_str() {
            "Mainnet" => Network::Mainnet,
            "Testnet" => Network::Testnet,
            _ => Network::Testnet,
        },
        is_default: row.get::<_, i64>(4)? != 0,
        last_success_at: row.get(5)?,
    }))
}

pub fn list_servers(conn: &Connection) -> rusqlite::Result<Vec<ServerInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, grpc_url, network, is_default, last_success_at FROM servers ORDER BY network ASC, is_default DESC, created_at ASC",
    )?;
    let servers = stmt
        .query_map([], |row| {
            let id_str: String = row.get(0)?;
            let network: String = row.get(3)?;
            Ok(ServerInfo {
                id: Uuid::parse_str(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                name: row.get(1)?,
                grpc_url: row.get(2)?,
                network: match network.as_str() {
                    "Mainnet" => Network::Mainnet,
                    "Testnet" => Network::Testnet,
                    _ => Network::Testnet,
                },
                is_default: row.get::<_, i64>(4)? != 0,
                last_success_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(servers)
}

pub fn set_default_server(conn: &mut Connection, server_id: Uuid) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    let network: String = tx.query_row(
        "SELECT network FROM servers WHERE id = ?1",
        [server_id.to_string()],
        |row| row.get(0),
    )?;

    tx.execute(
        "UPDATE servers SET is_default = 0 WHERE network = ?1",
        params![network],
    )?;
    tx.execute(
        "UPDATE servers SET is_default = 1 WHERE id = ?1",
        params![server_id.to_string()],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn update_last_success_at(
    conn: &Connection,
    server_id: Uuid,
    last_success_at: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE servers SET last_success_at = ?2 WHERE id = ?1",
        params![server_id.to_string(), last_success_at],
    )?;
    Ok(())
}
