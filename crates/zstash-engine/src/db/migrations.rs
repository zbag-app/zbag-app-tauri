use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use rusqlite::{Connection, OpenFlags, params};

use super::schema::INITIAL_SCHEMA_V1;

pub const LATEST_VERSION: i64 = 2;

pub fn migrate_with_rollback(db_path: &Path) -> anyhow::Result<()> {
    let existed = db_path.exists();
    let snapshot_path = snapshot_path(db_path);

    if existed {
        std::fs::copy(db_path, &snapshot_path).with_context(|| {
            format!(
                "failed to create pre-migration snapshot: {} -> {}",
                db_path.display(),
                snapshot_path.display()
            )
        })?;
    }

    let migrate_result = (|| {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .with_context(|| format!("failed to open app metadata db: {}", db_path.display()))?;

        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .context("failed to enable foreign_keys")?;

        apply_migrations(&conn)?;
        Ok::<(), anyhow::Error>(())
    })();

    match migrate_result {
        Ok(()) => {
            if existed && let Err(e) = std::fs::remove_file(&snapshot_path) {
                tracing::debug!(path = ?snapshot_path, error = ?e, "failed to cleanup snapshot file");
            }
            Ok(())
        }
        Err(err) => {
            if existed {
                let restore_result = std::fs::copy(&snapshot_path, db_path).with_context(|| {
                    format!(
                        "failed to restore pre-migration snapshot: {} -> {}",
                        snapshot_path.display(),
                        db_path.display()
                    )
                });
                if let Err(e) = std::fs::remove_file(&snapshot_path) {
                    tracing::debug!(path = ?snapshot_path, error = ?e, "failed to cleanup snapshot file");
                }
                if let Err(restore_err) = restore_result {
                    bail!("{err}\n{restore_err}");
                }
            }
            Err(err)
        }
    }
}

fn snapshot_path(db_path: &Path) -> PathBuf {
    let mut snapshot = db_path.as_os_str().to_os_string();
    snapshot.push(".pre_migration");
    PathBuf::from(snapshot)
}

pub fn apply_migrations(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .context("failed to enable foreign_keys")?;

    let current_version = current_version(conn)?;
    if current_version > LATEST_VERSION {
        bail!(
            "app metadata db version {} is newer than supported {}",
            current_version,
            LATEST_VERSION
        );
    }

    if current_version < 1 {
        apply_v1(conn)?;
        record_version(conn, 1)?;
    }

    if current_version < 2 {
        apply_v2(conn)?;
        record_version(conn, 2)?;
    }

    Ok(())
}

fn current_version(conn: &Connection) -> anyhow::Result<i64> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _app_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);",
    )
    .context("failed to ensure _app_migrations table")?;

    let mut stmt = conn
        .prepare("SELECT COALESCE(MAX(version), 0) FROM _app_migrations")
        .context("failed to prepare version query")?;
    let version: i64 = stmt
        .query_row([], |row| row.get(0))
        .context("failed to query current migration version")?;
    Ok(version)
}

fn record_version(conn: &Connection, version: i64) -> anyhow::Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO _app_migrations(version, applied_at) VALUES (?1, ?2)",
        params![version, now_ms],
    )
    .with_context(|| format!("failed to record migration version {version}"))?;
    Ok(())
}

fn apply_v1(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(INITIAL_SCHEMA_V1)
        .context("failed to apply initial schema")?;

    seed_servers_v1(conn)?;
    seed_tor_settings_v1(conn)?;

    Ok(())
}

fn apply_v2(_conn: &Connection) -> anyhow::Result<()> {
    // No-op migration for forward compatibility.
    // Databases may have been upgraded to v2 by experimental code that was
    // never merged. The v2 migration made no schema changes.
    Ok(())
}

fn seed_servers_v1(conn: &Connection) -> anyhow::Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();

    // Mainnet default
    conn.execute(
        "INSERT OR IGNORE INTO servers (id, name, grpc_url, network, is_default, last_success_at, created_at)
         VALUES (?1, ?2, ?3, 'Mainnet', 1, NULL, ?4)",
        params![
            "00000000-0000-0000-0000-000000000001",
            "lwd.zec.pro",
            "https://lwd.zec.pro",
            now_ms
        ],
    )
    .context("failed to seed mainnet default server")?;

    // Mainnet non-defaults
    for (id, name, url) in [
        (
            "00000000-0000-0000-0000-000000000002",
            "zec.rocks",
            "https://zec.rocks",
        ),
        (
            "00000000-0000-0000-0000-000000000003",
            "na.zec.rocks",
            "https://na.zec.rocks",
        ),
        (
            "00000000-0000-0000-0000-000000000004",
            "eu.zec.rocks",
            "https://eu.zec.rocks",
        ),
        (
            "00000000-0000-0000-0000-000000000005",
            "sa.zec.rocks",
            "https://sa.zec.rocks",
        ),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO servers (id, name, grpc_url, network, is_default, last_success_at, created_at)
             VALUES (?1, ?2, ?3, 'Mainnet', 0, NULL, ?4)",
            params![id, name, url, now_ms],
        )
        .with_context(|| format!("failed to seed mainnet server {url}"))?;
    }

    // Testnet default
    conn.execute(
        "INSERT OR IGNORE INTO servers (id, name, grpc_url, network, is_default, last_success_at, created_at)
         VALUES (?1, ?2, ?3, 'Testnet', 1, NULL, ?4)",
        params![
            "00000000-0000-0000-0000-000000000006",
            "lwd.testnet.zec.pro",
            "https://lwd.testnet.zec.pro",
            now_ms
        ],
    )
    .context("failed to seed testnet default server")?;

    Ok(())
}

fn seed_tor_settings_v1(conn: &Connection) -> anyhow::Result<()> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO tor_settings (id, enabled, status, last_error, updated_at)
         VALUES (1, 0, 'Off', NULL, ?1)",
        params![now_ms],
    )
    .context("failed to seed tor_settings")?;
    Ok(())
}
