use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn signing_export_uses_generic_filename() {
    let signing = repo_root().join("apps/zstash-app-tauri/src/pages/Signing.tsx");
    let src = std::fs::read_to_string(signing).expect("read Signing.tsx");
    assert!(
        src.contains("a.download = 'zstash-unsigned.pczt'"),
        "expected generic .pczt filename"
    );
}

#[test]
fn signing_export_has_no_hardware_wallet_branding() {
    let signing = repo_root().join("apps/zstash-app-tauri/src/pages/Signing.tsx");
    let src = std::fs::read_to_string(signing).expect("read Signing.tsx");
    assert!(
        !src.to_lowercase().contains("keystone-"),
        "must not include hardware wallet branding in exported filenames"
    );
}
