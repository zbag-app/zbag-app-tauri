//! Tests for ExactOutput (CrossPay) swap mode.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use zstash_core::domain::{Network, SwapIntent, SwapMode, SwapType};
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

/// Spawn a mock server that validates ExactOutput requests.
///
/// Returns the captured swap_type from the request body.
fn spawn_mock_1click_server_capturing_swap_type(
    expected_requests: usize,
) -> (String, Arc<Mutex<Option<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");
    let captured_swap_type: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let captured_clone = Arc::clone(&captured_swap_type);

    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]);

            // Extract swap_type from JSON body
            if let Some(body_start) = req.find("\r\n\r\n") {
                let body = &req[body_start + 4..];
                // Simple extraction: look for "swapType":"EXACT_OUTPUT" or "swapType":"EXACT_INPUT"
                if body.contains(r#""swapType":"EXACT_OUTPUT""#) {
                    *captured_clone.lock().expect("mutex poisoned") =
                        Some("EXACT_OUTPUT".to_string());
                } else if body.contains(r#""swapType":"EXACT_INPUT""#) {
                    *captured_clone.lock().expect("mutex poisoned") =
                        Some("EXACT_INPUT".to_string());
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

    (base_url, captured_swap_type, handle)
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
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    let (base_url, captured_swap_type, server) = spawn_mock_1click_server_capturing_swap_type(1);
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(base_url)
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

    // Verify the server received EXACT_OUTPUT swap type
    let captured = captured_swap_type.lock().expect("mutex poisoned").clone();
    assert_eq!(
        captured,
        Some("EXACT_OUTPUT".to_string()),
        "ExactOutput mode should send swapType: EXACT_OUTPUT"
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
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    // No server needed - request should fail validation before any network call
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(
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
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false, None)
        .expect("create wallet")
        .wallet;

    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(
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
