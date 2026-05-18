use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bagz_core::domain::{TorState, TorStatus};
use bagz_tor::{TorManager, TorManagerConfig};
use tempfile::TempDir;
use tokio::sync::oneshot;

fn new_tor_dir(prefix: &str) -> TempDir {
    // The returned TempDir must outlive any TorManager that uses its path.
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .expect("failed to create temp dir")
}

#[test]
fn initial_on_state_is_normalized_to_off() {
    let tor_dir = new_tor_dir("bagz-tor-test-initial-on");
    let config = TorManagerConfig::new(tor_dir.path().to_path_buf());
    let manager = TorManager::new(
        config,
        TorState {
            enabled: true,
            status: TorStatus::On,
            last_error: None,
        },
    );

    assert_eq!(manager.state().status, TorStatus::Off);
    assert!(manager.state().enabled);

    let rx = manager.subscribe();
    assert_eq!(rx.borrow().state.status, TorStatus::Off);
}

#[test]
fn initial_disabled_state_is_normalized_to_off() {
    let tor_dir = new_tor_dir("bagz-tor-test-initial-disabled");
    let config = TorManagerConfig::new(tor_dir.path().to_path_buf());
    let manager = TorManager::new(
        config,
        TorState {
            enabled: false,
            status: TorStatus::Error,
            last_error: Some("stale error".to_string()),
        },
    );

    assert!(!manager.state().enabled);
    assert_eq!(manager.state().status, TorStatus::Off);
    assert_eq!(manager.state().last_error.as_deref(), None);

    let rx = manager.subscribe();
    assert_eq!(rx.borrow().state.status, TorStatus::Off);
}

#[tokio::test]
async fn state_machine_transitions_to_error_on_bootstrap_failure() {
    let tor_dir = new_tor_dir("bagz-tor-test-bootstrap-failure");
    let mut config = TorManagerConfig::new(tor_dir.path().to_path_buf());
    config.bootstrap_timeout = Duration::from_secs(1);
    config.bootstrap = Arc::new(|_dir| {
        Box::pin(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Err("bootstrap failed".to_string())
        })
    });

    let manager = TorManager::new(
        config,
        TorState {
            enabled: false,
            status: TorStatus::Off,
            last_error: None,
        },
    );

    let mut rx = manager.subscribe();

    manager.set_enabled(true).expect("tokio runtime available");
    assert_eq!(manager.state().status, TorStatus::Connecting);

    // First change: Connecting.
    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);

    // Next change: Error.
    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Error);
    assert!(rx.borrow().state.last_error.is_some());
}

#[tokio::test]
async fn state_machine_transitions_to_error_on_bootstrap_timeout() {
    let tor_dir = new_tor_dir("bagz-tor-test-bootstrap-timeout");
    let mut config = TorManagerConfig::new(tor_dir.path().to_path_buf());
    config.bootstrap_timeout = Duration::from_millis(50);
    config.bootstrap = Arc::new(|_dir| {
        Box::pin(async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Err("bootstrap should have timed out".to_string())
        })
    });

    let manager = TorManager::new(
        config,
        TorState {
            enabled: false,
            status: TorStatus::Off,
            last_error: None,
        },
    );

    let mut rx = manager.subscribe();

    manager.set_enabled(true).expect("tokio runtime available");
    assert_eq!(manager.state().status, TorStatus::Connecting);

    // First change: Connecting.
    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);

    // Next change: Error due to timeout.
    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Error);
    assert_eq!(
        rx.borrow().state.last_error.as_deref(),
        Some("Tor bootstrap timed out")
    );
}

#[tokio::test]
async fn reenable_after_error_restarts_bootstrap_and_clears_last_error() {
    let (ready1_tx, ready1_rx) = oneshot::channel::<()>();
    let (proceed1_tx, proceed1_rx) = oneshot::channel::<()>();
    let (ready2_tx, ready2_rx) = oneshot::channel::<()>();
    let (proceed2_tx, proceed2_rx) = oneshot::channel::<()>();

    let steps = Arc::new(Mutex::new({
        let mut steps = VecDeque::new();
        steps.push_back((1usize, ready1_tx, proceed1_rx));
        steps.push_back((2usize, ready2_tx, proceed2_rx));
        steps
    }));

    let tor_dir = new_tor_dir("bagz-tor-test-reenable-after-error");
    let mut config = TorManagerConfig::new(tor_dir.path().to_path_buf());
    config.bootstrap_timeout = Duration::from_secs(1);
    config.bootstrap = {
        let steps = steps.clone();
        Arc::new(move |_dir| {
            let steps = steps.clone();
            Box::pin(async move {
                let (attempt, ready_tx, proceed_rx) = steps
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("bootstrap step available");
                let _ = ready_tx.send(());
                let _ = proceed_rx.await;
                Err(format!("bootstrap failed (attempt {attempt})"))
            })
        })
    };

    let manager = TorManager::new(
        config,
        TorState {
            enabled: false,
            status: TorStatus::Off,
            last_error: None,
        },
    );

    let mut rx = manager.subscribe();

    // First attempt: Connecting -> Error.
    manager.set_enabled(true).expect("tokio runtime available");
    assert_eq!(manager.state().status, TorStatus::Connecting);

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);

    ready1_rx.await.expect("bootstrap started");
    let _ = proceed1_tx.send(());

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Error);
    assert!(rx.borrow().state.last_error.is_some());

    // Re-enable while already enabled: should restart bootstrap and clear last_error.
    manager.set_enabled(true).expect("tokio runtime available");
    assert_eq!(manager.state().status, TorStatus::Connecting);
    assert_eq!(manager.state().last_error.as_deref(), None);

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);
    assert_eq!(rx.borrow().state.last_error.as_deref(), None);

    ready2_rx.await.expect("bootstrap started (attempt 2)");
    let _ = proceed2_tx.send(());

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Error);
    assert!(
        rx.borrow()
            .state
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("attempt 2"),
        "expected bootstrap to run again after re-enable"
    );
}

#[tokio::test]
async fn state_machine_transitions_to_error_on_tor_dir_create_failure() {
    let tor_dir = new_tor_dir("bagz-tor-test-tor-dir-create-failure");
    let tor_dir_path = tor_dir.path().join("tor-dir-is-a-file");
    std::fs::write(&tor_dir_path, b"not a directory").expect("create tor-dir sentinel file");

    let config = TorManagerConfig::new(tor_dir_path);

    let manager = TorManager::new(
        config,
        TorState {
            enabled: false,
            status: TorStatus::Off,
            last_error: None,
        },
    );

    let mut rx = manager.subscribe();

    manager.set_enabled(true).expect("tokio runtime available");
    assert_eq!(manager.state().status, TorStatus::Connecting);

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Error);
    assert!(
        rx.borrow()
            .state
            .last_error
            .as_deref()
            .unwrap_or_default()
            .starts_with("failed to create tor directory:"),
        "expected tor-dir creation failure error"
    );
}

/// Tests that disabling Tor during bootstrap results in Off state.
///
/// This validates the cancellation check in TorManager::spawn_bootstrap which prevents
/// the race condition where bootstrap completes after disable was called.
#[tokio::test]
async fn disable_during_bootstrap_results_in_off_state() {
    let (ready_tx, ready_rx) = oneshot::channel::<()>();
    let (proceed_tx, proceed_rx) = oneshot::channel::<()>();

    let tor_dir = new_tor_dir("bagz-tor-test-disable-during-bootstrap");
    let mut config = TorManagerConfig::new(tor_dir.path().to_path_buf());
    config.bootstrap_timeout = Duration::from_secs(5);

    let ready_tx = Mutex::new(Some(ready_tx));
    let proceed_rx = Mutex::new(Some(proceed_rx));

    config.bootstrap = Arc::new(move |_dir| {
        let ready_tx = ready_tx.lock().unwrap().take();
        let proceed_rx = proceed_rx.lock().unwrap().take();
        Box::pin(async move {
            // Signal that bootstrap has started
            if let Some(tx) = ready_tx {
                let _ = tx.send(());
            }
            // Wait for test to signal proceed (after calling disable)
            if let Some(rx) = proceed_rx {
                let _ = rx.await;
            }
            Err("cancelled".to_string())
        })
    });

    let manager = TorManager::new(
        config,
        TorState {
            enabled: false,
            status: TorStatus::Off,
            last_error: None,
        },
    );

    let mut rx = manager.subscribe();

    manager.set_enabled(true).expect("tokio runtime available");
    assert_eq!(manager.state().status, TorStatus::Connecting);

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);

    // Wait for bootstrap to start
    ready_rx.await.expect("bootstrap started");

    // Disable Tor while bootstrap is running
    manager.set_enabled(false).expect("tokio runtime available");
    assert_eq!(manager.state().status, TorStatus::Off);

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Off);

    // Let bootstrap proceed (it will return Err but task checks cancellation)
    let _ = proceed_tx.send(());

    // There should be no further transitions after disable. Use a generous
    // timeout to avoid false negatives on slow CI where task scheduling may lag.
    assert!(
        tokio::time::timeout(Duration::from_secs(2), rx.changed())
            .await
            .is_err(),
        "unexpected state change after disabling Tor"
    );

    // Final state should remain Off (not Error from bootstrap, not On)
    assert_eq!(manager.state().status, TorStatus::Off);
    assert!(!manager.state().enabled);
}

#[tokio::test]
async fn rapid_enable_disable_enable_restarts_bootstrap() {
    let (ready1_tx, ready1_rx) = oneshot::channel::<()>();
    let (proceed1_tx, proceed1_rx) = oneshot::channel::<()>();
    let (ready2_tx, ready2_rx) = oneshot::channel::<()>();
    let (proceed2_tx, proceed2_rx) = oneshot::channel::<()>();

    let steps = Arc::new(Mutex::new({
        let mut steps = VecDeque::new();
        steps.push_back((1usize, ready1_tx, proceed1_rx));
        steps.push_back((2usize, ready2_tx, proceed2_rx));
        steps
    }));

    let tor_dir = new_tor_dir("bagz-tor-test-rapid-toggle");
    let mut config = TorManagerConfig::new(tor_dir.path().to_path_buf());
    config.bootstrap_timeout = Duration::from_secs(1);
    config.bootstrap = {
        let steps = steps.clone();
        Arc::new(move |_dir| {
            let steps = steps.clone();
            Box::pin(async move {
                let (attempt, ready_tx, proceed_rx) = steps
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("bootstrap step available");
                let _ = ready_tx.send(());
                let _ = proceed_rx.await;
                Err(format!("bootstrap failed (attempt {attempt})"))
            })
        })
    };

    let manager = TorManager::new(
        config,
        TorState {
            enabled: false,
            status: TorStatus::Off,
            last_error: None,
        },
    );

    let mut rx = manager.subscribe();

    manager.set_enabled(true).expect("tokio runtime available");
    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);

    ready1_rx.await.expect("bootstrap started (attempt 1)");

    manager.set_enabled(false).expect("tokio runtime available");
    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Off);

    manager.set_enabled(true).expect("tokio runtime available");
    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Connecting);

    ready2_rx.await.expect("bootstrap started (attempt 2)");
    let _ = proceed2_tx.send(());

    rx.changed().await.expect("watch change");
    assert_eq!(rx.borrow().state.status, TorStatus::Error);
    assert!(
        rx.borrow()
            .state
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("attempt 2"),
        "expected second enable to restart bootstrap"
    );

    let _ = proceed1_tx.send(());

    assert!(
        tokio::time::timeout(Duration::from_secs(2), rx.changed())
            .await
            .is_err(),
        "unexpected state change after rapid enable/disable/enable sequence"
    );
}

// Note: Full success path testing (Connecting -> On) requires a real
// zcash_client_backend::tor::Client which needs actual Tor bootstrap. The Client
// type has no public constructor, so unit tests cannot return a fake client from
// our `TorManagerConfig.bootstrap` closure; a happy-path test would need an
// integration test that bootstraps Tor for real.
// The cancellation check in TorManager::spawn_bootstrap prevents race conditions
// after bootstrap succeeds. This is validated by code review and the
// disable_during_bootstrap_results_in_off_state test above covers the
// cancellation mechanism during the bootstrap phase.
