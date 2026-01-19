//! Filesystem permission hardening for wallet data.
//!
//! On Unix/macOS:
//! - Directories are created with mode 0700 (owner read/write/execute only)
//! - Files are created with mode 0600 (owner read/write only)
//!
//! On Windows, these functions are no-ops (permissions not enforced).

use std::io;
use std::path::Path;

/// Create a directory with secure permissions (0700 on Unix).
///
/// On Unix, permissions are set atomically during creation using DirBuilder,
/// preventing any window where the directory exists with broader permissions.
///
/// Returns an error if the directory already exists.
pub fn create_dir_secure(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)
    }

    #[cfg(not(unix))]
    {
        std::fs::create_dir(path)
    }
}

/// Recursively create directories with secure permissions (0700 on Unix).
///
/// Each directory in the path hierarchy is created with restrictive permissions.
pub fn create_dir_all_secure(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();

    // Collect all path components that need to be created
    let mut to_create = Vec::new();
    let mut current = path;
    while !current.exists() {
        to_create.push(current.to_path_buf());
        match current.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => current = parent,
            _ => break,
        }
    }

    // Check if path already exists before consuming to_create
    let path_already_existed = to_create.is_empty();

    // Create directories from root to leaf, setting permissions on each
    for dir in to_create.into_iter().rev() {
        // Use DirBuilder with mode on Unix for atomic creation with correct permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // Directory was created between our check and create - that's fine
                    // Still try to set permissions (best-effort)
                    let _ = set_dir_permissions(&dir);
                }
                Err(e) => return Err(e),
            }
        }

        #[cfg(not(unix))]
        {
            match std::fs::create_dir(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // Directory was created between our check and create - that's fine
                }
                Err(e) => return Err(e),
            }
        }
    }

    // If path already existed, try to set permissions (best-effort)
    // We use best-effort here because the directory might be a system directory
    // that we cannot modify (e.g., /tmp or /var/folders/...)
    if path_already_existed && path.is_dir() {
        let _ = set_dir_permissions(path);
    }

    Ok(())
}

/// Recursively create directories with secure permissions (0700 on Unix).
///
/// This is an async version that uses tokio::fs for non-blocking IO.
#[cfg(feature = "async")]
pub async fn create_dir_all_secure_async(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref().to_path_buf();

    // Collect all path components that need to be created
    let mut to_create = Vec::new();
    let mut current = path.as_path();
    while !tokio::fs::try_exists(&current).await.unwrap_or(false) {
        to_create.push(current.to_path_buf());
        match current.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => current = parent,
            _ => break,
        }
    }

    // Check if path already exists before consuming to_create
    let path_already_existed = to_create.is_empty();

    // Create directories from root to leaf, setting permissions on each
    for dir in to_create.into_iter().rev() {
        // Use DirBuilder with mode on Unix for atomic creation with correct permissions
        // We use spawn_blocking since there's no async DirBuilder with mode support
        #[cfg(unix)]
        {
            let dir_clone = dir.clone();
            let result = tokio::task::spawn_blocking(move || {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new().mode(0o700).create(&dir_clone)
            })
            .await
            .map_err(io::Error::other)?;

            match result {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // Directory was created between our check and create - that's fine
                    // Still try to set permissions (best-effort)
                    let _ = set_dir_permissions(&dir);
                }
                Err(e) => return Err(e),
            }
        }

        #[cfg(not(unix))]
        {
            match tokio::fs::create_dir(&dir).await {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // Directory was created between our check and create - that's fine
                }
                Err(e) => return Err(e),
            }
        }
    }

    // If path already existed, try to set permissions (best-effort)
    // We use best-effort here because the directory might be a system directory
    // that we cannot modify (e.g., /tmp or /var/folders/...)
    if path_already_existed
        && tokio::fs::metadata(&path)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
    {
        let _ = set_dir_permissions(&path);
    }

    Ok(())
}

/// Write data to a file with secure permissions (0600 on Unix).
///
/// The file is created with restrictive permissions atomically on Unix
/// to prevent race conditions where the file briefly exists with broader permissions.
pub fn write_file_secure(path: impl AsRef<Path>, contents: &[u8]) -> io::Result<()> {
    let path = path.as_ref();

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true).mode(0o600);
        let mut file = opts.open(path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, contents)?;
        // Best-effort permission setting on non-Unix
        let _ = set_file_permissions(path);
        Ok(())
    }
}

/// Set directory permissions to 0700 (owner-only access) on Unix.
///
/// On non-Unix platforms, this is a no-op (best-effort).
pub fn set_dir_permissions(path: impl AsRef<Path>) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(path, perms)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Set file permissions to 0600 (owner read/write only) on Unix.
///
/// On non-Unix platforms, this is a no-op (best-effort).
pub fn set_file_permissions(path: impl AsRef<Path>) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Open a file for writing with secure permissions (0600 on Unix).
///
/// Returns an `OpenOptions` configured for secure file creation.
#[cfg(unix)]
pub fn secure_open_options() -> std::fs::OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true).mode(0o600);
    opts
}

#[cfg(not(unix))]
pub fn secure_open_options() -> std::fs::OpenOptions {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    opts
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_create_dir_secure_sets_0700() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("secure_dir");

        create_dir_secure(&dir).unwrap();

        let meta = std::fs::metadata(&dir).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "directory should have mode 0700");
    }

    #[test]
    fn test_create_dir_all_secure_sets_0700_on_all() {
        let tmp = tempdir().unwrap();
        let nested = tmp.path().join("a").join("b").join("c");

        create_dir_all_secure(&nested).unwrap();

        // Check each directory in the chain has 0700
        for dir in [
            tmp.path().join("a"),
            tmp.path().join("a").join("b"),
            tmp.path().join("a").join("b").join("c"),
        ] {
            let meta = std::fs::metadata(&dir).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "directory {:?} should have mode 0700", dir);
        }
    }

    #[test]
    fn test_write_file_secure_sets_0600() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("secure_file.txt");

        write_file_secure(&file, b"secret data").unwrap();

        let meta = std::fs::metadata(&file).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "file should have mode 0600");
    }

    #[test]
    fn test_set_file_permissions() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test_file.txt");

        std::fs::write(&file, b"test").unwrap();
        set_file_permissions(&file).unwrap();

        let meta = std::fs::metadata(&file).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "file should have mode 0600 after set_file_permissions"
        );
    }

    #[test]
    fn test_set_dir_permissions() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("test_dir");

        std::fs::create_dir(&dir).unwrap();
        set_dir_permissions(&dir).unwrap();

        let meta = std::fs::metadata(&dir).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o700,
            "directory should have mode 0700 after set_dir_permissions"
        );
    }

    #[test]
    fn test_secure_open_options_sets_0600() {
        use std::io::Write;
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("secure_open_file.txt");

        let mut f = secure_open_options().open(&file).unwrap();
        f.write_all(b"test").unwrap();

        let meta = std::fs::metadata(&file).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "file should have mode 0600");
    }

    #[test]
    fn test_create_dir_all_secure_on_existing_leaf_sets_0700() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("existing");

        // Create with default permissions first
        std::fs::create_dir(&dir).unwrap();

        // Call create_dir_all_secure - should not error, should set permissions
        create_dir_all_secure(&dir).unwrap();

        let meta = std::fs::metadata(&dir).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "existing directory should have mode 0700");
    }
}

#[cfg(test)]
#[cfg(unix)]
#[cfg(feature = "async")]
mod async_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_dir_all_secure_async_sets_0700() {
        let tmp = tempdir().unwrap();
        let nested = tmp.path().join("a").join("b").join("c");

        create_dir_all_secure_async(&nested).await.unwrap();

        // Check each directory in the chain has 0700
        for dir in [
            tmp.path().join("a"),
            tmp.path().join("a").join("b"),
            tmp.path().join("a").join("b").join("c"),
        ] {
            let meta = std::fs::metadata(&dir).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "directory {:?} should have mode 0700", dir);
        }
    }
}
