use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use zbag_core::domain::{Network, SwapIntent, SwapType};
use zbag_core::errors;
use zbag_core::ipc::v1::commands::wallet::ReauthPurpose;
use zbag_engine::db::backup_meta;
use zbag_engine::error::find_engine_ipc_error;
use zbag_engine::key_store::KeyStore;
use zbag_engine::swap_service::SwapService;
use zbag_engine::wallet_manager::WalletManager;

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
    let root = std::env::temp_dir().join(format!("zbag_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn spawn_mock_1click_server(
    deposit_address: &'static str,
    expected_requests: usize,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]);
            let first = req.lines().next().unwrap_or_default();
            let path = first.split_whitespace().nth(1).unwrap_or("/");

            let deadline_iso = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();

            // New API returns nested quote response
            let body = if path.starts_with("/v0/quote") {
                format!(
                    r#"{{
                        "quote": {{
                            "amountIn": "100000000",
                            "amountInFormatted": "1",
                            "amountInUsd": "50.00",
                            "minAmountIn": "100000000",
                            "amountOut": "1000000000000000000000000",
                            "amountOutFormatted": "1",
                            "amountOutUsd": "50.00",
                            "minAmountOut": "990000000000000000000000",
                            "deadline": "{deadline_iso}",
                            "timeWhenInactive": "{deadline_iso}",
                            "timeEstimate": 120,
                            "depositAddress": "{deposit_address}",
                            "depositMemo": null
                        }},
                        "quoteRequest": {{}},
                        "signature": "mock",
                        "timestamp": "{deadline_iso}",
                        "correlationId": "test-correlation-id"
                    }}"#
                )
            } else if path.starts_with("/v0/deposit/submit") {
                // New API just acknowledges the deposit
                r#"{"success": true}"#.to_string()
            } else {
                r#"{"status":"FAILED","message":"unexpected path"}"#.to_string()
            };

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

    (base_url, handle)
}

fn spawn_mock_1click_server_decimal_amount(
    deposit_address: &'static str,
    expected_requests: usize,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]);
            let first = req.lines().next().unwrap_or_default();
            let path = first.split_whitespace().nth(1).unwrap_or("/");

            let deadline_iso = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();

            let body = if path.starts_with("/v0/quote") {
                // Ensure amountInFormatted includes a decimal point. Wallet send APIs expect zatoshis
                // (integer smallest-units), so using the formatted value would cause "invalid amount".
                format!(
                    r#"{{
                        "quote": {{
                            "amountIn": "10000000",
                            "amountInFormatted": "0.1",
                            "amountInUsd": "25.00",
                            "minAmountIn": "10000000",
                            "amountOut": "1000000",
                            "amountOutFormatted": "1",
                            "amountOutUsd": "25.00",
                            "minAmountOut": "990000",
                            "deadline": "{deadline_iso}",
                            "timeWhenInactive": "{deadline_iso}",
                            "timeEstimate": 120,
                            "depositAddress": "{deposit_address}",
                            "depositMemo": null
                        }},
                        "quoteRequest": {{}},
                        "signature": "mock",
                        "timestamp": "{deadline_iso}",
                        "correlationId": "test-correlation-id"
                    }}"#
                )
            } else if path.starts_with("/v0/deposit/submit") {
                r#"{"success": true}"#.to_string()
            } else {
                r#"{"status":"FAILED","message":"unexpected path"}"#.to_string()
            };

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

    (base_url, handle)
}

fn setup_from_zec_quote(
    swap: &SwapService,
    wallet_id: Uuid,
    network: Network,
) -> anyhow::Result<String> {
    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: Default::default(),
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: "1".to_string(),
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: None,
        destination_address: Some("near_destination.near".to_string()),
        // Refund address is required by the new API
        refund_address: Some("u1refundaddress".to_string()),
    };

    let res = swap.request_swap_quote(wallet_id, network, intent)?;
    Ok(res.quote_id)
}

#[test]
fn request_swap_quote_exact_input_rejects_zero_input_amount() {
    let root = temp_root("us9_zero_input_amount");
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
        .create_wallet_for_test("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // No server needed - request should fail validation before any network call.
    let near = zbag_network::near_intents::NearIntentsClient::with_base_url(
        "http://127.0.0.1:1".to_string(),
    )
    .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: Default::default(),
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: "0".to_string(),
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: None,
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let err = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::INVALID_REQUEST);
    assert!(
        ipc.message.contains("greater than zero"),
        "Error message should indicate amount must be > 0: {}",
        ipc.message
    );
}

#[test]
fn start_swap_from_zec_requires_privacy_ack() {
    let root = temp_root("us9_privacy_ack");
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
        .create_wallet_for_test("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Only 1 request: quote (dry=false returns deposit address)
    let (base_url, server) = spawn_mock_1click_server("t1fake", 1);
    let near = zbag_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let quote_id = setup_from_zec_quote(&swap, wallet.id, wallet.network).expect("quote");

    let err = swap
        .start_swap(wallet.id, wallet.network, &quote_id, false, None, None)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::PRIVACY_ACK_REQUIRED);

    server.join().expect("server joined");
}

#[test]
fn start_swap_from_zec_requires_reauth_token_when_acknowledged() {
    let root = temp_root("us9_reauth_required");
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
        .create_wallet_for_test("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // Only 1 request: quote (dry=false returns deposit address)
    let (base_url, server) = spawn_mock_1click_server("t1fake", 1);
    let near = zbag_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let quote_id = setup_from_zec_quote(&swap, wallet.id, wallet.network).expect("quote");

    let err = swap
        .start_swap(wallet.id, wallet.network, &quote_id, true, None, None)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::INVALID_REQUEST);

    server.join().expect("server joined");
}

#[test]
fn start_swap_from_zec_is_blocked_until_backup_complete() {
    let root = temp_root("us9_backup_gate");
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
        .create_wallet_for_test("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    let (reauth_token, _expires_at) = mgr
        .lock()
        .expect("mutex poisoned")
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth");

    // Only 1 request: quote (dry=false returns deposit address)
    // start_swap should fail with BACKUP_REQUIRED before making any more API calls
    let (base_url, server) = spawn_mock_1click_server("t1fake", 1);
    let near = zbag_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let quote_id = setup_from_zec_quote(&swap, wallet.id, wallet.network).expect("quote");

    let err = swap
        .start_swap(
            wallet.id,
            wallet.network,
            &quote_id,
            true,
            Some(&reauth_token),
            None,
        )
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::BACKUP_REQUIRED);

    server.join().expect("server joined");
}

#[test]
fn start_swap_from_zec_uses_zatoshis_amount_for_wallet_send() {
    let root = temp_root("us9_from_zec_zatoshis_send");
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
        .create_wallet_for_test("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    {
        let mgr = mgr.lock().expect("mutex poisoned");
        backup_meta::set_backup_required(mgr.app_db().conn(), wallet.id, false)
            .expect("disable backup gate");
    }

    let (reauth_token, _expires_at) = mgr
        .lock()
        .expect("mutex poisoned")
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth");

    // Use a quote response with amountInFormatted = "0.1" to ensure we don't pass formatted
    // amounts into send APIs that require zatoshis (integer).
    let (base_url, server) = spawn_mock_1click_server_decimal_amount("t1fake", 1);
    let near = zbag_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: Default::default(),
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: "0.1".to_string(),
        output_asset: "nep141:base.omft.near".to_string(),
        output_amount: None,
        destination_address: Some("0x3350Fe9Fc38cBa6518471693d748f3f3073C8fdB".to_string()),
        refund_address: Some("t1ZMK188cmsdQxYPQi7Y917332HwvsKCdjM".to_string()),
    };

    let quote_id = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .expect("quote")
        .quote_id;

    let err = swap
        .start_swap(
            wallet.id,
            wallet.network,
            &quote_id,
            true,
            Some(&reauth_token),
            None,
        )
        .expect_err("start swap should fail with invalid recipient (t1fake)");
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::INVALID_RECIPIENT);

    server.join().expect("server joined");
}
