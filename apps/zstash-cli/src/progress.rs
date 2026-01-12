//! Progress bar display for sync operations.

use std::io::IsTerminal as _;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use zstash_core::domain::{SyncPhase, SyncProgress};

/// Create a progress bar for sync operations.
pub fn create_sync_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}% {msg}")
            .expect("valid template")
            .progress_chars("=>-"),
    );

    // Avoid emitting terminal control sequences when output is piped (e.g. benchmarks).
    if std::io::stderr().is_terminal() {
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
    } else {
        pb.set_draw_target(ProgressDrawTarget::hidden());
    }
    pb
}

/// Update progress bar with sync progress.
pub fn update_sync_progress(pb: &ProgressBar, progress: &SyncProgress) {
    pb.set_position(progress.progress_percent as u64);

    let phase_str = match progress.phase {
        SyncPhase::Idle => "Idle",
        SyncPhase::Preparing => "Preparing",
        SyncPhase::Downloading => "Downloading",
        SyncPhase::Scanning => "Scanning",
        SyncPhase::Enhancing => "Enhancing",
        SyncPhase::CatchingUp => "Catching up",
    };

    let eta = progress
        .eta_seconds
        .map(|s| format!(" (ETA: {})", format_duration(s)))
        .unwrap_or_default();

    let height_info = if progress.scan_frontier_height > 0 {
        format!(" - Block {}", progress.scan_frontier_height)
    } else {
        String::new()
    };

    pb.set_message(format!("{}{}{}", phase_str, height_info, eta));
}

/// Format duration in seconds to human-readable string.
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    }
}

/// Format elapsed time as HH:MM:SS.
fn format_elapsed(elapsed: Duration) -> String {
    let total_secs = elapsed.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Format a progress log line for benchmarking/analysis.
///
/// Output format: `[HH:MM:SS]  15% | Scanning   | 3,450,000 / 3,868,370 |  2604 blk/s | ETA 2m 41s (161s)`
pub fn format_progress_log_line(
    elapsed: Duration,
    progress: &SyncProgress,
    start_height: u32,
    chain_tip: u32,
) -> String {
    let phase_str = match progress.phase {
        SyncPhase::Idle => "Idle",
        SyncPhase::Preparing => "Preparing",
        SyncPhase::Downloading => "Downloading",
        SyncPhase::Scanning => "Scanning",
        SyncPhase::Enhancing => "Enhancing",
        SyncPhase::CatchingUp => "Catching up",
    };

    let current_height = progress.scan_frontier_height;
    let blocks_scanned = current_height.saturating_sub(start_height);

    let rate = if elapsed.as_secs() > 0 {
        blocks_scanned as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    let eta_str = progress
        .eta_seconds
        .map(|s| format!("ETA {} ({}s)", format_duration(s), s))
        .unwrap_or_else(|| "ETA --".to_string());

    format!(
        "[{}] {:>3}% | {:<10} | {:>9} / {:>9} | {:>5.0} blk/s | {}",
        format_elapsed(elapsed),
        progress.progress_percent,
        phase_str,
        format_with_commas(current_height),
        format_with_commas(chain_tip),
        rate,
        eta_str
    )
}

/// Format a number with comma separators.
fn format_with_commas(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}
