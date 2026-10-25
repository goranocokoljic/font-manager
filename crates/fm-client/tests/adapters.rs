//! `fm-cli` and `fm-mcp` reach the app through `fm-client` only: each
//! depends on it and on no HTTP crate of its own (E25.4 criterion 4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

/// The crate names in the normal `[dependencies]` tables of `manifest`
/// (the plain table and any `[target.*.dependencies]`).
fn dependencies(manifest: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(manifest);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let mut names = Vec::new();
    let mut in_deps = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_deps = line == "[dependencies]"
                || (line.starts_with("[target.") && line.ends_with(".dependencies]"));
            continue;
        }
        if in_deps
            && let Some((name, _)) = line.split_once('=')
            && !line.starts_with('#')
        {
            names.push(name.trim().split('.').next().unwrap_or("").to_owned());
        }
    }
    names
}

#[test]
fn the_cli_and_the_mcp_server_use_fm_client_and_no_http_crate() {
    for manifest in ["fm-cli/Cargo.toml", "fm-mcp/Cargo.toml"] {
        let deps = dependencies(manifest);
        assert!(
            deps.iter().any(|d| d == "fm-client"),
            "{manifest}: {deps:?}"
        );
        for http in ["hyper", "hyper-util", "reqwest", "fm-endpoint"] {
            assert!(
                !deps.iter().any(|d| d == http),
                "{manifest} depends on {http} directly: {deps:?}"
            );
        }
    }
}
