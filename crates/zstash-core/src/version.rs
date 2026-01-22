//! Application version information.
//!
//! Version is sourced from Cargo.toml (workspace version) at compile time.
//! Git commit hash, build timestamp, and release status are captured via build.rs.

use serde::{Deserialize, Serialize};

/// Application version from Cargo.toml (workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash (short form) captured at build time.
/// Empty string if not available (e.g., not in a git repo).
pub const GIT_COMMIT: &str = match option_env!("ZSTASH_GIT_COMMIT") {
    Some(commit) => commit,
    None => "",
};

/// Git describe output captured at build time.
/// Format: "vX.Y.Z" for exact releases, "vX.Y.Z-N-gHASH" for post-release builds.
/// Empty string if not available.
pub const GIT_DESCRIBE: &str = match option_env!("ZSTASH_GIT_DESCRIBE") {
    Some(desc) => desc,
    None => "",
};

/// Build timestamp in UTC (e.g., "2026-01-22 14:30:00 UTC").
pub const BUILD_TIMESTAMP: &str = match option_env!("ZSTASH_BUILD_TIMESTAMP") {
    Some(ts) => ts,
    None => "",
};

/// Whether this is a release build (HEAD is exactly on a version tag).
pub fn is_release() -> bool {
    option_env!("ZSTASH_IS_RELEASE") == Some("true")
}

/// Full version string for display.
/// For release builds: "X.Y.Z"
/// For post-release builds: "X.Y.Z-N-gHASH" (from git describe, with 'v' prefix stripped)
pub fn full_version() -> String {
    if is_release() || GIT_DESCRIBE.is_empty() {
        VERSION.to_string()
    } else {
        // Strip leading 'v' from git describe output if present
        GIT_DESCRIBE
            .strip_prefix('v')
            .unwrap_or(GIT_DESCRIBE)
            .to_string()
    }
}

/// Structured version information for IPC responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    /// SemVer version string (e.g., "0.1.0")
    pub version: String,
    /// Short git commit hash (e.g., "abc1234"), None for release builds
    pub git_commit: Option<String>,
    /// Git describe output (e.g., "v0.1.0-3-gabc1234"), None if not available
    pub git_describe: Option<String>,
    /// Build timestamp in UTC (e.g., "2026-01-22 14:30:00 UTC")
    pub build_timestamp: String,
    /// Full version string for display (e.g., "0.1.0" or "0.1.0-3-gabc1234")
    pub full_version: String,
}

impl VersionInfo {
    /// Create version info from compile-time constants.
    #[must_use]
    pub fn current() -> Self {
        // For release builds, don't expose git commit
        let git_commit = if is_release() || GIT_COMMIT.is_empty() {
            None
        } else {
            Some(GIT_COMMIT.to_string())
        };

        let git_describe = if GIT_DESCRIBE.is_empty() {
            None
        } else {
            Some(GIT_DESCRIBE.to_string())
        };

        Self {
            version: VERSION.to_string(),
            git_commit,
            git_describe,
            build_timestamp: BUILD_TIMESTAMP.to_string(),
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
        assert!(!info.full_version.is_empty());
        assert!(!info.build_timestamp.is_empty());
    }

    #[test]
    fn full_version_format() {
        let full = full_version();
        // Full version should either be exactly VERSION (release) or derived from git describe
        if is_release() || GIT_DESCRIBE.is_empty() {
            assert_eq!(full, VERSION);
        } else {
            // For non-release builds, full version is git describe with 'v' stripped
            let expected = GIT_DESCRIBE.strip_prefix('v').unwrap_or(GIT_DESCRIBE);
            assert_eq!(full, expected);
        }
    }

    #[test]
    fn build_timestamp_format() {
        // Guard against empty timestamp
        assert!(
            !BUILD_TIMESTAMP.is_empty(),
            "BUILD_TIMESTAMP is empty - build.rs may not have run or ENV var missing"
        );
        // Build timestamp should be in UTC format
        assert!(
            BUILD_TIMESTAMP.ends_with("UTC"),
            "BUILD_TIMESTAMP should end with UTC, got: {BUILD_TIMESTAMP}"
        );
    }
}
