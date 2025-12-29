use std::sync::OnceLock;

use tokio::runtime::{Handle, Runtime};

fn fallback_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("create tokio runtime"))
}

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
