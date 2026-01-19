//! Tests for swap status refresh and resume functionality (US16).
//!
//! These tests cover the `refresh_swap_status` and `resume_pending_swaps` methods
//! which were added to support polling recovery after app restart.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use zstash_core::domain::{Network, SwapInfo, SwapState, SwapType};
use zstash_core::errors;
use zstash_engine::error::find_engine_ipc_error;
use zstash_engine::key_store::KeyStore;
use zstash_engine::swap_service::SwapService;
use zstash_engine::wallet_manager::WalletManager;

#[derive(Debug, Default, Clone)]
struct TestKeyStore {
    encrypted_mnemonics: Arc<Mutex<Vec<u8>>>,
}

impl KeyStore for TestKeyStore {
    fn store_encrypted_mnemonic(
        &self,
        _wallet_id: Uuid,
        _network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()> {
        *self.encrypted_mnemonics.lock().expect("mutex poisoned") = encrypted_mnemonic.to_vec();
        Ok(())
    }

    fn load_encrypted_mnemonic(
        &self,
        _wallet_id: Uuid,
        _network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let bytes = self
            .encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .clone();
        if bytes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(bytes))
        }
    }

    fn delete_encrypted_mnemonic(&self, _wallet_id: Uuid, _network: Network) -> anyhow::Result<()> {
        self.encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .clear();
        Ok(())
    }

    fn store_keychain_unlock_material(
        &self,
        _wallet_id: Uuid,
        _network: Network,
        _unlock_material: &[u8],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn load_keychain_unlock_material(
        &self,
        _wallet_id: Uuid,
        _network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn delete_keychain_unlock_material(
        &self,
        _wallet_id: Uuid,
        _network: Network,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("zstash_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

/// Spawn a mock server that responds to status requests with a given status string.
fn spawn_mock_status_server(
    status: &'static str,
    expected_requests: usize,
) -> (String, thread::JoinHandle<()>, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");
    let request_count = Arc::new(AtomicUsize::new(0));
    let request_count_clone = Arc::clone(&request_count);

    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            request_count_clone.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 16 * 1024];
            let _ = stream.read(&mut buf).expect("read request");

            let body = format!(r#"{{"status":"{status}"}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    (base_url, handle, request_count)
}

/// Spawn a mock server that returns an error response.
fn spawn_mock_error_server(expected_requests: usize) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let _ = stream.read(&mut buf).expect("read request");

            let body = r#"{"error":"Internal Server Error"}"#;
            let response = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    (base_url, handle)
}

fn create_test_swap(state: SwapState, has_deposit_address: bool) -> SwapInfo {
    let now_ms = chrono::Utc::now().timestamp_millis();
    SwapInfo {
        id: Uuid::new_v4(),
        remote_id: Some("test-correlation-id".to_string()),
        swap_type: SwapType::ToZec,
        input_asset: "nep141:eth.omft.near".to_string(),
        input_amount: "0.001".to_string(),
        output_asset: "nep141:zec.omft.near".to_string(),
        output_amount: Some("0.01".to_string()),
        deposit_address: if has_deposit_address {
            Some("0x1234567890abcdef".to_string())
        } else {
            None
        },
        deposit_memo: None,
        destination_address: Some("u1destination".to_string()),
        refund_address: Some("0xrefund".to_string()),
        state,
        deadline: Some(now_ms + 3600 * 1000),
        last_error: None,
        created_at: now_ms,
        updated_at: now_ms,
    }
}

fn open_app_db(path: &std::path::Path) -> anyhow::Result<rusqlite::Connection> {
    use rusqlite::OpenFlags;
    let conn = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}

fn insert_swap_directly(
    conn: &rusqlite::Connection,
    wallet_id: Uuid,
    swap: &SwapInfo,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO swaps (
            id, remote_id, wallet_id, swap_type, input_asset, input_amount, output_asset, output_amount,
            deposit_address, deposit_memo, destination_address, refund_address,
            state, deadline, last_error, created_at, updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17
        )",
        rusqlite::params![
            swap.id.to_string(),
            swap.remote_id,
            wallet_id.to_string(),
            format!("{:?}", swap.swap_type),
            swap.input_asset,
            swap.input_amount,
            swap.output_asset,
            swap.output_amount,
            swap.deposit_address,
            swap.deposit_memo,
            swap.destination_address,
            swap.refund_address,
            format!("{:?}", swap.state),
            swap.deadline,
            swap.last_error,
            swap.created_at,
            swap.updated_at,
        ],
    )?;
    Ok(())
}

#[test]
fn refresh_swap_status_updates_state_from_remote() {
    let root = temp_root("us16_refresh_updates_state");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mgr = WalletManager::new_with_wallets_root(
        app_db_path.clone(),
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mgr = Arc::new(Mutex::new(mgr));

    let wallet = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Insert a swap in AwaitingDeposit state
    let swap = create_test_swap(SwapState::AwaitingDeposit, true);
    let conn = open_app_db(&app_db_path).expect("open app db");
    insert_swap_directly(&conn, wallet.id, &swap).expect("insert swap");
    drop(conn);

    // Mock server returns SUCCESS status (maps to Confirming, then Completed if tx confirmed)
    // Since we have no confirmed tx, it will stay at Confirming
    let (base_url, server, _) = spawn_mock_status_server("SUCCESS", 1);
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap_service = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let res = swap_service
        .refresh_swap_status(wallet.id, swap.id, None)
        .expect("refresh swap status");

    // SUCCESS maps to Confirming (since we have no confirmed tx in the test)
    assert_eq!(res.swap.state, SwapState::Confirming);
    assert_ne!(res.swap.updated_at, swap.updated_at);

    server.join().expect("server joined");
}

#[test]
fn refresh_swap_status_is_noop_for_terminal_states() {
    let root = temp_root("us16_refresh_noop_terminal");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mgr = WalletManager::new_with_wallets_root(
        app_db_path.clone(),
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mgr = Arc::new(Mutex::new(mgr));

    let wallet = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Insert a swap in Completed state (terminal)
    let swap = create_test_swap(SwapState::Completed, true);
    let original_updated_at = swap.updated_at;
    let conn = open_app_db(&app_db_path).expect("open app db");
    insert_swap_directly(&conn, wallet.id, &swap).expect("insert swap");
    drop(conn);

    // Mock server should NOT receive any requests for terminal state
    let (base_url, server, request_count) = spawn_mock_status_server("SUCCESS", 0);
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap_service = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let res = swap_service
        .refresh_swap_status(wallet.id, swap.id, None)
        .expect("refresh swap status");

    // State should remain Completed
    assert_eq!(res.swap.state, SwapState::Completed);
    // updated_at should not change since no API call was made
    assert_eq!(res.swap.updated_at, original_updated_at);
    // Verify no API calls were made
    assert_eq!(request_count.load(Ordering::SeqCst), 0);

    server.join().expect("server joined");
}

#[test]
fn refresh_swap_status_is_noop_without_deposit_address() {
    let root = temp_root("us16_refresh_noop_no_deposit_addr");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mgr = WalletManager::new_with_wallets_root(
        app_db_path.clone(),
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mgr = Arc::new(Mutex::new(mgr));

    let wallet = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Insert a non-terminal swap without deposit_address
    let swap = create_test_swap(SwapState::AwaitingDeposit, false);
    let original_updated_at = swap.updated_at;
    let conn = open_app_db(&app_db_path).expect("open app db");
    insert_swap_directly(&conn, wallet.id, &swap).expect("insert swap");
    drop(conn);

    // Mock server should NOT receive any requests if deposit_address is missing
    let (base_url, server, request_count) = spawn_mock_status_server("SUCCESS", 0);
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap_service = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let res = swap_service
        .refresh_swap_status(wallet.id, swap.id, None)
        .expect("refresh swap status");

    assert_eq!(res.swap.state, SwapState::AwaitingDeposit);
    assert_eq!(res.swap.updated_at, original_updated_at);
    assert_eq!(request_count.load(Ordering::SeqCst), 0);
    server.join().expect("server joined");
}

#[test]
fn refresh_swap_status_stores_error_on_api_failure() {
    let root = temp_root("us16_refresh_stores_error");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mgr = WalletManager::new_with_wallets_root(
        app_db_path.clone(),
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mgr = Arc::new(Mutex::new(mgr));

    let wallet = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Insert a swap in AwaitingDeposit state
    let swap = create_test_swap(SwapState::AwaitingDeposit, true);
    let conn = open_app_db(&app_db_path).expect("open app db");
    insert_swap_directly(&conn, wallet.id, &swap).expect("insert swap");
    drop(conn);

    // Mock server returns error
    let (base_url, server) = spawn_mock_error_server(1);
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap_service = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    // refresh_swap_status should still return Ok (best-effort behavior)
    let res = swap_service
        .refresh_swap_status(wallet.id, swap.id, None)
        .expect("refresh swap status");

    // State should remain unchanged
    assert_eq!(res.swap.state, SwapState::AwaitingDeposit);
    // last_error should be populated
    assert!(res.swap.last_error.is_some());
    assert!(res.swap.last_error.as_ref().unwrap().contains("500"));

    server.join().expect("server joined");
}

#[test]
fn refresh_swap_status_rejects_wrong_wallet() {
    let root = temp_root("us16_refresh_wrong_wallet");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mgr = WalletManager::new_with_wallets_root(
        app_db_path.clone(),
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mgr = Arc::new(Mutex::new(mgr));

    // Create wallet A
    let wallet_a = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Wallet A", Network::Mainnet, "pw", false, None)
        .expect("create wallet A")
        .wallet;

    // Create wallet B
    let wallet_b = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Wallet B", Network::Mainnet, "pw", false, None)
        .expect("create wallet B")
        .wallet;

    // Insert a swap owned by wallet A
    let swap = create_test_swap(SwapState::AwaitingDeposit, true);
    let conn = open_app_db(&app_db_path).expect("open app db");
    insert_swap_directly(&conn, wallet_a.id, &swap).expect("insert swap");
    drop(conn);

    // No server needed since the request should fail before API call
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url("http://localhost:1")
        .expect("near client");
    let swap_service = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    // Try to refresh with wallet B's ID (should fail)
    let err = swap_service
        .refresh_swap_status(wallet_b.id, swap.id, None)
        .unwrap_err();

    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::SWAP_FAILED);
    assert!(ipc.message.contains("swap not found"));
}

#[test]
fn resume_pending_swaps_resumes_non_terminal_only() {
    let root = temp_root("us16_resume_non_terminal");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mgr = WalletManager::new_with_wallets_root(
        app_db_path.clone(),
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mgr = Arc::new(Mutex::new(mgr));

    let wallet = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Insert swaps with various states
    let swap_completed = create_test_swap(SwapState::Completed, true);
    let swap_awaiting = create_test_swap(SwapState::AwaitingDeposit, true);
    let swap_pending = create_test_swap(SwapState::Pending, true);

    let conn = open_app_db(&app_db_path).expect("open app db");
    insert_swap_directly(&conn, wallet.id, &swap_completed).expect("insert completed swap");
    insert_swap_directly(&conn, wallet.id, &swap_awaiting).expect("insert awaiting swap");
    insert_swap_directly(&conn, wallet.id, &swap_pending).expect("insert pending swap");
    drop(conn);

    // No server needed for resume_pending_swaps (it just starts polling tasks)
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url("http://localhost:1")
        .expect("near client");
    let swap_service = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let res = swap_service
        .resume_pending_swaps(wallet.id, None)
        .expect("resume pending swaps");

    // Only non-terminal swaps should be resumed (AwaitingDeposit and Pending)
    assert_eq!(res.resumed_count, 2);
}

#[test]
fn resume_pending_swaps_skips_swaps_without_deposit_address() {
    let root = temp_root("us16_resume_no_deposit_addr");
    let app_db_path = root.join("app.db");
    let wallets_root = root.join("wallets");

    let mgr = WalletManager::new_with_wallets_root(
        app_db_path.clone(),
        wallets_root,
        Box::new(TestKeyStore::default()),
    )
    .expect("create wallet manager");
    let mgr = Arc::new(Mutex::new(mgr));

    let wallet = mgr
        .lock()
        .expect("mutex poisoned")
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Insert a Draft swap without deposit_address
    let swap_draft = create_test_swap(SwapState::Draft, false);

    let conn = open_app_db(&app_db_path).expect("open app db");
    insert_swap_directly(&conn, wallet.id, &swap_draft).expect("insert draft swap");
    drop(conn);

    // No server needed
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url("http://localhost:1")
        .expect("near client");
    let swap_service = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let res = swap_service
        .resume_pending_swaps(wallet.id, None)
        .expect("resume pending swaps");

    // Swaps without a deposit_address cannot be polled and should not be counted.
    assert_eq!(res.resumed_count, 0);
}
