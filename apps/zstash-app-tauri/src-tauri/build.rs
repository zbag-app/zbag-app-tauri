fn main() {
    // Rebuild when frontend assets change
    println!("cargo:rerun-if-changed=../dist");
    tauri_build::build()
}
