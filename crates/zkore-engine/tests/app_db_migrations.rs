use std::path::PathBuf;

use rusqlite::Connection;
use uuid::Uuid;

use zkore_engine::db::migrations;

fn temp_db_path() -> PathBuf {
    std::env::temp_dir().join(format!("zkore_app_db_test_{}.sqlite", Uuid::new_v4()))
}

#[test]
fn app_db_initial_migration_creates_schema_and_seeds_servers() {
    let db_path = temp_db_path();

    migrations::migrate_with_rollback(&db_path).expect("migration should succeed");

    let conn = Connection::open(&db_path).expect("should open db");

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    for required in [
        "_app_migrations",
        "accounts",
        "backup_status",
        "receive_rotation",
        "servers",
        "swaps",
        "tor_settings",
        "wallet_encryption",
        "wallets",
    ] {
        assert!(tables.iter().any(|t| t == required), "missing table: {required}");
    }

    let server_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(server_count, 6);

    let mainnet_defaults: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM servers WHERE network='Mainnet' AND is_default=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(mainnet_defaults, 1);

    let testnet_defaults: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM servers WHERE network='Testnet' AND is_default=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(testnet_defaults, 1);
}

#[test]
fn app_db_migration_is_idempotent() {
    let db_path = temp_db_path();

    migrations::migrate_with_rollback(&db_path).expect("first migration should succeed");
    migrations::migrate_with_rollback(&db_path).expect("second migration should succeed");

    let conn = Connection::open(&db_path).expect("should open db");
    let server_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(server_count, 6);
}

