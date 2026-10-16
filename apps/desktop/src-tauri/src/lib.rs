//! The Font Manager desktop shell: Tauri commands are thin adapters over
//! `fm-api`. The window, router, stores and the generated API client arrive
//! with E1.5.

/// Starts the desktop app.
pub fn run() {
    // App folders, settings and the rotating log file. The guard lives until
    // `run` returns so buffered log lines are flushed on exit. A failure here
    // is reported but does not stop the app: it runs on default settings.
    let startup = match fm_core::startup::init() {
        Ok(startup) => Some(startup),
        Err(e) => {
            eprintln!("font manager: {e}");
            None
        }
    };
    if let Err(e) = tauri::Builder::default().run(tauri::generate_context!()) {
        tracing::error!(error = %e, "font manager failed to start");
        eprintln!("font manager failed to start: {e}");
        drop(startup); // flush the log before exit skips destructors
        std::process::exit(1);
    }
}
