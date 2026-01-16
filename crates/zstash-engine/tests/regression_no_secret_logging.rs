use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use zstash_core::domain::{Network, SwapIntent, SwapType};
use zstash_core::ipc::v1::commands::wallet::ReauthPurpose;
use zstash_core::sensitive::SensitiveString;
use zstash_engine::db::backup_meta;
use zstash_engine::key_store::KeyStore;
use zstash_engine::swap_service::SwapService;
use zstash_engine::wallet_manager::WalletManager;

type StoreKey = (Uuid, u8);
type Store = HashMap<StoreKey, Vec<u8>>;
type SharedStore = Arc<Mutex<Store>>;

#[derive(Clone, Default)]
struct BufMakeWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufMakeWriter {
    type Writer = BufWriterGuard;

    fn make_writer(&'a self) -> Self::Writer {
        BufWriterGuard {
            buf: Arc::clone(&self.buf),
        }
    }
}

struct BufWriterGuard {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl io::Write for BufWriterGuard {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buf
            .lock()
            .expect("mutex poisoned")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
struct TestKeyStore {
    encrypted_mnemonics: SharedStore,
}

impl KeyStore for TestKeyStore {
    fn store_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
        encrypted_mnemonic: &[u8],
    ) -> anyhow::Result<()> {
        self.encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .insert(
                (wallet_id, network_key(network)),
                encrypted_mnemonic.to_vec(),
            );
        Ok(())
    }

    fn load_encrypted_mnemonic(
        &self,
        wallet_id: Uuid,
        network: Network,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self
            .encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .get(&(wallet_id, network_key(network)))
            .cloned())
    }

    fn delete_encrypted_mnemonic(&self, wallet_id: Uuid, network: Network) -> anyhow::Result<()> {
        self.encrypted_mnemonics
            .lock()
            .expect("mutex poisoned")
            .remove(&(wallet_id, network_key(network)));
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

fn network_key(network: Network) -> u8 {
    match network {
        Network::Mainnet => 0,
        Network::Testnet => 1,
    }
}

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("zstash_{prefix}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn regression_no_secret_logging() {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let writer = BufMakeWriter {
        buf: Arc::clone(&buf),
    };
    let reauth_token_seen = Arc::new(Mutex::new(String::new()));

    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let password = "pw-SECRET-12345";
        let memo = "memo-SECRET-12345";
        let full_address = "u1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq";
        let signed_payload = "signed-payload-SECRET-12345";
        let restore_phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

        tracing::info!(memo = %zstash_engine::logging::redact_memo(memo));
        tracing::info!(address = %zstash_engine::logging::redact_address(full_address));

        // Create wallet (exercise mnemonic/password handling).
        let root = temp_root("no_secret_logging_create");
        let app_db_path = root.join("app.db");
        let wallets_root = root.join("wallets");
        let key_store = TestKeyStore::default();
        let wallet_manager = Arc::new(Mutex::new(
            WalletManager::new_with_wallets_root(
                app_db_path.clone(),
                wallets_root,
                Box::new(key_store),
            )
            .expect("create wallet manager"),
        ));

        let wallet_id = {
            let mut mgr = wallet_manager.lock().expect("mutex poisoned");
            mgr.create_wallet_for_test("Test Wallet", Network::Testnet, password, false, None)
                .expect("create wallet")
                .wallet
                .id
        };

        // Allow spend-related flows to proceed past the backup gate.
        {
            let mgr = wallet_manager.lock().expect("mutex poisoned");
            let now_ms = chrono::Utc::now().timestamp_millis();
            backup_meta::mark_backup_complete(mgr.app_db().conn(), wallet_id, now_ms, "test")
                .expect("mark backup complete");
        }

        // Try several sensitive flows; all should be safe to log (even if they error).
        let reauth_token = {
            let mut mgr = wallet_manager.lock().expect("mutex poisoned");
            let (token, _expires) = mgr
                .reauth_wallet(wallet_id, password, ReauthPurpose::Spend)
                .expect("issue reauth token");
            token
        };
        *reauth_token_seen.lock().expect("mutex poisoned") = reauth_token.clone();

        tracing::info!(reauth_token = %zstash_engine::logging::Redacted(&reauth_token));

        {
            let mut mgr = wallet_manager.lock().expect("mutex poisoned");
            let _ = mgr.prepare_send_for_test(0, "invalid-recipient", "1", Some(memo), false);
        }

        {
            let mut mgr = wallet_manager.lock().expect("mutex poisoned");
            let _ = mgr.shield_funds(0, true, &reauth_token, None);
        }

        {
            let mut mgr = wallet_manager.lock().expect("mutex poisoned");
            let _ = mgr.finalize_signing_for_test(
                "test-signing-request-id",
                signed_payload,
                &reauth_token,
                None,
            );
        }

        // Swap-from / quote paths should fail closed on Testnet without network calls.
        let tx_service = std::sync::Arc::new(std::sync::Mutex::new(
            zstash_engine::tx_service::TxService::new(zstash_engine::reauth::SystemClock),
        ));
        let swap_service = SwapService::new(
            app_db_path,
            Arc::clone(&wallet_manager),
            Arc::clone(&tx_service),
        )
        .expect("swap service");
        let _ = swap_service.request_swap_quote(
            wallet_id,
            Network::Testnet,
            SwapIntent {
                swap_type: SwapType::FromZec,
                swap_mode: Default::default(),
                input_asset: "ZEC".to_string(),
                input_amount: "1".to_string(),
                output_asset: "USDC".to_string(),
                output_amount: None,
                destination_address: Some(full_address.to_string()),
                refund_address: None,
            },
        );

        // Restore wallet (exercise seed phrase parsing + zeroization).
        let restore_root = temp_root("no_secret_logging_restore");
        let restore_app_db_path = restore_root.join("app.db");
        let restore_wallets_root = restore_root.join("wallets");
        let restore_key_store = TestKeyStore::default();
        let mut restore_mgr = WalletManager::new_with_wallets_root(
            restore_app_db_path,
            restore_wallets_root,
            Box::new(restore_key_store),
        )
        .expect("create restore wallet manager");
        let _ = restore_mgr
            .restore_wallet_for_test(
                "Restored wallet",
                Network::Testnet,
                password,
                false,
                SensitiveString::from(restore_phrase),
                None,
            )
            .expect("restore wallet should succeed");
    });

    let logs = String::from_utf8_lossy(&buf.lock().expect("mutex poisoned")).to_string();
    let reauth_token = reauth_token_seen.lock().expect("mutex poisoned").clone();

    // Secrets must never appear verbatim in logs.
    for secret in [
        "pw-SECRET-12345",
        "memo-SECRET-12345",
        "signed-payload-SECRET-12345",
        "u1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
    ] {
        assert!(
            !logs.contains(secret),
            "logs must not contain secret value: {secret}"
        );
    }
    assert!(
        reauth_token.is_empty() || !logs.contains(&reauth_token),
        "logs must not contain raw reauth token"
    );

    assert!(
        logs.contains("[REDACTED MEMO"),
        "expected redacted memo marker"
    );
}
