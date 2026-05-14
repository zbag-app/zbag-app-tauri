mod common;

use zstash_engine::db::{migrations, open_app_db_connection};

#[test]
fn app_db_initial_migration_creates_schema_and_seeds_servers() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_test");

    migrations::migrate_with_rollback(&db_path).expect("migration should succeed");

    let conn = open_app_db_connection(&db_path).expect("should open db");

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
        assert!(
            tables.iter().any(|t| t == required),
            "missing table: {required}"
        );
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
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_test");

    migrations::migrate_with_rollback(&db_path).expect("first migration should succeed");
    migrations::migrate_with_rollback(&db_path).expect("second migration should succeed");

    let conn = open_app_db_connection(&db_path).expect("should open db");
    let server_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(server_count, 6);
}

#[test]
fn app_db_v2_migration_creates_fiat_settings_table() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_test");

    migrations::migrate_with_rollback(&db_path).expect("migration should succeed");

    let conn = open_app_db_connection(&db_path).expect("should open db");

    // Verify fiat_settings table exists
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert!(
        tables.iter().any(|t| t == "fiat_settings"),
        "missing table: fiat_settings"
    );

    // Verify default row was seeded
    let row_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM fiat_settings", [], |row| row.get(0))
        .unwrap();
    assert_eq!(row_count, 1, "fiat_settings should have exactly one row");

    // Verify default values
    let (enabled, currency, privacy_acknowledged): (i64, String, i64) = conn
        .query_row(
            "SELECT enabled, currency, privacy_acknowledged FROM fiat_settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(
        enabled, 0,
        "fiat_settings.enabled should default to 0 (false)"
    );
    assert_eq!(
        currency, "USD",
        "fiat_settings.currency should default to USD"
    );
    assert_eq!(
        privacy_acknowledged, 0,
        "fiat_settings.privacy_acknowledged should default to 0 (false)"
    );
}

#[test]
fn app_db_v2_migration_fiat_settings_schema_correct() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_test");

    migrations::migrate_with_rollback(&db_path).expect("migration should succeed");

    let conn = open_app_db_connection(&db_path).expect("should open db");

    // Verify table schema using PRAGMA table_info
    let columns: Vec<(String, String, i64)> = conn
        .prepare("PRAGMA table_info(fiat_settings)")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?, // name
                row.get::<_, String>(2)?, // type
                row.get::<_, i64>(3)?,    // notnull
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    // Expected columns: id, enabled, currency, privacy_acknowledged, updated_at
    assert_eq!(columns.len(), 5, "fiat_settings should have 5 columns");

    let column_names: Vec<&str> = columns.iter().map(|(name, _, _)| name.as_str()).collect();
    assert!(column_names.contains(&"id"), "missing column: id");
    assert!(column_names.contains(&"enabled"), "missing column: enabled");
    assert!(
        column_names.contains(&"currency"),
        "missing column: currency"
    );
    assert!(
        column_names.contains(&"privacy_acknowledged"),
        "missing column: privacy_acknowledged"
    );
    assert!(
        column_names.contains(&"updated_at"),
        "missing column: updated_at"
    );

    // Verify NOT NULL constraints (except id, which is INTEGER PRIMARY KEY and implicitly non-null)
    for (name, _, notnull) in &columns {
        if name != "id" {
            assert_eq!(*notnull, 1, "column {name} should have NOT NULL constraint");
        }
    }
}
