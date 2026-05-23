use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use super::exit::ExitCode;
use super::log::LogArtifact;
use super::lsof::{FakeLsof, LsofOutput};
use super::parser::{classify_lsof_fields, fixture_stream};
use super::process::{FakeProcessEnumerator, filter_live_pids};
use super::sampler::{SampleOutcome, sample_once};

pub fn run(log: &Arc<LogArtifact>) -> ExitCode {
    log.write("running CEF network smoke parser self-test");

    let mut ok = true;
    ok &= parser_fixture(log, "loopback-listener", true);
    ok &= parser_fixture(log, "wildcard-listener", false);
    ok &= parser_fixture(log, "zero-listener", false);
    ok &= parser_fixture(log, "external-connected", false);
    ok &= parser_fixture(log, "loopback-connected", true);

    let dead_pid = spawn_dead_pid();
    let current_pid = std::process::id();
    ok &= live_pid_fixture(log, "all-live", &[current_pid], &[current_pid]);
    ok &= live_pid_fixture(log, "mixed", &[current_pid, dead_pid], &[current_pid]);
    ok &= live_pid_fixture(log, "all-dead", &[dead_pid], &[]);

    ok &= sample_socket_fixture(
        log,
        "evidence-with-nonzero-status",
        1,
        fake_evidence_failure(),
    );
    ok &= sample_socket_fixture(
        log,
        "clean-with-nonzero-status-no-race",
        2,
        FakeLsof::new(vec![LsofOutput::failure(
            1,
            fixture_stream("loopback-listener").expect("fixture"),
            b"lsof: synthetic generic error".to_vec(),
        )]),
    );
    ok &= sample_socket_fixture(
        log,
        "timeout-no-evidence",
        2,
        FakeLsof::new(vec![LsofOutput::timeout(Vec::new(), Vec::new())]),
    );
    ok &= sample_socket_fixture(
        log,
        "timeout-with-evidence",
        1,
        FakeLsof::new(vec![LsofOutput::timeout(
            fixture_stream("external-connected").expect("fixture"),
            Vec::new(),
        )]),
    );
    ok &= retry_benign_no_matching_files_fixture(log, current_pid, dead_pid);

    if ok {
        log.write("PASS: CEF network smoke parser self-test");
        ExitCode::Pass
    } else {
        ExitCode::Instrument
    }
}

fn parser_fixture(log: &LogArtifact, name: &str, should_pass: bool) -> bool {
    let stream = fixture_stream(name).expect("fixture");
    let violations = classify_lsof_fields(&format!("selftest:{name}"), &stream);
    let passed = violations.is_empty() == should_pass;

    if passed {
        log.write(&format!("PASS: parser fixture {name}"));
    } else {
        log.write(&format!(
            "FAIL: parser fixture {name} expected {} got {} violation(s)",
            if should_pass { "pass" } else { "fail" },
            violations.len()
        ));
    }

    passed
}

fn live_pid_fixture(log: &LogArtifact, label: &str, input: &[u32], expected: &[u32]) -> bool {
    let got = filter_live_pids(input);
    let passed = got == expected;

    if passed {
        log.write(&format!("PASS: filter_live_pids fixture {label}"));
    } else {
        log.write(&format!(
            "FAIL: filter_live_pids fixture {label} expected={expected:?} got={got:?}"
        ));
    }

    passed
}

fn sample_socket_fixture(
    log: &LogArtifact,
    label: &str,
    expected_code: u8,
    lsof: FakeLsof,
) -> bool {
    let enumerator = FakeProcessEnumerator::default();
    let outcome = sample_once(
        &format!("fixture-{label}"),
        std::process::id(),
        &enumerator,
        &lsof,
        Duration::from_secs(1),
        &|line| log.write(line),
    );

    let got_code = outcome_code(&outcome);
    if got_code == expected_code {
        log.write(&format!(
            "PASS: sample_sockets fixture {label} (rc={got_code})"
        ));
        true
    } else {
        log.write(&format!(
            "FAIL: sample_sockets fixture {label} expected={expected_code} got={got_code}"
        ));
        false
    }
}

fn retry_benign_no_matching_files_fixture(log: &LogArtifact, live_pid: u32, dead_pid: u32) -> bool {
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
        &|line| log.write(line),
    );
    let got_code = outcome_code(&outcome);

    if got_code == 0 {
        log.write("PASS: sample_sockets fixture retry-benign-no-matching-files (rc=0)");
        true
    } else {
        log.write(&format!(
            "FAIL: sample_sockets fixture retry-benign-no-matching-files expected=0 got={got_code}"
        ));
        false
    }
}

fn fake_evidence_failure() -> FakeLsof {
    FakeLsof::new(vec![LsofOutput::failure(
        1,
        fixture_stream("external-connected").expect("fixture"),
        b"lsof: synthetic stderr about a dead PID".to_vec(),
    )])
}

fn outcome_code(outcome: &SampleOutcome) -> u8 {
    match outcome {
        SampleOutcome::Ok => 0,
        SampleOutcome::Policy { .. } => 1,
        SampleOutcome::Instrument { .. } => 2,
    }
}

fn spawn_dead_pid() -> u32 {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(":")
        .spawn()
        .expect("spawn short-lived process");
    let pid = child.id();
    let _ = child.wait();
    pid
}
