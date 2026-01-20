mod common;

use zstash_core::domain::Network;
use zstash_engine::db::AppDb;
use zstash_engine::server_resolver::resolve_grpc_url;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn dev_override_takes_precedence_in_debug_builds() {
    let _guard = ENV_LOCK.lock().expect("mutex poisoned");
    let prev = std::env::var("ZSTASH_GRPC_URL").ok();
    unsafe {
        std::env::set_var("ZSTASH_GRPC_URL", "https://example.invalid");
    }
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url = resolve_grpc_url(&db, Network::Testnet).expect("resolve");
    assert_eq!(url, "https://example.invalid");

    match prev {
        Some(value) => unsafe { std::env::set_var("ZSTASH_GRPC_URL", value) },
        None => unsafe { std::env::remove_var("ZSTASH_GRPC_URL") },
    };
}

#[test]
fn defaults_to_seeded_default_server_when_no_override() {
    let _guard = ENV_LOCK.lock().expect("mutex poisoned");
    let prev = std::env::var("ZSTASH_GRPC_URL").ok();
    unsafe {
        std::env::remove_var("ZSTASH_GRPC_URL");
    }
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url = resolve_grpc_url(&db, Network::Testnet).expect("resolve");
    assert_eq!(url, "https://lwd.testnet.zec.pro");

    match prev {
        Some(value) => unsafe { std::env::set_var("ZSTASH_GRPC_URL", value) },
        None => unsafe { std::env::remove_var("ZSTASH_GRPC_URL") },
    };
}

#[cfg(not(debug_assertions))]
#[test]
fn override_is_ignored_in_release_builds() {
    unsafe {
        std::env::set_var("ZSTASH_GRPC_URL", "https://example.invalid");
    }
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url = resolve_grpc_url(&db, Network::Testnet).expect("resolve");
    assert_ne!(url, "https://example.invalid");

    unsafe {
        std::env::remove_var("ZSTASH_GRPC_URL");
    }
}
