//! `fm` — the Font Manager command-line tool.
//!
//! A thin adapter over `fm-api`. When the app is open, the CLI talks to it
//! through `fm-client` (the local API, ADR 0015) and holds it as
//! `Arc<dyn Api>`; commands arrive with their stories (E21).

use std::sync::Arc;

use fm_api::Api;

fn main() {
    println!(
        "fm {} (api {})",
        env!("CARGO_PKG_VERSION"),
        fm_api::API_VERSION
    );
    let app: Option<Arc<dyn Api>> = fm_client::running_app();
    match app.map(|api| api.app_info()) {
        Some(Ok(info)) => println!("Font Manager {} is running.", info.version),
        Some(Err(e)) => println!("Font Manager is running but did not answer: {e}"),
        None => println!("Font Manager is not running."),
    }
}
