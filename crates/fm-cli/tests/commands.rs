//! Every command on `MockApi`: what it prints with and without `--json`,
//! and what it exits with, for every `ErrorCode`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use clap::Parser;
use fm_api::mock::MockApi;
use fm_api::{API_VERSION, ApiError, ErrorCode};
use fm_cli::{Cli, Exit, Output};
use serde_json::Value;

/// Runs `fm <args>` on a fresh mock: exit, stdout, stderr.
fn fm(args: &[&str]) -> (Exit, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("fm").chain(args.iter().copied()))
        .expect("valid command line");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = fm_cli::run(&MockApi::new(), &cli, &mut out, &mut err);
    let text = |b: Vec<u8>| String::from_utf8(b).expect("UTF-8");
    (exit, text(out), text(err))
}

fn json(text: &str) -> Value {
    assert_eq!(text.lines().count(), 1, "one JSON value: {text}");
    serde_json::from_str(text).expect("JSON")
}

#[test]
fn version_prints_the_app_and_api_versions() {
    let (exit, out, err) = fm(&["version"]);
    assert_eq!((exit, err.as_str()), (Exit::Success, ""));
    assert!(out.contains(&format!("API {API_VERSION}")), "{out}");
    assert!(out.starts_with("Font Manager "), "{out}");

    let (exit, out, err) = fm(&["version", "--json"]);
    assert_eq!((exit, err.as_str()), (Exit::Success, ""));
    let v = json(&out);
    assert_eq!(v["apiVersion"]["minor"], API_VERSION.minor);
    assert!(v["version"].is_string());
}

#[test]
fn json_is_a_global_flag() {
    let (_, before, _) = fm(&["--json", "version"]);
    let (_, after, _) = fm(&["version", "--json"]);
    assert_eq!(before, after);
}

#[test]
fn a_font_argument_takes_an_id_or_a_name() {
    let (exit, out, _) = fm(&["--json", "resolve", "Inter-Italic"]);
    assert_eq!(exit, Exit::Success);
    let v = json(&out);
    assert_eq!(v["face"], 2);
    assert_eq!(v["postscriptName"], "Inter-Italic");

    let (exit, out, _) = fm(&["resolve", "2"]);
    assert_eq!(exit, Exit::Success);
    assert!(out.contains("Inter-Italic"), "{out}");
    assert!(out.contains("POSTSCRIPT NAME"), "a table: {out}");
}

#[test]
fn an_ambiguous_name_exits_2_and_lists_the_candidates() {
    let (exit, out, err) = fm(&["resolve", "Inter"]);
    assert_eq!((exit, out.as_str()), (Exit::Usage, ""));
    assert!(err.starts_with("fm: \"Inter\" matches 2 fonts."), "{err}");
    assert!(
        err.contains("Inter-Regular") && err.contains("Inter-Italic"),
        "{err}"
    );

    let (exit, out, err) = fm(&["--json", "resolve", "Inter"]);
    assert_eq!((exit, out.as_str()), (Exit::Usage, ""));
    let e = json(&err);
    assert_eq!(e["code"], "invalidRequest");
    let detail = e["detail"].as_str().expect("detail");
    assert!(detail.contains("1\tInter-Regular"), "{detail}");
    assert!(detail.contains("2\tInter-Italic"), "{detail}");
}

#[test]
fn an_unknown_font_exits_3_with_the_message_on_stderr() {
    let (exit, out, err) = fm(&["resolve", "No Such Font"]);
    assert_eq!((exit, out.as_str()), (Exit::NotFound, ""));
    assert_eq!(
        err,
        "fm: No font in the library is called \"No Such Font\".\n"
    );
}

#[test]
fn every_error_code_prints_and_exits_as_documented() {
    let cases = [
        (ErrorCode::NotFound, 3, "notFound"),
        (ErrorCode::InvalidRequest, 2, "invalidRequest"),
        (ErrorCode::Conflict, 4, "conflict"),
        (ErrorCode::PermissionDenied, 4, "permissionDenied"),
        (ErrorCode::Unsupported, 4, "unsupported"),
        (ErrorCode::Busy, 4, "busy"),
        (ErrorCode::Io, 1, "io"),
        (ErrorCode::Cancelled, 1, "cancelled"),
        (ErrorCode::Internal, 1, "internal"),
    ];
    for (code, exit, name) in cases {
        let error = ApiError::new(code, "It went wrong.").with_detail("where");
        let failed: Result<(), ApiError> = Err(error.clone());

        let (mut out, mut err) = (Vec::new(), Vec::new());
        let got = Output::new(false, &mut out, &mut err).print(failed.clone(), |(), _| Ok(()));
        assert_eq!(got.code(), exit, "{code:?}");
        assert!(out.is_empty());
        assert_eq!(String::from_utf8_lossy(&err), "fm: It went wrong.\n");

        let (mut out, mut err) = (Vec::new(), Vec::new());
        let got = Output::new(true, &mut out, &mut err).print(failed, |(), _| Ok(()));
        assert_eq!(got.code(), exit, "{code:?}");
        assert!(out.is_empty());
        let e = json(&String::from_utf8_lossy(&err));
        assert_eq!(e["code"], name);
        let back: ApiError = serde_json::from_value(e).expect("an ApiError");
        assert_eq!(back, error);
    }
}

#[test]
fn a_usage_error_exits_2_and_is_json_with_the_flag() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let got = Output::new(true, &mut out, &mut err).usage("error: unrecognized subcommand\n");
    assert_eq!(got, Exit::Usage);
    assert!(out.is_empty());
    let e = json(&String::from_utf8_lossy(&err));
    assert_eq!(e["code"], "invalidRequest");
    assert_eq!(e["detail"], "error: unrecognized subcommand");
}
