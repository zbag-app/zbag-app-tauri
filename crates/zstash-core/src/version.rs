//! Application version information.
//!
//! Version is sourced from Cargo.toml (workspace version) at compile time.
//! Git commit hash is captured via build.rs when available.

use serde::{Deserialize, Serialize};

/// Application version from Cargo.toml (workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash (short form) captured at build time.
/// Empty string if not available (e.g., not in a git repo).
pub const GIT_COMMIT: &str = match option_env!("ZSTASH_GIT_COMMIT") {
    Some(commit) => commit,
    None => "",
};

/// Full version string including git commit when available.
/// Format: "X.Y.Z" or "X.Y.Z (abc1234)"
pub fn full_version() -> String {
    if GIT_COMMIT.is_empty() {
        VERSION.to_string()
    } else {
        format!("{VERSION} ({GIT_COMMIT})")
    }
}

/// Structured version information for IPC responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    /// SemVer version string (e.g., "0.1.0")
    pub version: String,
    /// Short git commit hash (e.g., "abc1234"), empty if unavailable
    pub git_commit: String,
    /// Full version string for display (e.g., "0.1.0 (abc1234)")
    pub full_version: String,
}

impl VersionInfo {
    /// Create version info from compile-time constants.
    #[must_use]
    pub fn current() -> Self {
        Self {
            version: VERSION.to_string(),
            git_commit: GIT_COMMIT.to_string(),
            full_version: full_version(),
        }
    }
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self::current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        // Version should be a valid semver (at minimum X.Y.Z format)
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(parts.len() >= 3, "version should have at least 3 parts");
        for part in &parts[..3] {
            assert!(
                part.parse::<u32>().is_ok(),
                "version parts should be numeric"
            );
        }
    }

    #[test]
    fn version_info_current() {
        let info = VersionInfo::current();
        assert_eq!(info.version, VERSION);
        assert_eq!(info.git_commit, GIT_COMMIT);
        assert!(!info.full_version.is_empty());
    }

    #[test]
    fn full_version_format() {
        let full = full_version();
        assert!(full.starts_with(VERSION));
        if !GIT_COMMIT.is_empty() {
            assert!(full.contains(GIT_COMMIT));
        }
    }
}
