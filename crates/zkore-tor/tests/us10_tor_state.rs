use std::sync::Arc;
use std::time::Duration;

use zkore_core::domain::{TorState, TorStatus};
use zkore_tor::{TorManager, TorManagerConfig};

#[tokio::test]
async fn state_machine_transitions_to_error_on_bootstrap_failure() {
    let mut config = TorManagerConfig::new(std::env::temp_dir().join("zkore-tor-test"));
    config.bootstrap_timeout = Duration::from_secs(1);
    config.bootstrap = Arc::new(|_dir| {
        Box::pin(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Err("bootstrap failed".to_string())
        })
    });
    config.health_check = Arc::new(|_client| Box::pin(async { Ok(()) }));

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
