//! The Font Manager desktop shell: Tauri commands are thin adapters over
//! `fm-api`. The window, router, stores and the generated API client arrive
//! with E1.5.

/// Starts the desktop app.
pub fn run() {
    if let Err(e) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("font manager failed to start: {e}");
        std::process::exit(1);
    }
}
