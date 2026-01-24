//! Helper functions for the test bridge.

use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use tracing::error;

use zstash_core::domain::Network;
use zstash_core::errors;
use zstash_core::ipc::v1::common::IpcResult;

/// Timeout for server probe to avoid UI blocking when offline.
pub const SERVER_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Helper to map anyhow errors to IpcResult
pub fn map_anyhow<T, F>(f: F) -> IpcResult<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    match f() {
        Ok(v) => IpcResult::Ok { ok: v },
        Err(err) => {
            error!(error = ?err, "Command failed");
            IpcResult::Err {
                err: to_ipc_error(err),
            }
        }
    }
}

pub fn to_ipc_error(err: anyhow::Error) -> zstash_core::ipc::v1::common::IpcError {
    if let Some(engine) = zstash_engine::error::find_engine_ipc_error(&err) {
        return zstash_core::ipc::v1::common::IpcError {
            code: engine.code.to_string(),
            message: engine.message.clone(),
            details: engine.details.clone(),
        };
    }

    zstash_core::ipc::v1::common::IpcError {
        code: errors::INTERNAL_ERROR.to_string(),
        message: format!("{:#}", err),
        details: None,
    }
}

pub fn system_time_to_unix_ms(time: std::time::SystemTime) -> anyhow::Result<i64> {
    let duration = time.duration_since(std::time::UNIX_EPOCH)?;
    Ok(i64::try_from(duration.as_millis())?)
}

pub fn fallback_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("create tokio runtime"))
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => fallback_runtime().block_on(future),
    }
}

pub fn probe_chain_name_with_timeout(
    client: &zstash_network::grpc_client::GrpcClient,
) -> anyhow::Result<String> {
    let info = block_on(async {
        match tokio::time::timeout(SERVER_PROBE_TIMEOUT, client.probe_server()).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("connection timed out")),
        }
    })?;

    Ok(info.chain_name)
}

pub fn parse_network(chain_name: &str) -> anyhow::Result<Network> {
    let name = chain_name.trim().to_lowercase();
    match name.as_str() {
        "main" | "mainnet" => Ok(Network::Mainnet),
        "test" | "testnet" => Ok(Network::Testnet),
        other => Err(zstash_engine::error::ipc_err(
            errors::INVALID_REQUEST,
            format!("unsupported chain_name: {other}"),
        )),
    }
}

/// Load accounts for a wallet (helper extracted from wallet.rs)
pub fn load_accounts_for_wallet(
    mgr: &mut zstash_engine::wallet_manager::WalletManager,
    wallet_id: uuid::Uuid,
) -> anyhow::Result<Vec<zstash_core::domain::AccountInfo>> {
    use std::collections::HashMap;
    use tracing::warn;
    use zstash_core::domain::{AccountInfo, AccountType};

    let wallet_db_accounts = mgr.list_wallet_db_account_ids(wallet_id)?;
    let meta_accounts =
        zstash_engine::db::account_meta::list_accounts(mgr.app_db().conn(), wallet_id)
            .map_err(|e| anyhow::anyhow!(e))?;

    let meta_by_id: HashMap<u32, AccountInfo> =
        meta_accounts.into_iter().map(|a| (a.id, a)).collect();

    let mut out = Vec::with_capacity(wallet_db_accounts.len());
    for account_id in wallet_db_accounts {
        if let Some(meta) = meta_by_id.get(&account_id) {
            out.push(meta.clone());
            continue;
        }

        warn!(account_id, "Account metadata missing; applying defaults");
        out.push(AccountInfo {
            id: account_id,
            name: format!("Account {}", account_id + 1),
            account_type: if account_id == 0 {
                AccountType::Software
            } else {
                AccountType::HardwareSigner
            },
        });
    }

    Ok(out)
}
