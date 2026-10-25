//! `fm` — the Font Manager command-line tool.
//!
//! On start it looks for the running app ([`fm_cli::link::reach`]): when
//! the app's local API answers, every command goes through it (endpoint
//! mode); otherwise `fm` links `fm-core` directly on the index in the app
//! folders (direct-link mode). `--direct` forces direct-link. The app
//! folders are the user's, or those under `FM_HOME` (a scratch home with
//! `data`, `config`, `cache` and `logs`).

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;
use fm_api::{ApiError, ApiResult, ErrorCode};
use fm_cli::{Cli, Output};
use fm_platform::AppPaths;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            // Help and version go to stdout and exit 0.
            let _ = e.print();
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            let json = std::env::args_os().any(|a| a == "--json");
            let text = e.render().to_string();
            return usage(json, &text).into();
        }
    };
    let (mut out, mut err) = (io::stdout().lock(), io::stderr().lock());
    let reached = app_paths().and_then(|paths| fm_cli::link::reach(&paths, cli.direct));
    let (api, link) = match reached {
        Ok(reached) => reached,
        Err(e) => {
            let failed: Result<(), ApiError> = Err(e);
            return Output::new(cli.json, &mut out, &mut err)
                .print(failed, |(), _| Ok(()))
                .into();
        }
    };
    fm_cli::run(api.as_ref(), link, &cli, &mut out, &mut err).into()
}

/// A usage error clap found, printed as `fm` prints errors.
fn usage(json: bool, text: &str) -> fm_cli::Exit {
    let (mut out, mut err) = (io::stdout().lock(), io::stderr().lock());
    Output::new(json, &mut out, &mut err).usage(text)
}

/// `FM_HOME` when it is set to an absolute path, else the user's app folders.
fn app_paths() -> ApiResult<AppPaths> {
    if let Some(home) = std::env::var_os("FM_HOME").map(PathBuf::from)
        && home.is_absolute()
    {
        return Ok(AppPaths::under(&home));
    }
    AppPaths::current().map_err(|e| {
        ApiError::new(ErrorCode::Io, "The app folders could not be found.")
            .with_detail(e.to_string())
    })
}
