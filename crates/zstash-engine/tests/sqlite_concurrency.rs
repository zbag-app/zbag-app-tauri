//! Tests that verify busy_timeout prevents SQLITE_BUSY errors under concurrent access.

mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use zstash_engine::db::{
    OpenSqlcipherOptions, SQLITE_BUSY_TIMEOUT, open_app_db_connection, open_sqlcipher_db,
};
use zstash_engine::encryption;

/// Opens a plain (unencrypted) SQLite database connection for test fixtures.
///
/// Note: this intentionally doesn't reuse `open_app_db_connection` since these tests create an
/// ad-hoc schema and don't want app-db specific PRAGMAs/migration behavior.
fn open_test_db(path: &Path, create: bool) -> Connection {
    let flags = if create {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let conn = Connection::open_with_flags(path, flags).expect("open db");
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
        .expect("set busy_timeout");
    assert_busy_timeout_is_configured(&conn);
    conn
}

fn assert_busy_timeout_is_configured(conn: &Connection) {
    let busy_timeout_ms: i64 = conn
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .expect("query PRAGMA busy_timeout");
    let expected_ms =
        i64::try_from(SQLITE_BUSY_TIMEOUT.as_millis()).expect("busy_timeout fits in i64");
    assert_eq!(
        busy_timeout_ms, expected_ms,
        "expected PRAGMA busy_timeout to match SQLITE_BUSY_TIMEOUT"
    );
}

#[test]
fn busy_timeout_constant_is_30_seconds() {
    assert_eq!(SQLITE_BUSY_TIMEOUT, Duration::from_secs(30));
}

/// Tests that concurrent readers and writers don't get SQLITE_BUSY errors
/// when busy_timeout is properly configured.
#[test]
fn concurrent_access_with_busy_timeout_succeeds() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_concurrency_test");

    // Create the database and table.
    {
        let conn = open_test_db(&db_path, true);
        conn.execute(
            "CREATE TABLE test_data (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)",
            [],
        )
        .expect("create table");
        conn.execute("INSERT INTO test_data (id, value) VALUES (1, 0)", [])
            .expect("insert initial row");
    }

    let num_threads: u32 = 4;
    let iterations_per_thread: u32 = 50;
    let success_count = Arc::new(AtomicU32::new(0));
    let error_count = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();

    for thread_id in 0..num_threads {
        let path = db_path.clone();
        let successes = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);

        let handle = thread::spawn(move || {
            let conn = open_test_db(&path, false);

            for i in 0..iterations_per_thread {
                // Mix of reads and writes to create contention.
                if i % 2 == 0 {
                    // Write operation.
                    match conn.execute("UPDATE test_data SET value = value + 1 WHERE id = 1", []) {
                        Ok(_) => {
                            successes.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            // Check if it's a SQLITE_BUSY error.
                            if e.to_string().contains("database is locked") {
                                errors.fetch_add(1, Ordering::SeqCst);
                            } else {
                                panic!("thread {thread_id}: unexpected error: {e}");
                            }
                        }
                    }
                } else {
                    // Read operation.
                    match conn.query_row("SELECT value FROM test_data WHERE id = 1", [], |row| {
                        row.get::<_, i64>(0)
                    }) {
                        Ok(_) => {
                            successes.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            if e.to_string().contains("database is locked") {
                                errors.fetch_add(1, Ordering::SeqCst);
                            } else {
                                panic!("thread {thread_id}: unexpected error: {e}");
                            }
                        }
                    }
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread should complete");
    }

    let total_successes = success_count.load(Ordering::SeqCst);
    let total_errors = error_count.load(Ordering::SeqCst);
    let expected_ops: u32 = num_threads * iterations_per_thread;

    assert_eq!(
        total_errors, 0,
        "no SQLITE_BUSY errors should occur with busy_timeout; got {total_errors} errors"
    );
    assert_eq!(
        total_successes, expected_ops,
        "all operations should succeed; got {total_successes}/{expected_ops}"
    );
}

#[test]
fn app_db_busy_timeout_waits_for_lock_release() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_concurrency_test");

    {
        let conn = open_app_db_connection(&db_path).expect("open db");
        assert_busy_timeout_is_configured(&conn);
        conn.execute(
            "CREATE TABLE test_data (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)",
            [],
        )
        .expect("create table");
        conn.execute("INSERT INTO test_data (id, value) VALUES (1, 0)", [])
            .expect("insert initial row");
    }

    let mut conn_a = open_app_db_connection(&db_path).expect("open conn A");
    assert_busy_timeout_is_configured(&conn_a);

    let barrier = Arc::new(Barrier::new(2));
    let barrier_b = Arc::clone(&barrier);
    let path_b = db_path.clone();

    let tx = conn_a
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("begin immediate transaction");
    tx.execute("UPDATE test_data SET value = value WHERE id = 1", [])
        .expect("acquire write lock");

    let handle = thread::spawn(move || {
        let conn_b = open_app_db_connection(&path_b).expect("open conn B");
        assert_busy_timeout_is_configured(&conn_b);
        barrier_b.wait();
        conn_b.execute("UPDATE test_data SET value = value + 1 WHERE id = 1", [])
    });

    barrier.wait();
    // Hold the lock long enough that the other thread must wait (not race the commit).
    // If this ever flakes on heavily loaded CI, increase this delay.
    thread::sleep(Duration::from_millis(300));
    tx.commit().expect("commit");

    handle
        .join()
        .expect("thread should complete")
        .expect("busy_timeout should wait for lock and succeed");
}

#[test]
fn app_db_busy_timeout_eventually_fails_when_lock_is_held() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_concurrency_test");

    {
        let conn = open_app_db_connection(&db_path).expect("open db");
        conn.execute(
            "CREATE TABLE test_data (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)",
            [],
        )
        .expect("create table");
        conn.execute("INSERT INTO test_data (id, value) VALUES (1, 0)", [])
            .expect("insert initial row");
    }

    let short_timeout = Duration::from_millis(200);
    let expected_ms = i64::try_from(short_timeout.as_millis()).expect("busy_timeout fits in i64");

    let mut conn_a = open_app_db_connection(&db_path).expect("open conn A");
    conn_a
        .busy_timeout(short_timeout)
        .expect("set short busy_timeout");
    let conn_a_timeout_ms: i64 = conn_a
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .expect("query PRAGMA busy_timeout");
    assert_eq!(
        conn_a_timeout_ms, expected_ms,
        "short busy_timeout should be applied"
    );

    let barrier = Arc::new(Barrier::new(2));
    let barrier_b = Arc::clone(&barrier);
    let path_b = db_path.clone();

    let tx = conn_a
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("begin immediate transaction");
    tx.execute("UPDATE test_data SET value = value WHERE id = 1", [])
        .expect("acquire write lock");

    let handle = thread::spawn(move || {
        let conn_b = open_app_db_connection(&path_b).expect("open conn B");
        conn_b
            .busy_timeout(short_timeout)
            .expect("set short busy_timeout");
        let conn_b_timeout_ms: i64 = conn_b
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("query PRAGMA busy_timeout");
        assert_eq!(
            conn_b_timeout_ms, expected_ms,
            "short busy_timeout should be applied"
        );

        barrier_b.wait();
        conn_b.execute("UPDATE test_data SET value = value + 1 WHERE id = 1", [])
    });

    barrier.wait();

    let err = handle
        .join()
        .expect("thread should complete")
        .expect_err("short busy_timeout should return SQLITE_BUSY while lock is held");
    assert!(
        err.to_string().contains("database is locked"),
        "expected SQLITE_BUSY error; got {err}"
    );

    tx.commit().expect("commit");
}

/// Tests that concurrent access succeeds for SQLCipher-encrypted databases when busy_timeout is
/// configured via `open_sqlcipher_db`.
#[test]
fn concurrent_sqlcipher_access_with_busy_timeout_succeeds() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_concurrency_test");
    let dek = Arc::new(encryption::generate_dek());

    // Create the encrypted database and table.
    {
        let conn = open_sqlcipher_db(
            &db_path,
            dek.as_ref(),
            OpenSqlcipherOptions {
                create_if_missing: true,
                load_array_module: false,
            },
        )
        .expect("open encrypted db");
        assert_busy_timeout_is_configured(&conn);
        conn.execute(
            "CREATE TABLE test_data (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)",
            [],
        )
        .expect("create table");
        conn.execute("INSERT INTO test_data (id, value) VALUES (1, 0)", [])
            .expect("insert initial row");
    }

    let num_threads: u32 = 4;
    let iterations_per_thread: u32 = 50;
    let success_count = Arc::new(AtomicU32::new(0));
    let error_count = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();

    for thread_id in 0..num_threads {
        let path = db_path.clone();
        let dek = Arc::clone(&dek);
        let successes = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);

        let handle = thread::spawn(move || {
            let conn = open_sqlcipher_db(
                &path,
                dek.as_ref(),
                OpenSqlcipherOptions {
                    create_if_missing: false,
                    load_array_module: false,
                },
            )
            .expect("open encrypted db");
            assert_busy_timeout_is_configured(&conn);

            for i in 0..iterations_per_thread {
                // Mix of reads and writes to create contention.
                if i % 2 == 0 {
                    match conn.execute("UPDATE test_data SET value = value + 1 WHERE id = 1", []) {
                        Ok(_) => {
                            successes.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            if e.to_string().contains("database is locked") {
                                errors.fetch_add(1, Ordering::SeqCst);
                            } else {
                                panic!("thread {thread_id}: unexpected error: {e}");
                            }
                        }
                    }
                } else {
                    match conn.query_row("SELECT value FROM test_data WHERE id = 1", [], |row| {
                        row.get::<_, i64>(0)
                    }) {
                        Ok(_) => {
                            successes.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            if e.to_string().contains("database is locked") {
                                errors.fetch_add(1, Ordering::SeqCst);
                            } else {
                                panic!("thread {thread_id}: unexpected error: {e}");
                            }
                        }
                    }
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread should complete");
    }

    let total_successes = success_count.load(Ordering::SeqCst);
    let total_errors = error_count.load(Ordering::SeqCst);
    let expected_ops: u32 = num_threads * iterations_per_thread;

    assert_eq!(
        total_errors, 0,
        "no SQLITE_BUSY errors should occur with busy_timeout; got {total_errors} errors"
    );
    assert_eq!(
        total_successes, expected_ops,
        "all operations should succeed; got {total_successes}/{expected_ops}"
    );
}

/// Demonstrates that without busy_timeout, concurrent access can fail with SQLITE_BUSY.
///
/// NOTE: This test is informational and non-deterministic. It may not reliably produce
/// SQLITE_BUSY errors on fast machines or under low contention. The test always passes
/// regardless of whether errors occur - it exists to document the behavior difference
/// between connections with and without busy_timeout, not as a regression gate.
#[test]
#[ignore] // Run manually: cargo test --ignored
fn concurrent_access_without_busy_timeout_can_fail() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_concurrency_test");

    // Create the database and table.
    {
        let conn = open_test_db(&db_path, true);
        conn.execute(
            "CREATE TABLE test_data (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)",
            [],
        )
        .expect("create table");
        conn.execute("INSERT INTO test_data (id, value) VALUES (1, 0)", [])
            .expect("insert initial row");
    }

    let num_threads: u32 = 4;
    let iterations_per_thread: u32 = 100;
    let error_count = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();

    for _thread_id in 0..num_threads {
        let path = db_path.clone();
        let errors = Arc::clone(&error_count);

        let handle = thread::spawn(move || {
            // Open without busy_timeout.
            let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                .expect("open db");

            for i in 0..iterations_per_thread {
                let result = if i % 2 == 0 {
                    conn.execute("UPDATE test_data SET value = value + 1 WHERE id = 1", [])
                        .map(|_| ())
                } else {
                    conn.query_row("SELECT value FROM test_data WHERE id = 1", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map(|_| ())
                };

                match result {
                    Err(e) if e.to_string().contains("database is locked") => {
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => {}
                }
                // Ignore other errors for this test.
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread should complete");
    }

    // We expect some SQLITE_BUSY errors without busy_timeout, but this is probabilistic.
    // The test passes regardless - it just documents the behavior difference.
    let total_errors = error_count.load(Ordering::SeqCst);
    println!(
        "Without busy_timeout: {total_errors} SQLITE_BUSY errors out of {} operations",
        num_threads * iterations_per_thread
    );
}
