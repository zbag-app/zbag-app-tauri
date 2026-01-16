use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use zstash_core::domain::{Network, SwapIntent, SwapType};
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

fn spawn_mock_1click_quote_server(
    deadline_iso: String,
    expected_amount: &'static str,
    expected_recipient: &'static str,
    expected_refund_to: &'static str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("server addr");
    let base_url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0u8; 16 * 1024];
        let n = stream.read(&mut buf).expect("read request");
        let req = String::from_utf8_lossy(&buf[..n]);

        let body = req.split("\r\n\r\n").nth(1).unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(body).expect("parse request json");

        assert_eq!(
            json.get("originAsset").and_then(|v| v.as_str()),
            Some("nep141:eth.omft.near")
        );
        assert_eq!(
            json.get("destinationAsset").and_then(|v| v.as_str()),
            Some("nep141:zec.omft.near")
        );
        assert_eq!(
            json.get("amount").and_then(|v| v.as_str()),
            Some(expected_amount)
        );
        assert_eq!(
            json.get("swapType").and_then(|v| v.as_str()),
            Some("EXACT_INPUT")
        );
        assert_eq!(
            json.get("slippageTolerance").and_then(|v| v.as_u64()),
            Some(100)
        );
        assert_eq!(
            json.get("quoteWaitingTimeMs").and_then(|v| v.as_u64()),
            Some(3000)
        );
        assert_eq!(
            json.get("depositType").and_then(|v| v.as_str()),
            Some("ORIGIN_CHAIN")
        );
        assert_eq!(
            json.get("refundTo").and_then(|v| v.as_str()),
            Some(expected_refund_to)
        );
        assert_eq!(
            json.get("refundType").and_then(|v| v.as_str()),
            Some("ORIGIN_CHAIN")
        );
        assert_eq!(
            json.get("recipient").and_then(|v| v.as_str()),
            Some(expected_recipient)
        );
        assert_eq!(
            json.get("recipientType").and_then(|v| v.as_str()),
            Some("DESTINATION_CHAIN")
        );
        assert_eq!(json.get("dry").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            json.get("referral").and_then(|v| v.as_str()),
            Some("zstash")
        );
        assert!(
            json.get("deadline").and_then(|v| v.as_str()).is_some(),
            "deadline must be present"
        );

        // Development mode currently disables app fees.
        assert!(
            json.get("appFees").is_none(),
            "appFees should be omitted while development-mode fees are disabled"
        );

        let deposit_address = "0x0c79D7017D764b3109CEEFF082f3ea6d7b95e8ac";

        let ok_body = format!(
            r#"{{
                "quote": {{
                    "amountIn": "{expected_amount}",
                    "amountInFormatted": "0.001",
                    "amountInUsd": "3.10",
                    "minAmountIn": "{expected_amount}",
                    "amountOut": "672703",
                    "amountOutFormatted": "0.00672703",
                    "amountOutUsd": "2.88",
                    "minAmountOut": "665975",
                    "deadline": "{deadline_iso}",
                    "timeWhenInactive": "{deadline_iso}",
                    "timeEstimate": 160,
                    "depositAddress": "{deposit_address}",
                    "depositMemo": null
                }},
                "quoteRequest": {{}},
                "signature": "mock",
                "timestamp": "{deadline_iso}",
                "correlationId": "test-correlation-id"
            }}"#
        );

        let response = format!(
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            ok_body.len(),
            ok_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    (base_url, handle)
}

#[test]
fn request_swap_quote_to_zec_builds_expected_1click_payload() {
    let root = temp_root("us8_quote_to_zec");
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

    let recipient = "u10spepvgadw6djkhe8fq0mj2lj60dhf2g966c52eqzfh2hpgghlqzrgm246aa252fzx24cw6r88gu6vz99pyzzl4ryphlkteu7u3hq70k";
    let refund_to = "0x3350Fe9Fc38cBa6518471693d748f3f3073C8fdB";

    // Ensure the engine converts 0.001 ETH -> 1_000_000_000_000_000 wei.
    let expected_amount = "1000000000000000";
    let deadline_iso = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let (base_url, server) =
        spawn_mock_1click_quote_server(deadline_iso, expected_amount, recipient, refund_to);
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let tx_service = std::sync::Arc::new(std::sync::Mutex::new(
        zstash_engine::tx_service::TxService::new(zstash_engine::reauth::SystemClock),
    ));
    let swap = SwapService::new_with_near_client(
        app_db_path,
        Arc::clone(&mgr),
        Arc::clone(&tx_service),
        near,
    )
    .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::ToZec,
        swap_mode: Default::default(),
        input_asset: "nep141:eth.omft.near".to_string(),
        input_amount: "0.001".to_string(),
        output_asset: "nep141:zec.omft.near".to_string(),
        output_amount: None,
        destination_address: Some(recipient.to_string()),
        refund_address: Some(refund_to.to_string()),
    };

    let res = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .expect("quote");
    assert_eq!(res.quote_id, "test-correlation-id");
    assert_eq!(res.quote.correlation_id, "test-correlation-id");
    assert_eq!(res.quote.input_amount, expected_amount);
    assert_eq!(res.quote.input_amount_formatted, "0.001");
    assert_eq!(res.quote.output_asset, "nep141:zec.omft.near");
    assert_eq!(res.quote.output_amount, "672703");
    assert!(res.quote.deadline > 0);
    assert!(res.quote.deposit_address.is_some());
    assert_eq!(
        res.quote.app_fee_bps, None,
        "app_fee_bps should be omitted while development-mode fees are disabled"
    );

    server.join().expect("server joined");
}

#[test]
fn start_swap_rejects_expired_quote() {
    let root = temp_root("us8_start_swap_expired_quote");
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

    let recipient = "u10spepvgadw6djkhe8fq0mj2lj60dhf2g966c52eqzfh2hpgghlqzrgm246aa252fzx24cw6r88gu6vz99pyzzl4ryphlkteu7u3hq70k";
    let refund_to = "0x3350Fe9Fc38cBa6518471693d748f3f3073C8fdB";

    let expected_amount = "1000000000000000";
    let deadline_iso = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    let (base_url, server) =
        spawn_mock_1click_quote_server(deadline_iso, expected_amount, recipient, refund_to);
    let near = zstash_network::near_intents::NearIntentsClient::with_base_url(base_url)
        .expect("near client");
    let swap = SwapService::new_with_near_client(app_db_path, Arc::clone(&mgr), near)
        .expect("create swap service");

    let intent = SwapIntent {
        swap_type: SwapType::ToZec,
        swap_mode: Default::default(),
        input_asset: "nep141:eth.omft.near".to_string(),
        input_amount: "0.001".to_string(),
        output_asset: "nep141:zec.omft.near".to_string(),
        output_amount: None,
        destination_address: Some(recipient.to_string()),
        refund_address: Some(refund_to.to_string()),
    };

    let res = swap
        .request_swap_quote(wallet.id, wallet.network, intent)
        .expect("quote");

    let err = swap
        .start_swap(wallet.id, wallet.network, &res.quote_id, false, None, None)
        .unwrap_err();
    let ipc = find_engine_ipc_error(&err).expect("ipc error");
    assert_eq!(ipc.code, errors::QUOTE_EXPIRED);

    server.join().expect("server joined");
}
