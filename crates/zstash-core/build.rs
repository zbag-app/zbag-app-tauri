//! Build script for zstash-core.
//!
//! Captures build metadata as compile-time environment variables:
//! - `ZSTASH_GIT_COMMIT`: Short git commit hash
//! - `ZSTASH_BUILD_TIMESTAMP`: UTC build timestamp
//! - `ZSTASH_IS_RELEASE`: "true" if HEAD is on a version tag, "false" otherwise

use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;

/// Walk up the directory tree to find the git root (directory containing .git)
fn find_git_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|p| p.join(".git").exists())
        .map(|p| p.to_path_buf())
}

fn main() {
    // Capture build timestamp in UTC
    let build_timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    println!("cargo:rustc-env=ZSTASH_BUILD_TIMESTAMP={build_timestamp}");

    // Set up git-related rerun triggers and capture git info
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    if let Some(workspace_root) = find_git_root(Path::new(&manifest_dir)) {
        let git_dir = workspace_root.join(".git");
        let git_head = git_dir.join("HEAD");

        if git_head.exists() {
            // Always watch HEAD for branch switches
            println!("cargo:rerun-if-changed={}", git_head.display());

            // If HEAD is a symbolic ref, also watch the referenced branch file
            if let Ok(head_contents) = std::fs::read_to_string(&git_head) {
                let head_contents = head_contents.trim();
                if let Some(ref_path) = head_contents.strip_prefix("ref: ") {
                    let ref_file = git_dir.join(ref_path);
                    if ref_file.exists() {
                        println!("cargo:rerun-if-changed={}", ref_file.display());
                    }
                }
            }

            // Watch packed-refs as fallback for packed refs
            let packed_refs = git_dir.join("packed-refs");
            if packed_refs.exists() {
                println!("cargo:rerun-if-changed={}", packed_refs.display());
            }

            // Watch tags directory for tag changes
            let tags_dir = git_dir.join("refs").join("tags");
            if tags_dir.exists() {
                println!("cargo:rerun-if-changed={}", tags_dir.display());
            }
        }
    }

    // Capture git commit hash
    let git_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    println!("cargo:rustc-env=ZSTASH_GIT_COMMIT={git_commit}");

    // Capture git describe output for version string
    let git_describe = Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    println!("cargo:rustc-env=ZSTASH_GIT_DESCRIBE={git_describe}");

    // Determine if this is a release build (HEAD is exactly on a version tag)
    // `git describe --exact-match --tags HEAD` succeeds only if HEAD is tagged
    let is_release = Command::new("git")
        .args(["describe", "--exact-match", "--tags", "HEAD"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    println!(
        "cargo:rustc-env=ZSTASH_IS_RELEASE={}",
        if is_release { "true" } else { "false" }
    );
}
