use std::path::{Path, PathBuf};

use anyhow::Context as _;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt as _};

#[derive(Debug)]
pub struct LoggingGuard {
    _guard: WorkerGuard,
    log_directory: PathBuf,
    current_log_file: PathBuf,
}

impl LoggingGuard {
    pub fn log_directory(&self) -> &Path {
        &self.log_directory
    }

    pub fn current_log_file(&self) -> &Path {
        &self.current_log_file
    }
}

#[derive(Clone, Copy)]
pub struct Redacted<T>(pub T);

impl<T> std::fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl<T> std::fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Clone, Copy)]
pub struct RedactedMemo<'a>(pub &'a str);

impl std::fmt::Display for RedactedMemo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED MEMO len={}]", self.0.len())
    }
}

impl std::fmt::Debug for RedactedMemo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

#[derive(Clone, Copy)]
pub struct RedactedAddress<'a>(pub &'a str);

impl std::fmt::Display for RedactedAddress<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const KEEP: usize = 8;
        let prefix: String = self.0.chars().take(KEEP).collect();
        if prefix.chars().count() == self.0.chars().count() {
            f.write_str(&prefix)
        } else {
            write!(f, "{prefix}…")
        }
    }
}

impl std::fmt::Debug for RedactedAddress<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

pub fn redact_memo(memo: &str) -> RedactedMemo<'_> {
    RedactedMemo(memo)
}

pub fn redact_address(address: &str) -> RedactedAddress<'_> {
    RedactedAddress(address)
}

pub fn init_logging() -> anyhow::Result<LoggingGuard> {
    let log_directory = default_log_directory()?;
    std::fs::create_dir_all(&log_directory).with_context(|| {
        format!(
            "failed to create log directory: {}",
            log_directory.display()
        )
    })?;

    cleanup_old_logs(&log_directory, 7);

    let file_appender = tracing_appender::rolling::daily(&log_directory, "zkore");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| default_env_filter());

    let writer: BoxMakeWriter = if cfg!(debug_assertions) {
        BoxMakeWriter::new(non_blocking.and(std::io::stderr))
    } else {
        BoxMakeWriter::new(non_blocking)
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(writer)
        .with_file(cfg!(debug_assertions))
        .with_line_number(cfg!(debug_assertions))
        .with_target(cfg!(debug_assertions))
        .with_ansi(false)
        .try_init()
        .map_err(|e| anyhow::anyhow!(e))?;

    let current_log_file = current_log_file_path(&log_directory);
    Ok(LoggingGuard {
        _guard: guard,
        log_directory,
        current_log_file,
    })
}

fn default_env_filter() -> tracing_subscriber::EnvFilter {
    if cfg!(debug_assertions) {
        tracing_subscriber::EnvFilter::new(
            "info,zkore_engine=debug,zkore_network=debug,zkore_tor=debug,zkore_app_tauri_lib=debug",
        )
    } else {
        tracing_subscriber::EnvFilter::new("info")
    }
}

fn default_log_directory() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".zkore").join("logs"))
}

fn current_log_file_path(log_directory: &Path) -> PathBuf {
    // Best-effort name; tracing-appender uses `{prefix}.{date}.log`.
    let today = chrono::Utc::now().date_naive();
    log_directory.join(format!("zkore.{today}.log"))
}

fn cleanup_old_logs(log_directory: &Path, days_to_keep: i64) {
    let Ok(entries) = std::fs::read_dir(log_directory) else {
        return;
    };

    let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(days_to_keep);

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if !name.starts_with("zkore.") || !name.ends_with(".log") {
            continue;
        }

        let date_str = name.trim_start_matches("zkore.").trim_end_matches(".log");
        let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
            continue;
        };

        if date < cutoff && let Err(e) = std::fs::remove_file(&path) {
            tracing::debug!(path = ?path, error = ?e, "failed to cleanup old log file");
        }
    }
}
