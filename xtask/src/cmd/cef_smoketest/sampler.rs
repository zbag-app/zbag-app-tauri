use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use super::log::LogArtifact;
use super::lsof::LsofRunner;
use super::parser::{SocketViolation, classify_lsof_fields};
use super::process::{self, ProcessEnumerator};

#[derive(Clone, Debug)]
pub struct LoopControls {
    pub stop_helpers: Arc<AtomicBool>,
    pub signal_requested: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SampleOutcome {
    Ok,
    Policy { evidence: Vec<SocketViolation> },
    Instrument { reason: String },
}

impl SampleOutcome {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

pub fn sample_once(
    sample: &str,
    root_pid: u32,
    enumerator: &dyn ProcessEnumerator,
    lsof: &dyn LsofRunner,
    timeout: Duration,
    log: &dyn Fn(&str),
) -> SampleOutcome {
    let pid_csv = process::process_tree(root_pid, enumerator);
    if pid_csv.is_empty() {
        return SampleOutcome::Ok;
    }

    let first = match lsof.run(&pid_csv, timeout) {
        Ok(output) => output,
        Err(err) => {
            let reason = format!("lsof spawn failed sample={sample}: {err}");
            log(&format!("ERROR: {reason}"));
            return SampleOutcome::Instrument { reason };
        }
    };

    let violations = classify_lsof_fields(sample, &first.stdout);
    if !violations.is_empty() {
        log_policy_failures(&violations, log);
        return SampleOutcome::Policy {
            evidence: violations,
        };
    }

    if first.timed_out {
        let reason = format!(
            "lsof timed out sample={sample} timeout={}s",
            timeout.as_secs()
        );
        log(&format!("ERROR: {reason}"));
        return SampleOutcome::Instrument { reason };
    }
    if first.status == 0 || first.stderr.is_empty() {
        return SampleOutcome::Ok;
    }

    let live_csv = process::filter_live_pids(&pid_csv);
    if live_csv.is_empty() {
        return SampleOutcome::Ok;
    }
    if live_csv == pid_csv {
        let stderr = String::from_utf8_lossy(&first.stderr).replace('\n', " ");
        let reason = format!(
            "lsof instrumentation failed sample={sample} status={} stderr={stderr}",
            first.status
        );
        log(&format!("ERROR: {reason}"));
        return SampleOutcome::Instrument { reason };
    }

    let retry = match lsof.run(&live_csv, timeout) {
        Ok(output) => output,
        Err(err) => {
            let reason = format!("lsof retry spawn failed sample={sample}: {err}");
            log(&format!("ERROR: {reason}"));
            return SampleOutcome::Instrument { reason };
        }
    };

    let retry_violations = classify_lsof_fields(sample, &retry.stdout);
    if !retry_violations.is_empty() {
        log_policy_failures(&retry_violations, log);
        return SampleOutcome::Policy {
            evidence: retry_violations,
        };
    }
    if retry.timed_out {
        let reason = format!("lsof timed out (retry) sample={sample}");
        log(&format!("ERROR: {reason}"));
        return SampleOutcome::Instrument { reason };
    }
    if retry.status == 0 || retry.stderr.is_empty() {
        return SampleOutcome::Ok;
    }

    let reason = format!(
        "lsof instrumentation failed (retry) sample={sample} status={}",
        retry.status
    );
    log(&format!("ERROR: {reason}"));
    SampleOutcome::Instrument { reason }
}

pub fn run_loop(
    root_pid: u32,
    smoke_root: &Path,
    enumerator: &dyn ProcessEnumerator,
    lsof: &dyn LsofRunner,
    timeout: Duration,
    controls: LoopControls,
    log: Arc<LogArtifact>,
) {
    let mut sample = 0;
    while !controls.stop_helpers.load(Ordering::SeqCst)
        && !controls.signal_requested.load(Ordering::SeqCst)
    {
        if !process::pid_is_live(root_pid) {
            break;
        }

        sample += 1;
        match sample_once(
            &format!("sample-{sample}"),
            root_pid,
            enumerator,
            lsof,
            timeout,
            &|line| log.write(line),
        ) {
            SampleOutcome::Ok => {}
            SampleOutcome::Policy { .. } => touch_sentinel(smoke_root, "network-failure", &log),
            SampleOutcome::Instrument { .. } => {
                touch_sentinel(smoke_root, "instrumentation-failure", &log);
            }
        }

        for _ in 0..10 {
            if controls.stop_helpers.load(Ordering::SeqCst)
                || controls.signal_requested.load(Ordering::SeqCst)
            {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn log_policy_failures(violations: &[SocketViolation], log: &dyn Fn(&str)) {
    for violation in violations {
        log(&format!("FAIL: {violation}"));
    }
}

fn touch_sentinel(root: &Path, name: &str, log: &LogArtifact) {
    if let Err(err) = fs::write(root.join(name), b"") {
        log.write(&format!("ERROR: failed to write {name} sentinel: {err}"));
    }
}
