use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;
use std::time::Duration;

#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
#[cfg(target_os = "macos")]
use std::sync::mpsc;
#[cfg(target_os = "macos")]
use std::thread;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LsofOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
}

impl LsofOutput {
    pub fn success(stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 0,
            stdout: stdout.into(),
            stderr: Vec::new(),
            timed_out: false,
        }
    }

    pub fn failure(status: i32, stdout: impl Into<Vec<u8>>, stderr: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            stdout: stdout.into(),
            stderr: stderr.into(),
            timed_out: false,
        }
    }

    pub fn timeout(stdout: impl Into<Vec<u8>>, stderr: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 1,
            stdout: stdout.into(),
            stderr: stderr.into(),
            timed_out: true,
        }
    }
}

pub trait LsofRunner: Send + Sync {
    fn run(&self, pids: &[u32], timeout: Duration) -> io::Result<LsofOutput>;
}

#[derive(Debug, Default)]
pub struct FakeLsof {
    responses: Mutex<VecDeque<LsofOutput>>,
    calls: Mutex<Vec<Vec<u32>>>,
}

impl FakeLsof {
    pub fn new(responses: Vec<LsofOutput>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<Vec<u32>> {
        self.calls
            .lock()
            .map_or_else(|_| Vec::new(), |calls| calls.clone())
    }
}

impl LsofRunner for FakeLsof {
    fn run(&self, pids: &[u32], _timeout: Duration) -> io::Result<LsofOutput> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(pids.to_vec());
        }

        self.responses
            .lock()
            .map_err(|_| io::Error::other("fake lsof lock poisoned"))?
            .pop_front()
            .ok_or_else(|| io::Error::other("fake lsof response exhausted"))
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
pub struct RealLsof;

#[cfg(target_os = "macos")]
impl RealLsof {
    pub fn build_command(pids: &[u32]) -> Command {
        let pid_csv = pids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut command = Command::new("lsof");
        command.args([
            "-nP", "-a", "-p", &pid_csv, "-iTCP", "-iUDP", "-F", "pcPTn0",
        ]);
        command
    }
}

#[cfg(target_os = "macos")]
impl LsofRunner for RealLsof {
    fn run(&self, pids: &[u32], timeout: Duration) -> io::Result<LsofOutput> {
        let child = Self::build_command(pids)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let child_id = child.id();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let output = child.wait_with_output();
            let _ = tx.send(output);
        });

        match rx.recv_timeout(timeout) {
            Ok(output) => {
                let output = output?;
                Ok(LsofOutput {
                    status: output.status.code().unwrap_or(1),
                    stdout: output.stdout,
                    stderr: output.stderr,
                    timed_out: false,
                })
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = Command::new("kill")
                    .args(["-TERM", &child_id.to_string()])
                    .status();
                thread::sleep(Duration::from_secs(1));
                let _ = Command::new("kill")
                    .args(["-KILL", &child_id.to_string()])
                    .status();

                let output = rx
                    .recv()
                    .map_err(|_| io::Error::other("lsof watchdog thread disconnected"))??;
                Ok(LsofOutput {
                    status: output.status.code().unwrap_or(1),
                    stdout: output.stdout,
                    stderr: output.stderr,
                    timed_out: true,
                })
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(io::Error::other("lsof watchdog thread disconnected"))
            }
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::RealLsof;

    #[test]
    fn real_lsof_command_matches_bash_argv() {
        let command = RealLsof::build_command(&[1234, 5678]);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(command.get_program(), "lsof");
        assert_eq!(
            args,
            vec![
                "-nP",
                "-a",
                "-p",
                "1234,5678",
                "-iTCP",
                "-iUDP",
                "-F",
                "pcPTn0"
            ]
        );
    }
}
