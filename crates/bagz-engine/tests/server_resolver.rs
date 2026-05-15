mod common;

use bagz_core::domain::Network;
use bagz_engine::db::AppDb;
use bagz_engine::server_resolver::resolve_grpc_url;
use bagz_engine::server_resolver::resolve_grpc_url_with_dev_override;

#[cfg(debug_assertions)]
#[test]
fn dev_override_takes_precedence_in_debug_builds() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("bagz_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url =
        resolve_grpc_url_with_dev_override(&db, Network::Testnet, Some("https://example.invalid"))
            .expect("resolve");
    assert_eq!(url, "https://example.invalid");
}

#[cfg(debug_assertions)]
#[test]
fn resolve_grpc_url_reads_env_wrapper_in_debug_builds() {
    const CHILD_MARKER_ENV: &str = "__BAGZ_SERVER_RESOLVER_CHILD__";
    const OVERRIDE_URL: &str = "https://example.invalid";

    if std::env::var_os(CHILD_MARKER_ENV).is_some() {
        let (db_path, _cleanup) =
            common::temp_db_path_with_cleanup("bagz_app_db_server_resolver_child");
        let db = AppDb::open(db_path).expect("db open");

        let url = resolve_grpc_url(&db, Network::Testnet).expect("resolve");
        assert_eq!(url, OVERRIDE_URL);
        return;
    }

    let current_exe = std::env::current_exe().expect("current_exe");
    let status = std::process::Command::new(current_exe)
        .arg("--exact")
        .arg("resolve_grpc_url_reads_env_wrapper_in_debug_builds")
        .arg("--nocapture")
        .env(CHILD_MARKER_ENV, "1")
        .env("BAGZ_GRPC_URL", OVERRIDE_URL)
        .status()
        .expect("spawn child test process");

    assert!(status.success(), "child process failed: {status}");
}

#[test]
fn defaults_to_seeded_default_server_when_no_override() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("bagz_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url = resolve_grpc_url_with_dev_override(&db, Network::Testnet, None).expect("resolve");
    assert_eq!(url, "https://lwd.testnet.zec.pro");
}

#[cfg(not(debug_assertions))]
#[test]
fn override_is_ignored_in_release_builds() {
    let (db_path, _cleanup) = common::temp_db_path_with_cleanup("bagz_app_db_server_resolver");
    let db = AppDb::open(db_path).expect("db open");

    let url =
        resolve_grpc_url_with_dev_override(&db, Network::Testnet, Some("https://example.invalid"))
            .expect("resolve");
    assert_eq!(url, "https://lwd.testnet.zec.pro");
}

#[cfg(not(debug_assertions))]
#[test]
fn resolve_grpc_url_ignores_env_wrapper_in_release_builds() {
    const CHILD_MARKER_ENV: &str = "__BAGZ_SERVER_RESOLVER_CHILD__";
    const OVERRIDE_URL: &str = "https://example.invalid";

    if std::env::var_os(CHILD_MARKER_ENV).is_some() {
        let (db_path, _cleanup) =
            common::temp_db_path_with_cleanup("bagz_app_db_server_resolver_release_child");
        let db = AppDb::open(db_path).expect("db open");

        let url = resolve_grpc_url(&db, Network::Testnet).expect("resolve");
        assert_eq!(url, "https://lwd.testnet.zec.pro");
        return;
    }

    let current_exe = std::env::current_exe().expect("current_exe");
    let status = std::process::Command::new(current_exe)
        .arg("--exact")
        .arg("resolve_grpc_url_ignores_env_wrapper_in_release_builds")
        .arg("--nocapture")
        .env(CHILD_MARKER_ENV, "1")
        .env("BAGZ_GRPC_URL", OVERRIDE_URL)
        .status()
        .expect("spawn child test process");

    assert!(status.success(), "child process failed: {status}");
}
