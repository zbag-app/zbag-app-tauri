use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const SEND_TRANSACTION_TIMEOUT: Duration = Duration::from_secs(45);
pub const FAILOVER_TRANSPORT_FAILURE_THRESHOLD: u32 = 2;

const RETRY_SCHEDULE_SECS: [u64; 7] = [5, 15, 45, 120, 300, 900, 1_800];
const RETRY_JITTER_RATIO: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastErrorClass {
    TransientTransport,
    TransientServer,
    Terminal,
    Unknown,
}

pub fn classify_broadcast_error_message(message: &str) -> BroadcastErrorClass {
    let lower = message.to_lowercase();

    if lower.contains("broadcast rejected")
        || lower.contains("mempool")
        || lower.contains("non-mandatory-script-verify-flag")
        || lower.contains("bad-txns")
        || lower.contains("already spent")
        || lower.contains("txn-mempool-conflict")
    {
        return BroadcastErrorClass::Terminal;
    }

    if lower.contains("failed to connect")
        || lower.contains("connection")
        || lower.contains("transport")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("deadline exceeded")
        || lower.contains("unavailable")
        || lower.contains("broken pipe")
        || lower.contains("reset by peer")
        || lower.contains("refused")
        || lower.contains("dns")
        || lower.contains("h2")
    {
        return BroadcastErrorClass::TransientTransport;
    }

    if lower.contains("resource exhausted")
        || lower.contains("temporarily")
        || lower.contains("try again")
        || lower.contains("sendtransaction rpc failed")
        || lower.contains("rpc failed")
        || lower.contains("internal")
    {
        return BroadcastErrorClass::TransientServer;
    }

    BroadcastErrorClass::Unknown
}

pub fn is_retryable_broadcast_error_class(class: BroadcastErrorClass) -> bool {
    matches!(
        class,
        BroadcastErrorClass::TransientTransport
            | BroadcastErrorClass::TransientServer
            | BroadcastErrorClass::Unknown
    )
}

pub fn retry_backoff_base(attempt_count: u32) -> Duration {
    let idx = usize::try_from(attempt_count).unwrap_or(usize::MAX);
    let secs = RETRY_SCHEDULE_SECS
        .get(idx)
        .copied()
        .unwrap_or(*RETRY_SCHEDULE_SECS.last().expect("non-empty schedule"));
    Duration::from_secs(secs)
}

pub fn retry_backoff_with_jitter(attempt_count: u32, rng: &mut impl rand::Rng) -> Duration {
    let base = retry_backoff_base(attempt_count);
    let base_ms = base.as_millis() as f64;
    let jitter_ms = base_ms * RETRY_JITTER_RATIO;
    let offset_ms: f64 = rng.gen_range(-jitter_ms..=jitter_ms);
    let jittered_ms = (base_ms + offset_ms).max(1.0).round() as u64;
    Duration::from_millis(jittered_ms)
}

pub fn should_trigger_failover(transport_failure_streak: u32) -> bool {
    transport_failure_streak >= FAILOVER_TRANSPORT_FAILURE_THRESHOLD
}

pub async fn send_with_timeout<F>(send_future: F) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    send_with_timeout_for(SEND_TRANSACTION_TIMEOUT, send_future).await
}

pub async fn send_with_timeout_for<F>(timeout: Duration, send_future: F) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    match tokio::time::timeout(timeout, send_future).await {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!(
            "send transaction timed out after {}s",
            timeout.as_secs()
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use rand::SeedableRng as _;

    use super::*;

    #[test]
    fn retry_backoff_schedule_progresses_as_expected() {
        let expected = [5, 15, 45, 120, 300, 900, 1_800, 1_800, 1_800];
        let actual: Vec<u64> = (0..9)
            .map(|attempt| retry_backoff_base(attempt).as_secs())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn retry_backoff_jitter_stays_within_bounds() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let base = retry_backoff_base(3); // 120s
        let base_ms = base.as_millis() as i128;
        let max_delta = (base_ms as f64 * RETRY_JITTER_RATIO) as i128;

        for _ in 0..100 {
            let jittered = retry_backoff_with_jitter(3, &mut rng);
            let jittered_ms = jittered.as_millis() as i128;
            let delta = (jittered_ms - base_ms).abs();
            assert!(delta <= max_delta, "delta={delta} max_delta={max_delta}");
        }
    }

    #[test]
    fn classifies_timeout_as_transient_transport() {
        let class = classify_broadcast_error_message("send transaction timed out after 45s");
        assert_eq!(class, BroadcastErrorClass::TransientTransport);
        assert!(is_retryable_broadcast_error_class(class));
    }

    #[test]
    fn classifies_mempool_rejection_as_terminal() {
        let class = classify_broadcast_error_message(
            "broadcast rejected (code 16): txn-mempool-conflict: already spent",
        );
        assert_eq!(class, BroadcastErrorClass::Terminal);
        assert!(!is_retryable_broadcast_error_class(class));
    }

    #[test]
    fn unknown_errors_default_to_retryable() {
        let class = classify_broadcast_error_message("some completely new upstream failure");
        assert_eq!(class, BroadcastErrorClass::Unknown);
        assert!(is_retryable_broadcast_error_class(class));
    }

    #[test]
    fn failover_triggers_after_repeated_transport_failures() {
        assert!(!should_trigger_failover(1));
        assert!(should_trigger_failover(2));
    }

    #[tokio::test]
    async fn timeout_wrapper_returns_structured_timeout_error() {
        let err = send_with_timeout_for(Duration::from_millis(5), pending::<anyhow::Result<()>>())
            .await
            .expect_err("pending future should timeout");

        let message = err.to_string();
        assert!(message.contains("timed out"));
        assert_eq!(
            classify_broadcast_error_message(&message),
            BroadcastErrorClass::TransientTransport
        );
    }
}
