//! Tests for ExactOutput (CrossPay) swap mode.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use bagz_core::domain::{Network, SwapIntent, SwapMode, SwapType};
use bagz_core::errors;
use bagz_engine::error::find_engine_ipc_error;
use bagz_engine::key_store::KeyStore;
use bagz_engine::swap_service::SwapService;
use bagz_engine::wallet_manager::WalletManager;

#[derive(Debug, Default, Clone)]
struct TestKeyStore {
    encrypted_mnemonics: Arc<Mutex<Vec<u8>>>,
}

#[derive(Debug, Default, Clone)]
struct CapturedQuoteRequest {
    swap_type: Option<String>,
    amount: Option<String>,
    has_app_fees: Option<bool>,
    quote_requests: usize,
    token_requests: usize,
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
    let root = std::env::temp_dir().join(format!("bagz_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

/// Spawn a mock server that validates ExactOutput requests.
///
/// Returns the captured `swapType` and `amount` fields from the request body.
fn spawn_mock_1click_server_capturing_quote_request(
    expected_requests: usize,
) -> (
    String,
    Arc<Mutex<CapturedQuoteRequest>>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");
    let captured: Arc<Mutex<CapturedQuoteRequest>> =
        Arc::new(Mutex::new(CapturedQuoteRequest::default()));
    let captured_clone = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]);
            let first = req.lines().next().unwrap_or_default();
            let path = first.split_whitespace().nth(1).unwrap_or("/");

            // Extract relevant fields from JSON body
            if path.starts_with("/v0/quote")
                && let Some(body_start) = req.find("\r\n\r\n")
            {
                let body = &req[body_start + 4..];

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
                    let mut captured = captured_clone.lock().expect("mutex poisoned");
                    captured.quote_requests += 1;
                    captured.swap_type = json
                        .get("swapType")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    captured.amount = json
                        .get("amount")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    captured.has_app_fees = Some(
                        json.get("appFees")
                            .and_then(|v| v.as_array())
                            .is_some_and(|fees| !fees.is_empty()),
                    );
                }
            }

            let deadline_iso = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();

            let body = format!(
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
                        "depositAddress": "t1fake",
                        "depositMemo": null
                    }},
                    "quoteRequest": {{}},
                    "signature": "mock",
                    "timestamp": "{deadline_iso}",
                    "correlationId": "test-correlation-id"
                }}"#
            );

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

    (base_url, captured, handle)
}

fn spawn_mock_1click_server_tokens_and_quote(
    tokens_body: String,
    expected_requests: usize,
) -> (
    String,
    Arc<Mutex<CapturedQuoteRequest>>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");
    let captured: Arc<Mutex<CapturedQuoteRequest>> =
        Arc::new(Mutex::new(CapturedQuoteRequest::default()));
    let captured_clone = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]);
            let first = req.lines().next().unwrap_or_default();
            let path = first.split_whitespace().nth(1).unwrap_or("/");

            let deadline_iso = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();

            let response_body = if path.starts_with("/v0/tokens") {
                let mut captured = captured_clone.lock().expect("mutex poisoned");
                captured.token_requests += 1;
                tokens_body.clone()
            } else if path.starts_with("/v0/quote") {
                if let Some(body_start) = req.find("\r\n\r\n") {
                    let body = &req[body_start + 4..];
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
                        let mut captured = captured_clone.lock().expect("mutex poisoned");
                        captured.quote_requests += 1;
                        captured.swap_type = json
                            .get("swapType")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        captured.amount = json
                            .get("amount")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        captured.has_app_fees = Some(
                            json.get("appFees")
                                .and_then(|v| v.as_array())
                                .is_some_and(|fees| !fees.is_empty()),
                        );
                    }
                }

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
                            "depositAddress": "t1fake",
                            "depositMemo": null
                        }},
                        "quoteRequest": {{}},
                        "signature": "mock",
                        "timestamp": "{deadline_iso}",
                        "correlationId": "test-correlation-id"
                    }}"#
                )
            } else {
                r#"{"status":"FAILED","message":"unexpected path"}"#.to_string()
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    (base_url, captured, handle)
}

#[test]
fn request_swap_quote_exact_output_sends_correct_swap_type() {
    let root = temp_root("us10_exact_output_swap_type");
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

    let (base_url, captured_request, server) = spawn_mock_1click_server_capturing_quote_request(1);
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    // Create ExactOutput intent - specifying output_amount, not input_amount
    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: String::new(), // Not used for ExactOutput
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: Some("1".to_string()), // Required for ExactOutput
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let _res = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .expect("quote should succeed");

    server.join().expect("server joined");

    // Verify the server received EXACT_OUTPUT swap type and the amount was converted using the
    // output asset's decimals (NEAR has 24 decimals).
    let captured = captured_request.lock().expect("mutex poisoned").clone();
    assert_eq!(
        captured.swap_type,
        Some("EXACT_OUTPUT".to_string()),
        "ExactOutput mode should send swapType: EXACT_OUTPUT"
    );
    assert_eq!(
        captured.amount,
        Some("1000000000000000000000000".to_string()),
        "ExactOutput should convert 1 NEAR (24 decimals) to smallest units"
    );
}

#[test]
fn request_swap_quote_exact_output_requires_output_amount() {
    let root = temp_root("us10_exact_output_missing_amount");
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

    // No server needed - request should fail validation before any network call
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(
        "http://127.0.0.1:1".to_string(),
    )
    .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    // Create ExactOutput intent WITHOUT output_amount - should fail
    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: "1".to_string(), // This is ignored for ExactOutput
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: None, // Missing! Should cause an error
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let err = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::INVALID_REQUEST);
    assert!(
        ipc.message.contains("output_amount"),
        "Error message should mention output_amount: {}",
        ipc.message
    );
}

#[test]
fn request_swap_quote_exact_output_rejects_empty_output_amount() {
    let root = temp_root("us10_exact_output_empty_amount");
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

    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(
        "http://127.0.0.1:1".to_string(),
    )
    .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    // Create ExactOutput intent with empty output_amount - should fail
    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: "1".to_string(),
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: Some("".to_string()), // Empty string should also fail
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let err = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::INVALID_REQUEST);
}

#[test]
fn request_swap_quote_exact_output_rejects_whitespace_output_amount() {
    let root = temp_root("us10_exact_output_whitespace_amount");
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
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(
        "http://127.0.0.1:1".to_string(),
    )
    .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: String::new(),
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: Some("   \n\t  ".to_string()),
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let err = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::INVALID_REQUEST);
    assert!(
        ipc.message.contains("output_amount"),
        "Error message should mention output_amount: {}",
        ipc.message
    );
}

#[test]
fn request_swap_quote_exact_output_truncates_excess_decimals() {
    let root = temp_root("us10_exact_output_truncates_excess_decimals");
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

    let (base_url, captured_request, server) = spawn_mock_1click_server_capturing_quote_request(1);
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    // NEAR uses 24 decimals. Provide 25 fractional digits; the last digit should be truncated.
    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: String::new(),
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: Some("1.1234567890123456789012345".to_string()),
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let _res = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .expect("quote should succeed");

    server.join().expect("server joined");

    let captured = captured_request.lock().expect("mutex poisoned").clone();
    assert_eq!(captured.swap_type, Some("EXACT_OUTPUT".to_string()));
    assert_eq!(
        captured.amount,
        Some("1123456789012345678901234".to_string()),
        "should truncate to 24 decimals before converting to smallest units"
    );
}

#[test]
fn request_swap_quote_exact_output_rejects_zero_output_amount() {
    let root = temp_root("us10_exact_output_zero_amount");
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
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(
        "http://127.0.0.1:1".to_string(),
    )
    .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: String::new(),
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: Some("0".to_string()),
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
fn request_swap_quote_exact_output_resolves_decimals_from_tokens_for_unknown_asset() {
    let root = temp_root("us10_exact_output_decimals_from_tokens");
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

    let output_asset = "nep245:v2_1.omni.hot.tg:137_mockasset";
    let tokens_body = format!(
        r#"
        [
          {{
            "assetId": "{output_asset}",
            "symbol": "MOCK",
            "blockchain": "pol",
            "decimals": 6,
            "price": 1.0
          }}
        ]
        "#
    );

    let (base_url, captured_request, server) =
        spawn_mock_1click_server_tokens_and_quote(tokens_body, 2);
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: String::new(),
        output_asset: output_asset.to_string(),
        output_amount: Some("1.234567".to_string()),
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let _res = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .expect("quote should succeed");

    server.join().expect("server joined");

    let captured = captured_request.lock().expect("mutex poisoned").clone();
    assert_eq!(
        captured.token_requests, 1,
        "expected one /v0/tokens request"
    );
    assert_eq!(captured.quote_requests, 1, "expected one /v0/quote request");
    assert_eq!(captured.swap_type, Some("EXACT_OUTPUT".to_string()));
    assert_eq!(
        captured.amount,
        Some("1234567".to_string()),
        "1.234567 with 6 decimals should convert to smallest units correctly"
    );
}

#[test]
fn request_swap_quote_exact_output_rejects_unknown_asset_missing_from_tokens() {
    let root = temp_root("us10_exact_output_unknown_asset_rejected");
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

    let tokens_body = r#"
    [
      {
        "assetId": "nep141:wrap.near",
        "symbol": "NEAR",
        "blockchain": "near",
        "decimals": 24,
        "price": 5.0
      }
    ]
    "#
    .to_string();

    let (base_url, captured_request, server) =
        spawn_mock_1click_server_tokens_and_quote(tokens_body, 1);
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: String::new(),
        output_asset: "nep245:v2_1.omni.hot.tg:137_unknown_asset".to_string(),
        output_amount: Some("1".to_string()),
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let err = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::INVALID_ASSET);
    assert!(ipc.message.contains("unsupported asset"));

    server.join().expect("server joined");

    let captured = captured_request.lock().expect("mutex poisoned").clone();
    assert_eq!(
        captured.token_requests, 1,
        "expected one /v0/tokens request"
    );
    assert_eq!(
        captured.quote_requests, 0,
        "quote request should not be made when asset decimals cannot be resolved"
    );
}

#[test]
fn request_swap_quote_exact_output_omits_app_fees_in_development_mode() {
    let root = temp_root("us10_exact_output_app_fees_disabled");
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

    let (base_url, captured, server) = spawn_mock_1click_server_capturing_quote_request(1);
    let near = bagz_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::FromZec,
        swap_mode: SwapMode::ExactOutput,
        input_asset: "nep141:zec.omft.near".to_string(),
        input_amount: String::new(),
        output_asset: "nep141:wrap.near".to_string(),
        output_amount: Some("1".to_string()),
        destination_address: Some("near_destination.near".to_string()),
        refund_address: Some("u1refundaddress".to_string()),
    };

    let res = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .expect("quote should succeed");

    server.join().expect("server joined");

    let captured = captured.lock().expect("mutex poisoned").clone();
    assert_eq!(captured.quote_requests, 1);
    assert_eq!(
        captured.has_app_fees,
        Some(false),
        "appFees should be omitted while development-mode fees are disabled"
    );
    assert_eq!(
        res.quote.app_fee_bps, None,
        "quote should not advertise app fee when appFees are disabled"
    );
}
