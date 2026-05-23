use std::collections::HashMap;
use std::time::Duration;

use bagz_xtask::cmd::cef_smoketest::lsof::{FakeLsof, LsofOutput};
use bagz_xtask::cmd::cef_smoketest::parser::fixture_stream;
use bagz_xtask::cmd::cef_smoketest::process::FakeProcessEnumerator;
use bagz_xtask::cmd::cef_smoketest::sampler::{SampleOutcome, sample_once};

#[test]
fn evidence_with_nonzero_status_is_policy_failure() {
    let outcome = run_sample(FakeLsof::new(vec![LsofOutput::failure(
        1,
        fixture_stream("external-connected").expect("fixture"),
        b"lsof: synthetic stderr about a dead PID".to_vec(),
    )]));

    assert!(matches!(outcome, SampleOutcome::Policy { .. }));
}

#[test]
fn clean_with_nonzero_status_and_no_race_is_instrumentation_failure() {
    let outcome = run_sample(FakeLsof::new(vec![LsofOutput::failure(
        1,
        fixture_stream("loopback-listener").expect("fixture"),
        b"lsof: synthetic generic error".to_vec(),
    )]));

    assert!(matches!(outcome, SampleOutcome::Instrument { .. }));
}

#[test]
fn timeout_without_evidence_is_instrumentation_failure() {
    let outcome = run_sample(FakeLsof::new(vec![LsofOutput::timeout(
        Vec::new(),
        Vec::new(),
    )]));

    assert!(matches!(outcome, SampleOutcome::Instrument { .. }));
}

#[test]
fn timeout_with_evidence_is_policy_failure() {
    let outcome = run_sample(FakeLsof::new(vec![LsofOutput::timeout(
        fixture_stream("external-connected").expect("fixture"),
        Vec::new(),
    )]));

    assert!(matches!(outcome, SampleOutcome::Policy { .. }));
}

#[test]
fn retry_benign_no_matching_files_passes() {
    let live_pid = std::process::id();
    let dead_pid = spawn_dead_pid();
    let lsof = FakeLsof::new(vec![
        LsofOutput::failure(
            1,
            fixture_stream("loopback-listener").expect("fixture"),
            b"lsof: synthetic stderr forcing race retry".to_vec(),
        ),
        LsofOutput::failure(1, Vec::new(), Vec::new()),
    ]);
    let enumerator = FakeProcessEnumerator::new(HashMap::from([(live_pid, vec![dead_pid])]));

    let outcome = sample_once(
        "fixture-retry-benign-no-matching-files",
        live_pid,
        &enumerator,
        &lsof,
        Duration::from_secs(1),
        &|_| {},
    );

    assert!(matches!(outcome, SampleOutcome::Ok));
    assert_eq!(lsof.calls().len(), 2);
}

fn run_sample(lsof: FakeLsof) -> SampleOutcome {
    let enumerator = FakeProcessEnumerator::default();
    sample_once(
        "fixture",
        std::process::id(),
        &enumerator,
        &lsof,
        Duration::from_secs(1),
        &|_| {},
    )
}

fn spawn_dead_pid() -> u32 {
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(":")
        .spawn()
        .expect("spawn short-lived process");
    let pid = child.id();
    let _ = child.wait();
    pid
}
