//! Trains the pairing model (E33.2) on `fixtures/fonts/` and the curated
//! pairings of `data/pairings/pairings.json`, and writes
//! `data/pairings/model.json`.
//!
//! ```text
//! cargo run -p fm-core --example train_pairings
//! ```
//!
//! The run is deterministic (a fixed seed): the same corpus and pairings
//! give the same file. A test fails when the committed model is stale.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let model = match fm_core::pairing::model::train_on_folder(&root.join("fixtures/fonts")) {
        Ok(model) => model,
        Err(e) => {
            eprintln!("training failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = root.join("data/pairings/model.json");
    if let Err(e) = std::fs::write(&out, model.to_json()) {
        eprintln!("could not write {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    println!("wrote {}", out.display());
    ExitCode::SUCCESS
}
