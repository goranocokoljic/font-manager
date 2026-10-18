//! `fm-bench` — one measurement per run, for `scripts/bench.sh`.
//!
//! ```text
//! fm-bench start --home DIR                    # cold start; prints one JSON line, exits
//! fm-bench idle --home DIR                     # start, warm the searches, print one JSON line, wait for stdin to close
//! fm-bench scan --library DIR --home DIR       # first scan into a fresh index
//! fm-bench incremental --library DIR --home DIR
//! fm-bench search --home DIR [--rounds N]
//! fm-bench preview-cache --home DIR [--rounds N]
//! ```
//!
//! Every command prints one JSON line on stdout; errors go to stderr with
//! exit code 1, usage errors with exit code 2.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use fm_bench::{BenchResult, first_scan, incremental, preview_cache, search, start};
use serde::Serialize;

const USAGE: &str = "usage: fm-bench <start|idle|scan|incremental|search|preview-cache> --home DIR [--library DIR] [--rounds N]";

#[derive(Debug, Default)]
struct Args {
    command: String,
    home: Option<PathBuf>,
    library: Option<PathBuf>,
    rounds: Option<usize>,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = argv.iter();
    args.command = it.next().cloned().ok_or("missing command")?;
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--home" => args.home = Some(PathBuf::from(value)),
            "--library" => args.library = Some(PathBuf::from(value)),
            "--rounds" => {
                let n = value
                    .parse()
                    .ok()
                    .filter(|&n: &usize| n > 0)
                    .ok_or("--rounds needs a positive integer")?;
                args.rounds = Some(n);
            }
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }
    Ok(args)
}

fn print(value: &impl Serialize) -> BenchResult<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn run(args: Args) -> BenchResult<()> {
    let home = args.home.ok_or("--home is required")?;
    let library = || args.library.clone().ok_or("--library is required");
    match args.command.as_str() {
        "start" => {
            let (started, _log) = start(&home)?;
            print(&started)
        }
        "idle" => {
            let (started, _log) = start(&home)?;
            // What a user does in the first minute: a few searches.
            search(&home, 1)?;
            print(&started)?;
            let mut sink = Vec::new();
            std::io::stdin().read_to_end(&mut sink)?;
            Ok(())
        }
        "scan" => print(&first_scan(&library()?, &home)?),
        "incremental" => print(&incremental(&library()?, &home)?),
        "search" => print(&search(&home, args.rounds.unwrap_or(20))?),
        "preview-cache" => print(&preview_cache(&home, args.rounds.unwrap_or(20))?),
        other => Err(format!("unknown command: {other}\n{USAGE}").into()),
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("fm-bench: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fm-bench: {e}");
            ExitCode::FAILURE
        }
    }
}
