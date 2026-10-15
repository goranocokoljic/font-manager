//! `fm` — the Font Manager command-line tool.
//!
//! A thin adapter over `fm-api`. Commands arrive with their stories (E21).

fn main() {
    println!(
        "fm {} (api {})",
        env!("CARGO_PKG_VERSION"),
        fm_api::API_VERSION
    );
}
