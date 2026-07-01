//! Unit tests for memo enhancement batch functions.
//!
//! These tests verify the batch query functions used during the enhancement phase:
//! - `count_txids_needing_memo_enhancement` - counts transactions needing memo fetching
//! - `get_txids_needing_memo_enhancement_batch` - retrieves txids with pagination
//!
//! The tests use a minimal mock database schema to isolate the query logic.

use std::path::PathBuf;

use rusqlite::Connection;
use tempfile::TempDir;

use zbag_engine::db::{OpenSqlcipherOptions, open_sqlcipher_db};
use zbag_engine::encryption::Dek;
use zbag_engine::sync_service::{
    count_txids_needing_memo_enhancement, get_txids_needing_memo_enhancement_batch,
};

/// Creates a test DEK for encryption.
fn test_dek() -> Dek {
    Dek([0x42u8; 32])
}

/// Creates a minimal mock wallet database with the required schema.
///
/// The schema mimics just enough of the `zcash_client_sqlite` wallet schema
/// to test the memo enhancement queries.
fn create_mock_wallet_db(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("wallet.db");
    let dek = test_dek();

    let conn = open_sqlcipher_db(
        &path,
        &dek,
        OpenSqlcipherOptions {
            create_if_missing: true,
            load_array_module: false,
        },
    )
    .expect("failed to create test db");

    // Create minimal schema matching the wallet database structure
    conn.execute_batch(
        r#"
        CREATE TABLE transactions (
            id_tx INTEGER PRIMARY KEY,
            txid BLOB NOT NULL UNIQUE
        );

        CREATE TABLE sapling_received_notes (
            id INTEGER PRIMARY KEY,
            transaction_id INTEGER NOT NULL REFERENCES transactions(id_tx),
            memo BLOB
        );

        CREATE TABLE orchard_received_notes (
            id INTEGER PRIMARY KEY,
            transaction_id INTEGER NOT NULL REFERENCES transactions(id_tx),
            memo BLOB
        );
        "#,
    )
    .expect("failed to create schema");

    path
}

/// Helper to insert a transaction with a given txid (32 bytes).
fn insert_transaction(conn: &Connection, txid: &[u8; 32]) -> i64 {
    conn.execute(
        "INSERT INTO transactions (txid) VALUES (?)",
        [txid.as_slice()],
    )
    .expect("insert transaction");
    conn.last_insert_rowid()
}

/// Helper to insert a sapling note with optional memo.
fn insert_sapling_note(conn: &Connection, tx_id: i64, memo: Option<&[u8]>) {
    conn.execute(
        "INSERT INTO sapling_received_notes (transaction_id, memo) VALUES (?, ?)",
        rusqlite::params![tx_id, memo],
    )
    .expect("insert sapling note");
}

/// Helper to insert an orchard note with optional memo.
fn insert_orchard_note(conn: &Connection, tx_id: i64, memo: Option<&[u8]>) {
    conn.execute(
        "INSERT INTO orchard_received_notes (transaction_id, memo) VALUES (?, ?)",
        rusqlite::params![tx_id, memo],
    )
    .expect("insert orchard note");
}

#[test]
fn count_txids_needing_memo_enhancement_returns_correct_count() {
    let dir = TempDir::new().expect("create temp dir");
    let path = create_mock_wallet_db(&dir);
    let dek = test_dek();

    // Open connection to insert test data
    let conn = open_sqlcipher_db(
        &path,
        &dek,
        OpenSqlcipherOptions {
            create_if_missing: false,
            load_array_module: false,
        },
    )
    .expect("open db");

    // Insert transactions:
    // - tx1: sapling note with NULL memo (needs enhancement)
    // - tx2: orchard note with NULL memo (needs enhancement)
    // - tx3: sapling note with memo (does NOT need enhancement)
    // - tx4: both sapling and orchard NULL memos (counts as 1 due to DISTINCT)
    let txid1 = [1u8; 32];
    let txid2 = [2u8; 32];
    let txid3 = [3u8; 32];
    let txid4 = [4u8; 32];

    let tx1_id = insert_transaction(&conn, &txid1);
    insert_sapling_note(&conn, tx1_id, None);

    let tx2_id = insert_transaction(&conn, &txid2);
    insert_orchard_note(&conn, tx2_id, None);

    let tx3_id = insert_transaction(&conn, &txid3);
    insert_sapling_note(&conn, tx3_id, Some(b"Hello"));

    let tx4_id = insert_transaction(&conn, &txid4);
    insert_sapling_note(&conn, tx4_id, None);
    insert_orchard_note(&conn, tx4_id, None);

    drop(conn); // Close connection before calling the function

    // Should count 3 distinct txids needing enhancement (tx1, tx2, tx4)
    let count = count_txids_needing_memo_enhancement(&path, &dek).expect("count");
    assert_eq!(count, 3, "expected 3 transactions needing enhancement");
}

#[test]
fn get_txids_needing_memo_enhancement_batch_respects_limit_offset() {
    let dir = TempDir::new().expect("create temp dir");
    let path = create_mock_wallet_db(&dir);
    let dek = test_dek();

    let conn = open_sqlcipher_db(
        &path,
        &dek,
        OpenSqlcipherOptions {
            create_if_missing: false,
            load_array_module: false,
        },
    )
    .expect("open db");

    // Insert 5 transactions with NULL memos
    for i in 0..5u8 {
        let mut txid = [0u8; 32];
        txid[0] = i + 1;
        let tx_id = insert_transaction(&conn, &txid);
        insert_sapling_note(&conn, tx_id, None);
    }

    drop(conn);

    // Get first batch of 2
    let batch1 = get_txids_needing_memo_enhancement_batch(&path, &dek, 0, 2).expect("batch1");
    assert_eq!(batch1.len(), 2, "first batch should have 2 txids");

    // Get second batch of 2 with offset
    let batch2 = get_txids_needing_memo_enhancement_batch(&path, &dek, 2, 2).expect("batch2");
    assert_eq!(batch2.len(), 2, "second batch should have 2 txids");

    // Get remaining with offset 4
    let batch3 = get_txids_needing_memo_enhancement_batch(&path, &dek, 4, 2).expect("batch3");
    assert_eq!(batch3.len(), 1, "third batch should have 1 txid");

    // Get batch beyond available data
    let batch4 = get_txids_needing_memo_enhancement_batch(&path, &dek, 10, 2).expect("batch4");
    assert!(batch4.is_empty(), "batch beyond data should be empty");

    // Verify all txids are unique across batches
    let mut all_txids: Vec<[u8; 32]> = Vec::new();
    all_txids.extend(batch1);
    all_txids.extend(batch2);
    all_txids.extend(batch3);
    assert_eq!(all_txids.len(), 5, "should have 5 total txids");

    // Check uniqueness
    let unique_count = {
        let mut set = std::collections::HashSet::new();
        for txid in &all_txids {
            set.insert(*txid);
        }
        set.len()
    };
    assert_eq!(unique_count, 5, "all txids should be unique");
}

#[test]
fn get_txids_needing_memo_enhancement_batch_handles_empty_database() {
    let dir = TempDir::new().expect("create temp dir");
    let path = create_mock_wallet_db(&dir);
    let dek = test_dek();

    // Query empty database
    let count = count_txids_needing_memo_enhancement(&path, &dek).expect("count");
    assert_eq!(count, 0, "empty database should have count 0");

    let batch = get_txids_needing_memo_enhancement_batch(&path, &dek, 0, 100).expect("batch");
    assert!(batch.is_empty(), "empty database should return empty batch");
}

#[test]
fn count_txids_uses_union_deduplication() {
    let dir = TempDir::new().expect("create temp dir");
    let path = create_mock_wallet_db(&dir);
    let dek = test_dek();

    let conn = open_sqlcipher_db(
        &path,
        &dek,
        OpenSqlcipherOptions {
            create_if_missing: false,
            load_array_module: false,
        },
    )
    .expect("open db");

    // Create a self-send scenario:
    // Same transaction has NULL memos in both sapling and orchard
    // UNION should deduplicate, counting it as 1
    let txid = [42u8; 32];
    let tx_id = insert_transaction(&conn, &txid);
    insert_sapling_note(&conn, tx_id, None);
    insert_orchard_note(&conn, tx_id, None);

    drop(conn);

    let count = count_txids_needing_memo_enhancement(&path, &dek).expect("count");
    assert_eq!(
        count, 1,
        "self-send with NULL memos in both pools should count as 1"
    );

    let batch = get_txids_needing_memo_enhancement_batch(&path, &dek, 0, 100).expect("batch");
    assert_eq!(batch.len(), 1, "should return exactly 1 txid");
    assert_eq!(batch[0], txid, "should be the correct txid");
}
