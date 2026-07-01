use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::cli::CefSmoketestArgs;

use super::exit::ExitCode;
use super::log::LogArtifact;

#[cfg(target_os = "macos")]
use std::fs::{self, OpenOptions};
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::{Child, Command, ExitStatus, Stdio};
#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;
#[cfg(target_os = "macos")]
use std::sync::mpsc;
#[cfg(target_os = "macos")]
use std::thread::{self, JoinHandle};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use super::bundle;
#[cfg(target_os = "macos")]
use super::lsof::RealLsof;
#[cfg(target_os = "macos")]
use super::process::{self, Pgrep};
#[cfg(target_os = "macos")]
use tempfile::TempDir;

#[cfg(not(target_os = "macos"))]
pub fn run_smoke(
    _args: &CefSmoketestArgs,
    log: &Arc<LogArtifact>,
    _signal_requested: Arc<AtomicBool>,
) -> ExitCode {
    log.write(&format!(
        "smoke not implemented for {}",
        std::env::consts::OS
    ));
    ExitCode::Pass
}

#[cfg(target_os = "macos")]
pub fn run_smoke(
    args: &CefSmoketestArgs,
    log: &Arc<LogArtifact>,
    signal_requested: Arc<AtomicBool>,
) -> ExitCode {
    if !lsof_is_available() {
        log.write("error: lsof is required on Darwin");
        return ExitCode::Instrument;
    }

    let app_bundle = args
        .app
        .clone()
        .unwrap_or_else(|| PathBuf::from("target/release/bundle/macos/zbag.app"));
    let Some(app_exe) = bundle::resolve_executable(&app_bundle) else {
        log.write(&format!(
            "error: bundled app executable not found in: {}/Contents/MacOS",
            app_bundle.display()
        ));
        return ExitCode::Instrument;
    };

    let smoke_root = match tempfile::Builder::new()
        .prefix("zbag-cef-smoketest.")
        .tempdir()
    {
        Ok(dir) => dir,
        Err(err) => {
            log.write(&format!("error: failed to create smoke root: {err}"));
            return ExitCode::Instrument;
        }
    };

    if let Err(err) = create_smoke_dirs(smoke_root.path()) {
        log.write(&format!("error: failed to create smoke state dirs: {err}"));
        return ExitCode::Instrument;
    }

    let duration_secs = args.duration_secs.get();
    let hard_timeout_secs = duration_secs + 30;
    let env = smoke_env(smoke_root.path(), args.duration_secs);
    let start = Instant::now();

    log.write(&format!(
        "starting CEF network smoke: app={} duration={}s timeout={}s",
        app_exe.display(),
        duration_secs,
        hard_timeout_secs
    ));

    let mut command = Command::new(&app_exe);
    command.envs(&env);
    match open_log_stdio(log) {
        Ok((stdout, stderr)) => {
            command.stdout(stdout);
            command.stderr(stderr);
        }
        Err(err) => {
            log.write(&format!("error: failed to open smoke run log: {err}"));
            return ExitCode::Instrument;
        }
    }

    let child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            log.write(&format!("error: failed to spawn app: {err}"));
            return ExitCode::Instrument;
        }
    };

    let mut session = SmokeSession::new(
        smoke_root,
        child,
        args.duration_secs,
        args.lsof_timeout_secs,
        Arc::clone(log),
        Arc::clone(&signal_requested),
    );

    let wait = match wait_for_child(&mut session.child, &signal_requested) {
        Ok(outcome) => outcome,
        Err(err) => {
            log.write(&format!("FAIL: app wait failed: {err}"));
            return ExitCode::Instrument;
        }
    };
    let app_elapsed = start.elapsed().as_secs();

    session.stop_helpers_and_join();

    match wait {
        WaitOutcome::Signaled => {
            log.write("FAIL: signal received during smoke");
            ExitCode::Instrument
        }
        WaitOutcome::Exited(status) => post_wait_checks(
            session.smoke_root.path(),
            status,
            app_elapsed,
            start.elapsed().as_secs(),
            args.duration_secs,
            hard_timeout_secs,
            log,
        ),
    }
}

#[cfg(target_os = "macos")]
struct SmokeSession {
    smoke_root: TempDir,
    child: Child,
    sampler_handle: Option<JoinHandle<()>>,
    watchdog_handle: Option<JoinHandle<()>>,
    watchdog_waker: Option<mpsc::Sender<()>>,
    stop_helpers: Arc<AtomicBool>,
    log: Arc<LogArtifact>,
}

#[cfg(target_os = "macos")]
impl SmokeSession {
    fn new(
        smoke_root: TempDir,
        child: Child,
        duration_secs: NonZeroU32,
        lsof_timeout_secs: NonZeroU32,
        log: Arc<LogArtifact>,
        signal_requested: Arc<AtomicBool>,
    ) -> Self {
        let root_pid = child.id();
        let stop_helpers = Arc::new(AtomicBool::new(false));
        let (watchdog_tx, watchdog_rx) = mpsc::channel();

        let watchdog_root = smoke_root.path().to_path_buf();
        let watchdog_stop = Arc::clone(&stop_helpers);
        let watchdog_signal = Arc::clone(&signal_requested);
        let watchdog_log = Arc::clone(&log);
        let watchdog_handle = thread::spawn(move || {
            let timeout = Duration::from_secs(u64::from(duration_secs.get() + 30));
            match watchdog_rx.recv_timeout(timeout) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if watchdog_stop.load(Ordering::SeqCst)
                        || watchdog_signal.load(Ordering::SeqCst)
                    {
                        return;
                    }
                    watchdog_log.write(&format!(
                        "WATCHDOG: hard timeout after {}s",
                        duration_secs.get() + 30
                    ));
                    if let Err(err) = fs::write(watchdog_root.join("watchdog-fired"), b"") {
                        watchdog_log
                            .write(&format!("ERROR: failed to write watchdog sentinel: {err}"));
                    }
                    process::kill_tree(root_pid, &Pgrep);
                }
            }
        });

        let sampler_root = smoke_root.path().to_path_buf();
        let sampler_stop = Arc::clone(&stop_helpers);
        let sampler_signal = Arc::clone(&signal_requested);
        let sampler_log = Arc::clone(&log);
        let sampler_handle = thread::spawn(move || {
            let enumerator = Pgrep;
            let lsof = RealLsof;
            super::sampler::run_loop(
                root_pid,
                &sampler_root,
                &enumerator,
                &lsof,
                Duration::from_secs(u64::from(lsof_timeout_secs.get())),
                super::sampler::LoopControls {
                    stop_helpers: sampler_stop,
                    signal_requested: sampler_signal,
                },
                sampler_log,
            );
        });

        Self {
            smoke_root,
            child,
            sampler_handle: Some(sampler_handle),
            watchdog_handle: Some(watchdog_handle),
            watchdog_waker: Some(watchdog_tx),
            stop_helpers,
            log,
        }
    }

    fn stop_helpers_and_join(&mut self) {
        self.stop_helpers.store(true, Ordering::SeqCst);
        self.watchdog_waker.take();

        if let Some(handle) = self.sampler_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.watchdog_handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for SmokeSession {
    fn drop(&mut self) {
        self.stop_helpers_and_join();
        process::kill_tree(self.child.id(), &Pgrep);
        let _ = self.child.wait();
        self.log.copy_once();
    }
}

#[cfg(target_os = "macos")]
enum WaitOutcome {
    Exited(ExitStatus),
    Signaled,
}

#[cfg(target_os = "macos")]
fn wait_for_child(
    child: &mut Child,
    signal_requested: &AtomicBool,
) -> std::io::Result<WaitOutcome> {
    loop {
        if signal_requested.load(Ordering::SeqCst) {
            return Ok(WaitOutcome::Signaled);
        }
        if let Some(status) = child.try_wait()? {
            return Ok(WaitOutcome::Exited(status));
        }
        thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(target_os = "macos")]
fn post_wait_checks(
    smoke_root: &Path,
    status: ExitStatus,
    app_elapsed: u64,
    total_elapsed: u64,
    duration_secs: NonZeroU32,
    hard_timeout_secs: u32,
    log: &LogArtifact,
) -> ExitCode {
    let lower_bound = u64::from(duration_secs.get().saturating_sub(2));
    let upper_bound = u64::from(duration_secs.get() + 5);

    if smoke_root.join("watchdog-fired").is_file() {
        log.write(&format!(
            "FAIL: app hung until watchdog fired after {hard_timeout_secs}s"
        ));
        return ExitCode::Instrument;
    }
    if !smoke_root.join("smoke-ready").is_file() {
        log.write(&format!(
            "FAIL: app exited before CEF smoke setup wrote readiness sentinel status={:?} elapsed={total_elapsed}s",
            status.code()
        ));
        return ExitCode::Instrument;
    }
    if app_elapsed < lower_bound {
        log.write(&format!(
            "FAIL: app exited too early status={:?} app_elapsed={app_elapsed}s total_elapsed={total_elapsed}s expected_min={lower_bound}s",
            status.code()
        ));
        return ExitCode::Instrument;
    }
    if app_elapsed > upper_bound {
        log.write(&format!(
            "FAIL: app exited too late status={:?} app_elapsed={app_elapsed}s total_elapsed={total_elapsed}s expected_max={upper_bound}s",
            status.code()
        ));
        return ExitCode::Instrument;
    }
    if !status.success() {
        log.write(&format!(
            "FAIL: app exited non-zero status={:?} elapsed={total_elapsed}s",
            status.code()
        ));
        return ExitCode::Instrument;
    }
    if smoke_root.join("instrumentation-failure").is_file() {
        log.write("FAIL: lsof failed during sampling");
        return ExitCode::Instrument;
    }
    if smoke_root.join("network-failure").is_file() {
        log.write("FAIL: non-loopback CEF socket observed");
        return ExitCode::Policy;
    }

    log.write("PASS: CEF network smoke observed no non-loopback sockets");
    ExitCode::Pass
}

pub fn smoke_env(root: &Path, duration_secs: NonZeroU32) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("HOME".to_string(), root.join("home").display().to_string()),
        (
            "XDG_CACHE_HOME".to_string(),
            root.join("cache").display().to_string(),
        ),
        (
            "XDG_CONFIG_HOME".to_string(),
            root.join("config").display().to_string(),
        ),
        (
            "XDG_DATA_HOME".to_string(),
            root.join("data").display().to_string(),
        ),
        (
            "XDG_STATE_HOME".to_string(),
            root.join("state").display().to_string(),
        ),
        ("TMPDIR".to_string(), root.join("tmp").display().to_string()),
        (
            "ZBAG_GRPC_URL".to_string(),
            "https://127.0.0.1:1".to_string(),
        ),
        ("ZBAG_HEADLESS_SMOKE".to_string(), "1".to_string()),
        (
            "ZBAG_SMOKE_DURATION_SECS".to_string(),
            duration_secs.get().to_string(),
        ),
        (
            "ZBAG_SMOKE_READY_FILE".to_string(),
            root.join("smoke-ready").display().to_string(),
        ),
        ("ZBAG_USE_SYSTEM_KEYCHAIN".to_string(), "0".to_string()),
    ])
}

#[cfg(target_os = "macos")]
fn lsof_is_available() -> bool {
    Command::new("lsof")
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

#[cfg(target_os = "macos")]
fn create_smoke_dirs(root: &Path) -> std::io::Result<()> {
    for dir in ["home", "cache", "config", "data", "state", "tmp"] {
        fs::create_dir_all(root.join(dir))?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_log_stdio(log: &LogArtifact) -> std::io::Result<(Stdio, Stdio)> {
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log.run_log_path())?;
    let stderr = stdout.try_clone()?;
    Ok((Stdio::from(stdout), Stdio::from(stderr)))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::smoke_env;

    #[test]
    fn smoke_env_preserves_app_isolation_contract() {
        let duration = NonZeroU32::new(15).expect("non-zero");
        let env = smoke_env(std::path::Path::new("/tmp/zbag-smoke"), duration);

        assert_eq!(env["ZBAG_GRPC_URL"], "https://127.0.0.1:1");
        assert_eq!(env["ZBAG_HEADLESS_SMOKE"], "1");
        assert_eq!(env["ZBAG_USE_SYSTEM_KEYCHAIN"], "0");
        assert_eq!(env["ZBAG_SMOKE_DURATION_SECS"], "15");
        assert_eq!(env["ZBAG_SMOKE_READY_FILE"], "/tmp/zbag-smoke/smoke-ready");
        assert_eq!(env["HOME"], "/tmp/zbag-smoke/home");
        assert_eq!(env["XDG_CACHE_HOME"], "/tmp/zbag-smoke/cache");
        assert_eq!(env["XDG_CONFIG_HOME"], "/tmp/zbag-smoke/config");
        assert_eq!(env["XDG_DATA_HOME"], "/tmp/zbag-smoke/data");
        assert_eq!(env["XDG_STATE_HOME"], "/tmp/zbag-smoke/state");
        assert_eq!(env["TMPDIR"], "/tmp/zbag-smoke/tmp");
    }
}
