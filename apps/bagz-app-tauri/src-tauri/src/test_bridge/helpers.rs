//! Helper functions for the test bridge.

use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use tracing::error;

use bagz_core::errors;
use bagz_core::ipc::v1::common::IpcResult;

/// Default timeout for server probe to avoid UI blocking when offline.
const DEFAULT_SERVER_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

pub fn server_probe_timeout() -> Duration {
    let raw = match std::env::var("BAGZ_TEST_BRIDGE_PROBE_TIMEOUT_MS") {
        Ok(value) => value,
        Err(_) => return DEFAULT_SERVER_PROBE_TIMEOUT,
    };

    match raw.trim().parse::<u64>() {
        Ok(ms) if ms > 0 => Duration::from_millis(ms),
        _ => {
            error!(
                value = raw.as_str(),
                "invalid BAGZ_TEST_BRIDGE_PROBE_TIMEOUT_MS; using default"
            );
            DEFAULT_SERVER_PROBE_TIMEOUT
        }
    }
}

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

pub fn to_ipc_error(err: anyhow::Error) -> bagz_core::ipc::v1::common::IpcError {
    if let Some(engine) = bagz_engine::error::find_engine_ipc_error(&err) {
        return bagz_core::ipc::v1::common::IpcError {
            code: engine.code.to_string(),
            message: engine.message.clone(),
            details: engine.details.clone(),
        };
    }

    bagz_core::ipc::v1::common::IpcError {
        code: errors::INTERNAL_ERROR.to_string(),
        message: format!("{:#}", err),
        details: None,
    }
}

/// Fallback runtime for synchronous callers outside a Tokio context (e.g. tests).
pub fn fallback_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("create tokio runtime"))
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    // In the normal test-bridge server, we're already on Tokio. The fallback
    // runtime is mainly for unit tests or utilities that call this helper
    // outside an async runtime. Avoid block_in_place on current-thread runtimes.
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread {
                return fallback_runtime().block_on(future);
            }
            tokio::task::block_in_place(|| handle.block_on(future))
        }
        Err(_) => fallback_runtime().block_on(future),
    }
}
