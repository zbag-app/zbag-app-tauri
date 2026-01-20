use std::path::{Path, PathBuf};
use std::process::Command;

/// Walk up the directory tree to find the git root (directory containing .git)
fn find_git_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|p| p.join(".git").exists())
        .map(|p| p.to_path_buf())
}

fn main() {
    // Rebuild when frontend assets change
    println!("cargo:rerun-if-changed=../dist");

    // Capture git commit hash at build time
    // Rebuild when HEAD changes (new commits or branch switch)
    // Use CARGO_MANIFEST_DIR to find workspace root reliably
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
        }
    }

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

    tauri_build::build()
}
