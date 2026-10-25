//! `fm` — the Font Manager command-line tool.
//!
//! Links `fm-core` directly: [`CoreApi`] over the index in the app folders
//! (or under `FM_HOME`, a scratch home with `data`, `config`, `cache` and
//! `logs`), held as `Arc<dyn Api>`. Talking to the running app through
//! `fm-client` instead comes with E26.6.

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use clap::error::ErrorKind;
use fm_api::{Api, ApiError, ApiResult, ErrorCode};
use fm_cli::{Cli, Link, Output};
use fm_core::{CoreApi, startup};
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
    let api = match open() {
        Ok(api) => api,
        Err(e) => {
            let failed: Result<(), ApiError> = Err(e);
            return Output::new(cli.json, &mut out, &mut err)
                .print(failed, |(), _| Ok(()))
                .into();
        }
    };
    // Always the index directly until E26.6 talks to the running app.
    fm_cli::run(api.as_ref(), Link::Direct, &cli, &mut out, &mut err).into()
}

/// A usage error clap found, printed as `fm` prints errors.
fn usage(json: bool, text: &str) -> fm_cli::Exit {
    let (mut out, mut err) = (io::stdout().lock(), io::stderr().lock());
    Output::new(json, &mut out, &mut err).usage(text)
}

/// The API over the index in the app folders.
fn open() -> ApiResult<Arc<dyn Api>> {
    let paths = app_paths()?;
    startup::prepare(&paths)?;
    let index = startup::open_index(&paths)?.index;
    let activator = fm_platform::activator::system(&paths).map_err(|e| {
        ApiError::new(ErrorCode::Io, "Font activation is not available.").with_detail(e.to_string())
    })?;
    Ok(Arc::new(CoreApi::open(&paths, index, activator)))
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
