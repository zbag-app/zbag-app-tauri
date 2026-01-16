use std::sync::OnceLock;

use tokio::runtime::{Handle, Runtime};

fn fallback_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("create tokio runtime"))
}

/// Spawn a future on the Tokio runtime, safely handling both inside and outside runtime contexts.
///
/// - If called from within a Tokio runtime, spawns on the current runtime
/// - If called from outside any runtime, uses a shared fallback runtime
pub(crate) fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    match Handle::try_current() {
        Ok(handle) => handle.spawn(future),
        Err(_) => fallback_runtime().spawn(future),
    }
}

/// Spawn a blocking task on the Tokio blocking thread pool.
///
/// Use this for CPU-intensive or blocking I/O operations (filesystem, SQLite)
/// that would otherwise starve the async runtime.
pub(crate) fn spawn_blocking<F, R>(f: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    match Handle::try_current() {
        Ok(handle) => handle.spawn_blocking(f),
        Err(_) => fallback_runtime().spawn_blocking(f),
    }
}
