//! Progress bar display for sync operations.

use indicatif::{ProgressBar, ProgressStyle};

use zkore_core::domain::{SyncPhase, SyncProgress};

/// Create a progress bar for sync operations.
pub fn create_sync_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}% {msg}")
            .expect("valid template")
            .progress_chars("=>-"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
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
fn format_duration(seconds: u64) -> String {
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
