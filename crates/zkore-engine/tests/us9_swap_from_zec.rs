use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use zkore_core::domain::{Network, SwapIntent, SwapType};
use zkore_core::errors;
use zkore_core::ipc::v1::commands::wallet::ReauthPurpose;
use zkore_engine::error::find_engine_ipc_error;
use zkore_engine::key_store::KeyStore;
use zkore_engine::swap_service::SwapService;
use zkore_engine::wallet_manager::WalletManager;

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
    let root = std::env::temp_dir().join(format!("zkore_{prefix}_{}", Uuid::new_v4()));
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

            let deadline_ms = chrono::Utc::now().timestamp_millis() + 60_000;

            let body = if path.starts_with("/v0/quote") {
                format!(
                    r#"{{"quote_id":"q1","output_amount":"1","fee_amount":"0","fee_asset":"","deadline":{deadline_ms},"rate":"1"}}"#
                )
            } else if path.starts_with("/v0/deposit/submit") {
                format!(
                    r#"{{"remote_id":"r1","deposit_address":"{deposit_address}","deposit_memo":null,"deadline":{deadline_ms}}}"#
                )
            } else {
                r#"{"status":"FAILED","message":"unexpected path"}"#.to_string()
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).expect("write response");
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
        input_asset: "zcash:mainnet:native".to_string(),
        input_amount: "1000".to_string(),
        output_asset: "near:mainnet:native".to_string(),
        destination_address: Some("near_destination".to_string()),
        refund_address: None,
    };

    let res = swap.request_swap_quote(wallet_id, network, intent)?;
    Ok(res.quote_id)
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
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false)
        .expect("create wallet")
        .wallet;

    let (base_url, server) = spawn_mock_1click_server("t1fake", 1);
    let near = zkore_network::near_intents::NearIntentsClient::with_base_url(base_url)
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
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false)
        .expect("create wallet")
        .wallet;

    let (base_url, server) = spawn_mock_1click_server("t1fake", 1);
    let near = zkore_network::near_intents::NearIntentsClient::with_base_url(base_url)
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
        .create_wallet("Test Wallet", Network::Mainnet, "pw", false)
        .expect("create wallet")
        .wallet;

    let (reauth_token, _expires_at) = mgr
        .lock()
        .expect("mutex poisoned")
        .reauth_wallet(wallet.id, "pw", ReauthPurpose::Spend)
        .expect("reauth");

    let (base_url, server) = spawn_mock_1click_server("t1fake", 2);
    let near = zkore_network::near_intents::NearIntentsClient::with_base_url(base_url)
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
