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

/// Block on a future, safely handling both inside and outside Tokio runtime contexts.
///
/// This helper consolidates blocking patterns to avoid panics from nested runtime usage:
/// - If called from within a Tokio runtime, uses `block_in_place` to safely block
/// - If called from outside any runtime, uses a shared fallback runtime
///
/// # Panics
///
/// - If called from a `current_thread` runtime (single-threaded), panics with
///   "can call blocking only when running on the multi-threaded runtime".
/// - If the fallback runtime cannot be created (unlikely).
pub(crate) fn block_on<F: std::future::Future>(future: F) -> F::Output {
    match Handle::try_current() {
        Ok(handle) => {
            // Use block_in_place to avoid "cannot start runtime from within runtime" panic.
            // This moves the current task off the runtime thread, allowing block_on to work.
            tokio::task::block_in_place(|| handle.block_on(future))
        }
        Err(_) => {
            // No runtime in scope - use the shared fallback runtime
            fallback_runtime().block_on(future)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_on_outside_runtime() {
        // Calling block_on from outside any runtime should work
        let result = block_on(async { 42 });
        assert_eq!(result, 42);
    }

    #[test]
    fn block_on_inside_runtime() {
        // Calling block_on from inside a runtime should not panic
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // Spawn a blocking task that calls our block_on helper
            let handle = tokio::task::spawn_blocking(|| block_on(async { 123 }));
            let result = handle.await.unwrap();
            assert_eq!(result, 123);
        });
    }

    #[test]
    fn block_on_nested_from_spawn_blocking() {
        // This tests the exact scenario that would panic without block_in_place
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            tokio::task::spawn_blocking(|| {
                // Inside spawn_blocking, we still have a handle but need block_in_place
                block_on(async { "nested" })
            })
            .await
            .unwrap()
        });
        assert_eq!(result, "nested");
    }

    #[test]
    fn spawn_outside_runtime() {
        // spawn should work outside runtime using fallback
        let handle = spawn(async { 99 });
        let result = block_on(async { handle.await.unwrap() });
        assert_eq!(result, 99);
    }

    #[test]
    fn spawn_inside_runtime() {
        // spawn should work inside runtime
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let handle = spawn(async { 77 });
            handle.await.unwrap()
        });
        assert_eq!(result, 77);
    }

    #[test]
    #[should_panic(expected = "can call blocking only when running on the multi-threaded runtime")]
    fn block_on_panics_in_current_thread_runtime() {
        // block_in_place panics when called from a current_thread runtime's single thread
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            // Directly calling block_on from the async context triggers the panic
            // because block_in_place cannot move the task off the only thread
            block_on(async { 1 })
        });
    }
}
