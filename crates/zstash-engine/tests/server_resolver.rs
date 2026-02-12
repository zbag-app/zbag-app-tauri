mod common;

use zstash_core::domain::Network;
use zstash_engine::db::AppDb;
use zstash_engine::server_resolver::{resolve_grpc_url, resolve_grpc_url_with_dev_override};

#[test]
fn dev_override_takes_precedence_in_debug_builds() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url =
        resolve_grpc_url_with_dev_override(&db, Network::Testnet, Some("https://example.invalid"))
            .expect("resolve");
    assert_eq!(url, "https://example.invalid");
}

#[test]
fn defaults_to_seeded_default_server_when_no_override() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url = resolve_grpc_url(&db, Network::Testnet).expect("resolve");
    assert_eq!(url, "https://lwd.testnet.zec.pro");
}

#[cfg(not(debug_assertions))]
#[test]
fn override_is_ignored_in_release_builds() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("zstash_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url =
        resolve_grpc_url_with_dev_override(&db, Network::Testnet, Some("https://example.invalid"))
            .expect("resolve");
    assert_ne!(url, "https://example.invalid");
}
