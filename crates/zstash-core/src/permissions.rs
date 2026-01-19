//! Filesystem permission hardening for wallet data.
//!
//! On Unix/macOS:
//! - Directories are created with mode 0700 (owner read/write/execute only)
//! - Files are created with mode 0600 (owner read/write only)
//!
//! On Windows, these functions are no-ops (permissions not enforced).

use std::io;
use std::path::{Path, PathBuf};

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

    // Avoid pre-checking with `Path::exists()` (TOCTOU + extra syscalls). Instead,
    // attempt creation and handle `NotFound` by walking up the parent chain.
    let mut to_create = Vec::new();
    let mut current = path;

    loop {
        match create_dir_secure(current) {
            Ok(()) => break,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if current.is_dir() {
                    if current == path {
                        // Best-effort: the directory might already exist with broader
                        // permissions, or it may have been concurrently created.
                        //
                        // We intentionally don't fail if `chmod`/`set_permissions` fails,
                        // because callers may pass directories we don't own (e.g. system
                        // temp roots on macOS) and this helper is used for both creation
                        // and "ensure exists".
                        let _ = set_dir_permissions(current);
                    }
                    break;
                }

                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("path exists but is not a directory: {}", current.display()),
                ));
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                match current.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => {
                        to_create.push(current.to_path_buf());
                        current = parent;
                    }
                    _ => return Err(e),
                }
            }
            Err(e) => return Err(e),
        }
    }

    // Create directories from root to leaf, setting permissions on each.
    for dir in to_create.into_iter().rev() {
        match create_dir_secure(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if dir.is_dir() {
                    let _ = set_dir_permissions(&dir);
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        format!("path exists but is not a directory: {}", dir.display()),
                    ));
                }
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

/// Recursively create directories with secure permissions (0700 on Unix).
///
/// This is an async wrapper around the sync implementation using `spawn_blocking`.
#[cfg(feature = "async")]
pub async fn create_dir_all_secure_async(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref().to_path_buf();
    tokio::task::spawn_blocking(move || create_dir_all_secure(path))
        .await
        .map_err(io::Error::other)?
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

        // `OpenOptionsExt::mode` applies only when the file is created. If the file
        // already exists, we must `chmod` it to ensure 0600.
        //
        // Avoid a pre-check (`Path::exists`) by attempting a create-new first.
        let mut create_opts = std::fs::OpenOptions::new();
        create_opts.write(true).create_new(true).mode(0o600);

        match create_opts.open(path) {
            Ok(mut file) => {
                file.write_all(contents)?;
                file.sync_all()?;
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let mut overwrite_opts = std::fs::OpenOptions::new();
                overwrite_opts
                    .write(true)
                    .create(true)
                    // Don't truncate on open: ensure we can harden permissions first.
                    .mode(0o600);
                let mut file = overwrite_opts.open(path)?;
                set_file_permissions(path)?;
                file.set_len(0)?;
                file.write_all(contents)?;
                file.sync_all()?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(not(unix))]
    {
        use std::io::Write;

        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        let mut file = opts.open(path)?;
        file.write_all(contents)?;
        file.sync_all()?;
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

/// Set secure file permissions on a SQLite database and its common sidecar files.
///
/// SQLite may create auxiliary files next to the main database file (e.g. `-wal`, `-shm`,
/// `-journal`). This helper applies 0600 (best-effort) to those sidecar files if they exist.
///
/// Note: This does not prevent SQLite from briefly creating sidecar files with the process umask
/// defaults. Prefer placing databases in directories secured with `create_dir_all_secure()`.
pub fn set_sqlite_file_permissions(db_path: impl AsRef<Path>) -> io::Result<()> {
    let db_path = db_path.as_ref();

    set_file_permissions(db_path)?;

    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = db_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let sidecar_path = PathBuf::from(sidecar);
        match set_file_permissions(&sidecar_path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_e) => {
                // Best-effort: sidecar files are opportunistic and may appear/disappear.
            }
        }
    }

    Ok(())
}

/// Open a file for writing with secure permissions (0600 on Unix).
///
/// Returns an `OpenOptions` configured for secure *write-only* file creation.
#[cfg(unix)]
#[must_use]
pub fn secure_open_options() -> std::fs::OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true).mode(0o600);
    opts
}

#[cfg(not(unix))]
#[must_use]
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
    fn test_create_dir_all_secure_fails_if_intermediate_is_file() {
        let tmp = tempdir().unwrap();
        let intermediate = tmp.path().join("a");
        std::fs::write(&intermediate, b"not a dir").unwrap();

        let nested = tmp.path().join("a").join("b").join("c");
        let err = create_dir_all_secure(&nested).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotADirectory);
        assert!(
            err.to_string().contains("not a directory"),
            "error should mention non-directory path: {err}"
        );
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
    fn test_write_file_secure_hardens_existing_file_permissions() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("existing_file.txt");

        std::fs::write(&file, b"existing").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

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
    fn test_set_sqlite_file_permissions_sets_0600_on_sidecars() {
        let tmp = tempdir().unwrap();
        let db = tmp.path().join("wallet.sqlite");
        let wal = tmp.path().join("wallet.sqlite-wal");
        let shm = tmp.path().join("wallet.sqlite-shm");
        let journal = tmp.path().join("wallet.sqlite-journal");

        std::fs::write(&db, b"db").unwrap();
        std::fs::write(&wal, b"wal").unwrap();
        std::fs::write(&shm, b"shm").unwrap();
        std::fs::write(&journal, b"journal").unwrap();

        set_sqlite_file_permissions(&db).unwrap();

        for file in [&db, &wal, &shm, &journal] {
            let meta = std::fs::metadata(file).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "file {:?} should have mode 0600", file);
        }
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
