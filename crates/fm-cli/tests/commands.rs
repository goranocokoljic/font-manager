//! Every command on `MockApi`: what it prints with and without `--json`,
//! and what it exits with, for every `ErrorCode`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use clap::Parser;
use fm_api::mock::MockApi;
use fm_api::search::{Filters, SearchRequest, SimilarRequest, Supports};
use fm_api::{API_VERSION, ApiError, ErrorCode, LibraryService, SearchService};
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

/// Runs `fm <args>` on `api`: exit, stdout, stderr.
fn fm_on(api: &MockApi, args: &[&str]) -> (Exit, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("fm").chain(args.iter().copied()))
        .expect("valid command line");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = fm_cli::run(api, &cli, &mut out, &mut err);
    let text = |b: Vec<u8>| String::from_utf8(b).expect("UTF-8");
    (exit, text(out), text(err))
}

#[test]
fn scan_without_a_folder_scans_every_source_with_progress_on_stderr() {
    let (exit, out, err) = fm(&["scan"]);
    assert_eq!(exit, Exit::Success, "{err}");
    assert!(
        err.contains("Scanning /Users/demo/Fonts: 7 of 7 files"),
        "{err}"
    );
    assert!(out.contains("FOLDER") && out.contains("PROBLEMS"), "{out}");
    assert!(out.contains("/Users/demo/Fonts"), "{out}");

    let (exit, out, err) = fm(&["--json", "scan"]);
    assert_eq!(
        (exit, err.as_str()),
        (Exit::Success, ""),
        "no progress with --json"
    );
    let v = json(&out);
    let scans = v.as_array().expect("one scan per source");
    assert_eq!(scans.len(), 2);
    assert_eq!(scans[0]["source"]["path"], "/Users/demo/Fonts");
    for key in ["added", "changed", "removed", "problems"] {
        assert!(scans[0][key].is_u64(), "{key}: {v}");
    }
}

#[test]
fn scan_adds_a_folder_once_and_scans_it_again_after() {
    let api = MockApi::new();
    let folder = std::env::temp_dir().join("fm-cli-scan-folder");
    let path = folder.display().to_string();
    let (exit, out, err) = fm_on(&api, &["--json", "scan", &path]);
    assert_eq!(exit, Exit::Success, "{err}");
    let first = json(&out);
    assert_eq!(first[0]["source"]["path"], path.as_str());
    let sources = LibraryService::list_sources(&api).expect("sources");
    assert_eq!(sources.len(), 3, "added as a source");

    // Already a source: scanned again, not refused.
    let (exit, out, err) = fm_on(&api, &["--json", "scan", &path]);
    assert_eq!(exit, Exit::Success, "{err}");
    assert_eq!(json(&out)[0]["source"], first[0]["source"]);
    assert_eq!(
        LibraryService::list_sources(&api).expect("sources").len(),
        3
    );
}

#[test]
fn list_prints_the_families_or_the_faces_of_one() {
    let (exit, out, _) = fm(&["--json", "list"]);
    assert_eq!(exit, Exit::Success);
    let v = json(&out);
    let names: Vec<&str> = v
        .as_array()
        .expect("families")
        .iter()
        .map(|f| f["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        [
            "Inter",
            "JetBrains Mono",
            "Legacy Grotesk",
            "Source Serif 4"
        ]
    );

    let (exit, out, _) = fm(&["list"]);
    assert_eq!(exit, Exit::Success);
    assert!(
        out.contains("Source Serif 4") && out.ends_with("4 families\n"),
        "{out}"
    );

    let (exit, out, _) = fm(&["--json", "list", "--family", "Source Serif 4"]);
    assert_eq!(exit, Exit::Success);
    let v = json(&out);
    let faces: Vec<&str> = v
        .as_array()
        .expect("faces")
        .iter()
        .map(|f| f["postscriptName"].as_str().expect("name"))
        .collect();
    assert_eq!(
        faces,
        [
            "SourceSerif4-Regular",
            "SourceSerif4-It",
            "SourceSerif4-Bold"
        ]
    );

    let (exit, out, err) = fm(&["list", "--family", "No Such Family"]);
    assert_eq!((exit, out.as_str()), (Exit::NotFound, ""));
    assert!(err.starts_with("fm: "), "{err}");
}

#[test]
fn search_flags_become_the_request_filters() {
    let cli = Cli::try_parse_from([
        "fm",
        "search",
        "inter",
        "--supports",
        "sr-Cyrl,sr-Latn",
        "--feature",
        "smcp",
        "--feature",
        "ss01,onum",
        "--variable",
    ])
    .expect("valid");
    let fm_cli::Command::Search {
        supports,
        feature,
        variable,
        ..
    } = cli.command
    else {
        panic!("a search");
    };
    let flags = fm_cli::SearchFlags {
        supports,
        features: feature,
        variable,
    };
    assert_eq!(
        flags.filters(),
        Filters {
            supports: Some(Supports::Languages {
                codes: vec!["sr-Cyrl".into(), "sr-Latn".into()],
            }),
            features: vec!["smcp".into(), "ss01".into(), "onum".into()],
            variable: Some(true),
            ..Filters::default()
        }
    );
    assert_eq!(fm_cli::SearchFlags::default().filters(), Filters::default());
}

/// The family hits `fm --json search` printed.
fn hits(out: &str) -> Vec<fm_api::search::FamilyHit> {
    serde_json::from_value(json(out)).expect("family hits")
}

/// A direct `search` over everything `filters` keep.
fn direct_search(api: &MockApi, filters: Filters) -> Vec<fm_api::search::FamilyHit> {
    SearchService::search(
        api,
        SearchRequest {
            filters,
            ..SearchRequest::default()
        },
    )
    .expect("search")
    .families
}

#[test]
fn search_prints_the_hits_of_a_direct_search_with_the_same_filters() {
    let api = MockApi::new();
    let direct = direct_search(
        &api,
        Filters {
            supports: Some(Supports::Languages {
                codes: vec!["sr-Cyrl".into(), "sr-Latn".into()],
            }),
            features: vec!["smcp".into()],
            variable: Some(true),
            ..Filters::default()
        },
    );
    assert!(!direct.is_empty(), "the filters keep something");
    let (exit, out, _) = fm_on(
        &api,
        &[
            "--json",
            "search",
            "",
            "--supports",
            "sr-Cyrl,sr-Latn",
            "--feature",
            "smcp",
            "--variable",
        ],
    );
    assert_eq!(exit, Exit::Success);
    assert_eq!(hits(&out), direct);

    // The characters a family misses are printed.
    let direct = direct_search(
        &api,
        Filters {
            supports: Some(Supports::Languages {
                codes: vec!["sr-Cyrl".into()],
            }),
            ..Filters::default()
        },
    );
    let (exit, out, _) = fm_on(&api, &["--json", "search", "--supports", "sr-Cyrl"]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(hits(&out), direct);
    let (_, text, _) = fm_on(&api, &["search", "--supports", "sr-Cyrl"]);
    assert!(text.contains("MISSING"), "{text}");
    for hit in &direct {
        if let Some(missing) = hit.missing.as_ref().filter(|m| !m.chars.is_empty()) {
            assert!(text.contains(&missing.chars.join(" ")), "{text}");
        }
    }

    let (_, text, _) = fm(&["search", "mono"]);
    assert!(
        text.contains("JetBrains Mono") && !text.contains("MISSING"),
        "{text}"
    );
}

#[test]
fn info_prints_the_face_detail() {
    let api = MockApi::new();
    let detail = LibraryService::get_face(&api, fm_api::FaceId(1)).expect("face");
    let (exit, out, _) = fm_on(&api, &["--json", "info", "Inter-Regular"]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(json(&out), serde_json::to_value(&detail).expect("JSON"));

    let (exit, out, _) = fm(&["info", "1"]);
    assert_eq!(exit, Exit::Success);
    for line in [
        "PostScript name   Inter-Regular",
        "Family            Inter",
        "Style             Regular",
        "Weight            400",
        "Slant             upright",
        "Variable          yes",
        "Axes              wght 100\u{2013}900 (default 400)",
        "Features          kern liga smcp onum ss01",
        "Embedding         installable",
    ] {
        assert!(out.contains(line), "{line:?} in {out}");
    }
}

#[test]
fn coverage_exits_0_when_covered_and_5_when_a_character_is_missing() {
    let (exit, out, _) = fm(&["coverage", "SourceSerif4-Regular", "--text", "abc"]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(out, "Every character is covered.\n");

    let (exit, out, err) = fm(&[
        "coverage",
        "SourceSerif4-Regular",
        "--text",
        "ab\u{436}\u{437}",
    ]);
    assert_eq!((exit, err.as_str()), (Exit::Problems, ""));
    assert_eq!(out, "2 characters are missing: \u{436} \u{437}\n");

    let (exit, out, _) = fm(&["--json", "coverage", "3", "--text", "ab\u{436}"]);
    assert_eq!(exit.code(), 5);
    let v = json(&out);
    assert_eq!(v["complete"], false);
    assert_eq!(v["missing"], serde_json::json!(["\u{436}"]));

    let (exit, out, _) = fm(&[
        "coverage",
        "JetBrainsMono-Regular",
        "--supports",
        "sr-Cyrl,en",
    ]);
    assert_eq!(exit, Exit::Success, "{out}");
    assert!(out.contains("sr-Cyrl: ") && out.contains("en: "), "{out}");
    let (exit, _, _) = fm(&["coverage", "SourceSerif4-Regular", "--supports", "sr-Cyrl"]);
    assert_eq!(exit, Exit::Problems);

    let (exit, _, _) = fm(&["coverage", "No Such Font", "--text", "a"]);
    assert_eq!(exit, Exit::NotFound);
    assert!(
        Cli::try_parse_from(["fm", "coverage", "Inter-Regular"]).is_err(),
        "a text or languages is required"
    );
}

#[test]
fn similar_prints_the_similar_faces() {
    let api = MockApi::new();
    let direct = SearchService::find_similar(
        &api,
        SimilarRequest {
            face: fm_api::FaceId(3),
            limit: 2,
        },
    )
    .expect("similar");
    assert_eq!(direct.len(), 2);
    let (exit, out, _) = fm_on(
        &api,
        &["--json", "similar", "SourceSerif4-Regular", "--limit", "2"],
    );
    assert_eq!(exit, Exit::Success);
    let printed: Vec<fm_api::search::SimilarFace> =
        serde_json::from_value(json(&out)).expect("similar faces");
    assert_eq!(printed, direct);

    let (exit, out, _) = fm(&["similar", "SourceSerif4-Regular"]);
    assert_eq!(exit, Exit::Success);
    assert!(out.contains("SCORE") && out.contains("REASONS"), "{out}");
    assert!(out.contains(&direct[0].face.postscript_name), "{out}");
}
