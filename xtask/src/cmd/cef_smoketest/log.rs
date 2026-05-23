use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};

use chrono::Utc;
use tempfile::NamedTempFile;

pub struct LogArtifact {
    artifact_path: PathBuf,
    run_log: NamedTempFile,
    copied: Once,
    write_lock: Mutex<()>,
}

impl LogArtifact {
    pub fn new(log_path: Option<PathBuf>) -> io::Result<Self> {
        Ok(Self {
            artifact_path: resolve_artifact_path(log_path, |key| std::env::var(key).ok()),
            run_log: NamedTempFile::new()?,
            copied: Once::new(),
            write_lock: Mutex::new(()),
        })
    }

    pub fn write(&self, message: &str) {
        let line = format!("{} {message}", Utc::now().format("%Y-%m-%dT%H:%M:%SZ"));
        let _guard = self.write_lock.lock().ok();

        if let Ok(mut file) = OpenOptions::new().append(true).open(self.run_log.path()) {
            let _ = writeln!(file, "{line}");
        }
        eprintln!("{line}");
    }

    pub fn copy_once(&self) {
        self.copied.call_once(|| {
            if let Some(parent) = self.artifact_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            if self.run_log.path().is_file() {
                let _ = fs::copy(self.run_log.path(), &self.artifact_path);
            } else {
                let _ = fs::File::create(&self.artifact_path);
            }
        });
    }

    pub fn artifact_path(&self) -> &Path {
        &self.artifact_path
    }

    pub fn run_log_path(&self) -> &Path {
        self.run_log.path()
    }
}

impl Drop for LogArtifact {
    fn drop(&mut self) {
        self.copy_once();
        println!("{}", self.artifact_path.display());
    }
}

pub fn resolve_artifact_path<F>(flag: Option<PathBuf>, get_env: F) -> PathBuf
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(path) = flag {
        return path;
    }

    let base = get_env("RUNNER_TEMP").unwrap_or_else(|| "/tmp".to_string());
    PathBuf::from(base).join("bagz-cef-smoketest.log")
}

#[cfg(test)]
mod tests {
    use super::{LogArtifact, resolve_artifact_path};

    #[test]
    fn resolves_runner_temp_artifact_path() {
        let path = resolve_artifact_path(None, |key| {
            (key == "RUNNER_TEMP").then(|| "/tmp/bagz-runner".to_string())
        });

        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/bagz-runner/bagz-cef-smoketest.log")
        );
    }

    #[test]
    fn explicit_log_path_wins() {
        let explicit = std::path::PathBuf::from("/tmp/explicit-smoke.log");
        let path =
            resolve_artifact_path(Some(explicit.clone()), |_| Some("/tmp/ignored".to_string()));

        assert_eq!(path, explicit);
    }

    #[test]
    fn drop_copies_log_even_without_explicit_copy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let artifact = tempdir.path().join("artifact.log");
        {
            let log = LogArtifact::new(Some(artifact.clone())).expect("log");
            log.write("hello");
        }

        let copied = std::fs::read_to_string(artifact).expect("artifact copied");
        assert!(copied.contains("hello"));
    }
}
