fn main() {
    tauri_build::build();

    // tauri-build embeds the Common Controls v6 manifest in the app binary
    // only. Without it a Windows test binary that links Tauri aborts at load
    // with STATUS_ENTRYPOINT_NOT_FOUND, so the integration tests get it too.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let manifest = std::path::Path::new(&dir).join("tests/windows-test.manifest");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
