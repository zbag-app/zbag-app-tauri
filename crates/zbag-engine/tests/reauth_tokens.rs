use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use zbag_core::ipc::v1::commands::wallet::ReauthPurpose;
use zbag_engine::reauth::{Clock, ReauthError, ReauthManager};

#[derive(Debug, Clone)]
struct TestClock(Arc<Mutex<SystemTime>>);

impl Clock for TestClock {
    fn now(&self) -> SystemTime {
        *self.0.lock().expect("mutex poisoned")
    }
}

impl TestClock {
    fn new(now: SystemTime) -> Self {
        Self(Arc::new(Mutex::new(now)))
    }

    fn advance(&self, delta: Duration) {
        let mut now = self.0.lock().expect("mutex poisoned");
        *now += delta;
    }
}

#[test]
fn reauth_token_is_single_use() {
    let wallet_id = Uuid::new_v4();
    let clock = TestClock::new(SystemTime::UNIX_EPOCH);
    let mut mgr = ReauthManager::new(clock);
    let (token, _) = mgr.issue(wallet_id, ReauthPurpose::Spend);

    mgr.validate_and_consume(&token, wallet_id, ReauthPurpose::Spend)
        .expect("first use should succeed");

    let err = mgr
        .validate_and_consume(&token, wallet_id, ReauthPurpose::Spend)
        .expect_err("replay must fail");
    assert!(matches!(err, ReauthError::Invalid));
}

#[test]
fn reauth_token_expires_after_two_minutes() {
    let wallet_id = Uuid::new_v4();
    let clock = TestClock::new(SystemTime::UNIX_EPOCH);
    let mut mgr = ReauthManager::new(clock.clone());
    let (token, expires_at) = mgr.issue(wallet_id, ReauthPurpose::Spend);

    assert!(expires_at > SystemTime::UNIX_EPOCH);
    clock.advance(Duration::from_secs(121));

    let err = mgr
        .validate_and_consume(&token, wallet_id, ReauthPurpose::Spend)
        .expect_err("expired token must fail");
    assert!(matches!(err, ReauthError::Expired));
}
