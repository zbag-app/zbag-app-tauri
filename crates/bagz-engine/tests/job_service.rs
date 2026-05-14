//! Unit tests for the background job service.
//!
//! These tests cover the public API of JobService:
//! - Job lookup (get_progress, list_jobs)
//! - Cancellation behavior (cancel_job)
//! - Job cleanup (clear_finished_jobs)
//!
//! Note: Full integration tests for start_send_job and start_shield_job
//! require wallet setup and are covered in us2/us3 tests.

use uuid::Uuid;

use zstash_core::domain::{JobProgress, JobState, JobType};
use zstash_engine::job_service::JobService;

#[test]
fn job_service_new_creates_empty_state() {
    let service = JobService::new();
    let wallet_id = Uuid::new_v4();

    // No jobs should exist for any wallet
    let jobs = service.list_jobs(wallet_id);
    assert!(jobs.is_empty(), "new job service should have no jobs");
}

#[test]
fn get_progress_returns_none_for_unknown_job() {
    let service = JobService::new();

    let progress = service.get_progress("unknown-job-id");
    assert!(
        progress.is_none(),
        "unknown job should return None for progress"
    );
}

#[test]
fn cancel_job_returns_false_for_unknown_job() {
    let service = JobService::new();

    let result = service.cancel_job("unknown-job-id");
    assert!(!result, "cancelling unknown job should return false");
}

#[test]
fn list_jobs_returns_empty_for_unknown_wallet() {
    let service = JobService::new();
    let unknown_wallet = Uuid::new_v4();

    let jobs = service.list_jobs(unknown_wallet);
    assert!(jobs.is_empty(), "unknown wallet should have no jobs");
}

#[test]
fn job_progress_queued_state_is_cancellable() {
    let progress = JobProgress::queued("job1".to_string(), JobType::Send);
    assert!(matches!(progress.state, JobState::Queued));
    assert!(progress.can_cancel, "queued jobs should be cancellable");
    assert_eq!(progress.job_type, JobType::Send);
    assert_eq!(progress.job_id, "job1");
}

#[test]
fn job_progress_proving_state_is_cancellable() {
    let progress = JobProgress::proving("job2".to_string(), JobType::Shield, Some(50));
    assert!(matches!(progress.state, JobState::Running));
    assert!(progress.can_cancel, "proving phase should be cancellable");
    assert_eq!(progress.progress_percent, Some(50));
    assert_eq!(progress.job_type, JobType::Shield);
}

#[test]
fn job_progress_broadcasting_state_is_not_cancellable() {
    let progress =
        JobProgress::broadcasting("job3".to_string(), JobType::Send, "txid123".to_string());
    assert!(matches!(progress.state, JobState::Running));
    assert!(
        !progress.can_cancel,
        "broadcasting phase should not be cancellable"
    );
    assert_eq!(progress.txid, Some("txid123".to_string()));
}

#[test]
fn job_progress_completed_state_is_terminal() {
    let progress =
        JobProgress::completed("job4".to_string(), JobType::Shield, "txid456".to_string());
    assert!(matches!(progress.state, JobState::Completed));
    assert!(!progress.can_cancel, "completed jobs cannot be cancelled");
    assert_eq!(progress.txid, Some("txid456".to_string()));
    assert!(progress.error.is_none());
}

#[test]
fn job_progress_failed_state_contains_error() {
    let progress = JobProgress::failed(
        "job5".to_string(),
        JobType::Send,
        "network error".to_string(),
        Some("partial-txid".to_string()),
    );
    assert!(matches!(progress.state, JobState::Failed));
    assert!(!progress.can_cancel);
    assert_eq!(progress.error, Some("network error".to_string()));
    assert_eq!(progress.txid, Some("partial-txid".to_string()));
}

#[test]
fn job_progress_cancelled_state_is_terminal() {
    let progress = JobProgress::cancelled("job6".to_string(), JobType::Shield);
    assert!(matches!(progress.state, JobState::Cancelled));
    assert!(
        !progress.can_cancel,
        "cancelled jobs cannot be re-cancelled"
    );
}

#[test]
fn job_service_default_is_same_as_new() {
    let service1 = JobService::new();
    let service2 = JobService::default();

    // Both should start with empty state
    let wallet_id = Uuid::new_v4();
    assert!(service1.list_jobs(wallet_id).is_empty());
    assert!(service2.list_jobs(wallet_id).is_empty());
}

#[test]
fn clear_finished_jobs_on_empty_service_does_not_panic() {
    let service = JobService::new();
    let wallet_id = Uuid::new_v4();

    // Should not panic on empty service
    service.clear_finished_jobs(wallet_id);

    // Should still be empty
    assert!(service.list_jobs(wallet_id).is_empty());
}
