use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_os = "macos")]
pub fn resolve_executable(bundle: &Path) -> Option<PathBuf> {
    let macos_dir = bundle.join("Contents/MacOS");
    let plist_path = bundle.join("Contents/Info.plist");

    if plist_path.is_file()
        && let Ok(plist::Value::Dictionary(dict)) = plist::Value::from_file(&plist_path)
        && let Some(plist::Value::String(executable_name)) = dict.get("CFBundleExecutable")
    {
        let executable = macos_dir.join(executable_name);
        if is_executable_file(&executable) {
            return Some(executable);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(&macos_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_executable_file(&path) {
                candidates.push(path);
            }
        }
    }

    (candidates.len() == 1).then(|| candidates.remove(0))
}

#[cfg(target_os = "macos")]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn resolve_executable(_bundle: &Path) -> Option<PathBuf> {
    None
}
