//! Every command on `MockApi`: what it prints with and without `--json`,
//! and what it exits with, for every `ErrorCode`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use clap::Parser;
use fm_api::activation::ActivationMode;
use fm_api::mock::MockApi;
use fm_api::organization::{CollectionFacesRequest, CreateCollectionRequest, SetEntry, SetExport};
use fm_api::search::{Filters, SearchRequest, SimilarRequest, Supports};
use fm_api::{
    API_VERSION, ActivationService, ApiError, ErrorCode, FaceId, LibraryService,
    OrganizationService, SearchService, SetId,
};
use fm_cli::{Cli, Exit, Link, Output};
use serde_json::Value;

/// Runs `fm <args>` on a fresh mock: exit, stdout, stderr.
fn fm(args: &[&str]) -> (Exit, String, String) {
    fm_on(&MockApi::new(), args)
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

/// Runs `fm <args>` on `api`, as through the app's endpoint: exit, stdout,
/// stderr.
fn fm_on(api: &MockApi, args: &[&str]) -> (Exit, String, String) {
    fm_linked(api, Link::Endpoint, args)
}

/// Runs `fm <args>` on `api`, reached over `link`: exit, stdout, stderr.
fn fm_linked(api: &MockApi, link: Link, args: &[&str]) -> (Exit, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("fm").chain(args.iter().copied()))
        .expect("valid command line");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = fm_cli::run(api, link, &cli, &mut out, &mut err);
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

    // The characters a family misses are printed: Legacy Grotesk (Latin
    // only) is a near miss of Serbian Latin.
    let direct = direct_search(
        &api,
        Filters {
            supports: Some(Supports::Languages {
                codes: vec!["sr-Latn".into()],
            }),
            ..Filters::default()
        },
    );
    let (exit, out, _) = fm_on(&api, &["--json", "search", "--supports", "sr-Latn"]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(hits(&out), direct);
    let legacy = direct
        .iter()
        .find(|h| h.family.name == "Legacy Grotesk")
        .and_then(|h| h.missing.clone())
        .expect("a near miss");
    assert_eq!(legacy.chars.len(), 5, "{legacy:?}");
    let (_, text, _) = fm_on(&api, &["search", "--supports", "sr-Latn"]);
    let row = text
        .lines()
        .find(|l| l.contains("Legacy Grotesk"))
        .unwrap_or_else(|| panic!("a Legacy Grotesk row: {text}"));
    assert!(row.trim_end().ends_with(&legacy.chars.join(" ")), "{row}");
    assert!(text.contains("MISSING"), "{text}");

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

/// The faces the app has active, with their mode.
fn active(api: &MockApi) -> Vec<(u32, ActivationMode)> {
    api.list_active()
        .unwrap()
        .into_iter()
        .map(|a| (a.face.0, a.mode))
        .collect()
}

/// The `status` of each outcome of a `--json` activation, by face id.
fn statuses(out: &str) -> Vec<(u64, String)> {
    json(out)
        .as_array()
        .expect("an array of outcomes")
        .iter()
        .map(|o| {
            (
                o["face"].as_u64().expect("face"),
                o["outcome"]["status"].as_str().expect("status").to_owned(),
            )
        })
        .collect()
}

/// The line of `out` that starts with face `id`.
fn line_of(out: &str, id: u32) -> &str {
    out.lines()
        .map(str::trim)
        .find(|l| l.starts_with(&format!("{id} ")))
        .unwrap_or_else(|| panic!("no line for face {id}: {out}"))
}

#[test]
fn activate_a_family_name_activates_every_style_one_line_per_face() {
    let api = MockApi::new();
    let (exit, out, err) = fm_on(&api, &["activate", "Inter"]);
    assert_eq!((exit, err.as_str()), (Exit::Success, ""));
    assert_eq!(
        out.lines().count(),
        3,
        "a header and one line per face: {out}"
    );
    let regular = line_of(&out, 1);
    assert!(
        regular.contains("Inter Regular") && regular.ends_with("done"),
        "{out}"
    );
    let italic = line_of(&out, 2);
    assert!(
        italic.contains("Inter Italic") && italic.ends_with("done"),
        "{out}"
    );
    assert_eq!(
        active(&api),
        [(1, ActivationMode::Session), (2, ActivationMode::Session)]
    );

    // Already active: unchanged is a success too.
    let (exit, out, _) = fm_on(&api, &["--json", "activate", "Inter"]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(
        statuses(&out),
        [(1, "unchanged".to_owned()), (2, "unchanged".to_owned())]
    );
}

#[test]
fn activate_permanent_asks_for_a_permanent_activation_of_one_face() {
    let api = MockApi::new();
    let (exit, out, _) = fm_on(&api, &["--json", "activate", "Inter-Italic", "--permanent"]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(statuses(&out), [(2, "done".to_owned())]);
    assert_eq!(active(&api), [(2, ActivationMode::Permanent)]);
}

#[test]
fn activate_a_set_exits_4_when_a_face_clashes_and_activates_the_rest() {
    let api = MockApi::new();
    // The mock set "Client brand": Inter Regular, and JetBrains Mono, which
    // clashes with an installed font.
    let (exit, out, err) = fm_on(&api, &["activate", "--set", "client BRAND"]);
    assert_eq!((exit, err.as_str()), (Exit::Refused, ""));
    assert!(line_of(&out, 1).ends_with("done"), "{out}");
    let clash = line_of(&out, 6);
    assert!(
        clash.contains("JetBrains Mono Regular") && clash.contains("conflict: an installed font"),
        "{out}"
    );
    assert_eq!(active(&api), [(1, ActivationMode::Session)]);

    let (exit, out, _) = fm_on(&MockApi::new(), &["--json", "activate", "--set", "1"]);
    assert_eq!(exit, Exit::Refused);
    assert_eq!(
        statuses(&out),
        [(1, "done".to_owned()), (6, "conflict".to_owned())]
    );
}

#[test]
fn activate_a_font_that_cannot_be_activated_exits_4_with_the_reason() {
    let api = MockApi::new();
    let (exit, out, _) = fm_on(&api, &["activate", "LegacyGrotesk"]);
    assert_eq!(exit, Exit::Refused);
    assert!(
        line_of(&out, 7).contains("skipped: Type 1 fonts cannot be activated"),
        "{out}"
    );
    assert!(active(&api).is_empty());
}

#[test]
fn activate_and_deactivate_a_collection_by_name_or_id() {
    let api = MockApi::new();
    let picks = api
        .create_collection(CreateCollectionRequest {
            name: "Serif picks".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .unwrap();
    api.add_to_collection(CollectionFacesRequest {
        collection: picks.id,
        faces: vec![FaceId(3), FaceId(5)],
        families: Vec::new(),
    })
    .unwrap();

    let (exit, out, _) = fm_on(
        &api,
        &["--json", "activate", "--collection", " serif picks "],
    );
    assert_eq!(exit, Exit::Success);
    assert_eq!(
        statuses(&out),
        [(3, "done".to_owned()), (5, "done".to_owned())]
    );

    let id = picks.id.0.to_string();
    let (exit, out, _) = fm_on(&api, &["--json", "deactivate", "--collection", &id]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(
        statuses(&out),
        [(3, "done".to_owned()), (5, "done".to_owned())]
    );
    assert!(active(&api).is_empty());
}

#[test]
fn a_target_that_names_nothing_or_several_things_is_refused() {
    let api = MockApi::new();
    let (exit, out, err) = fm_on(&api, &["activate", "--collection", "Nope"]);
    assert_eq!((exit, out.as_str()), (Exit::NotFound, ""));
    assert!(
        err.contains("No collection in the library is called \"Nope\"."),
        "{err}"
    );

    let (exit, _, err) = fm_on(&api, &["--json", "deactivate", "--set", "Nope"]);
    assert_eq!(exit, Exit::NotFound);
    assert_eq!(json(&err)["code"], "notFound");

    let (exit, _, _) = fm_on(&api, &["activate", "No Such Font"]);
    assert_eq!(exit, Exit::NotFound);

    // Two collections of one name, under different parents.
    let new = |name: &str, parent| {
        api.create_collection(CreateCollectionRequest {
            name: name.into(),
            parent,
            rules: None,
            rule_group: None,
        })
        .unwrap()
    };
    let (a, b) = (new("Work", None), new("Play", None));
    let (x, y) = (new("Drafts", Some(a.id)), new("Drafts", Some(b.id)));
    let (exit, out, err) = fm_on(&api, &["--json", "activate", "--collection", "drafts"]);
    assert_eq!((exit, out.as_str()), (Exit::Usage, ""));
    let e = json(&err);
    assert_eq!(e["code"], "invalidRequest");
    let detail = e["detail"].as_str().expect("detail");
    assert!(detail.contains(&format!("{}\tDrafts", x.id)), "{detail}");
    assert!(detail.contains(&format!("{}\tDrafts", y.id)), "{detail}");
    assert!(active(&api).is_empty());

    // A font, a collection or a set: one at a time.
    assert!(Cli::try_parse_from(["fm", "activate", "x", "--collection", "--set"]).is_err());
}

#[test]
fn deactivate_reports_faces_the_app_did_not_activate_as_skipped() {
    let api = MockApi::new();
    fm_on(&api, &["activate", "Inter-Regular"]);
    let (exit, out, err) = fm_on(&api, &["deactivate", "Inter"]);
    assert_eq!((exit, err.as_str()), (Exit::Refused, ""));
    assert!(line_of(&out, 1).ends_with("done"), "{out}");
    assert!(
        line_of(&out, 2).contains("skipped: The app did not activate this font."),
        "{out}"
    );
    assert!(active(&api).is_empty());
}

#[test]
fn deactivate_never_deregisters_a_font_activated_outside_the_app() {
    let api = MockApi::new();
    fm_on(&api, &["activate", "Inter-Regular"]);
    // JetBrains Mono is installed on the system, not activated by the app.
    let (exit, out, _) = fm_on(&api, &["--json", "deactivate", "JetBrainsMono-Regular"]);
    assert_eq!(exit, Exit::Refused);
    assert_eq!(statuses(&out), [(6, "skipped".to_owned())]);
    // What the app did activate is left alone too.
    assert_eq!(active(&api), [(1, ActivationMode::Session)]);
}

#[test]
fn direct_link_mode_refuses_to_activate_or_deactivate() {
    let api = MockApi::new();
    let message = "fm: Font Manager must be running to activate fonts; start it and try again\n";
    let (exit, out, err) = fm_linked(&api, Link::Direct, &["activate", "Inter"]);
    assert_eq!(
        (exit, out.as_str(), err.as_str()),
        (Exit::Refused, "", message)
    );
    assert!(active(&api).is_empty(), "nothing was activated");

    fm_on(&api, &["activate", "Inter-Regular"]);
    let (exit, out, err) = fm_linked(&api, Link::Direct, &["--json", "deactivate", "Inter"]);
    assert_eq!((exit, out.as_str()), (Exit::Refused, ""));
    assert_eq!(json(&err)["code"], "unsupported");
    assert_eq!(
        active(&api),
        [(1, ActivationMode::Session)],
        "nothing was deactivated"
    );

    // Other commands still run in direct-link mode.
    let (exit, _, _) = fm_linked(&api, Link::Direct, &["resolve", "Inter-Regular"]);
    assert_eq!(exit, Exit::Success);
}

/// A path under the system temp folder for one test's file.
fn temp_file(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("fm-cli-{label}-{}.json", std::process::id()))
}

#[test]
fn sets_export_prints_the_export_json_or_writes_it_to_a_file() {
    let api = MockApi::new();
    let expected = api.export_set(SetId(1)).unwrap();

    let (exit, out, err) = fm_on(&api, &["sets", "export", "Client brand"]);
    assert_eq!((exit, err.as_str()), (Exit::Success, ""));
    let printed: SetExport = serde_json::from_str(&out).expect("the export JSON");
    assert_eq!(printed, expected);

    let (exit, out, _) = fm_on(&api, &["--json", "sets", "export", "1"]);
    assert_eq!(exit, Exit::Success);
    assert_eq!(json(&out), serde_json::to_value(&expected).unwrap());

    let file = temp_file("export");
    let path = file.display().to_string();
    let (exit, out, _) = fm_on(&api, &["sets", "export", "Client brand", "--out", &path]);
    assert_eq!(exit, Exit::Success);
    assert!(
        out.contains(&format!("Wrote set \"Client brand\" (2 fonts) to {path}")),
        "{out}"
    );
    let written: SetExport =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).expect("JSON file");
    let _ = std::fs::remove_file(&file);
    assert_eq!(written, expected);

    let (exit, _, _) = fm_on(&api, &["sets", "export", "No such set"]);
    assert_eq!(exit, Exit::NotFound);
}

#[test]
fn sets_import_prints_the_fonts_found_and_missing() {
    let api = MockApi::new();
    let mut export = api.export_set(SetId(1)).unwrap();
    export.name = "From a teammate".into();
    export.fonts.push(SetEntry {
        family: "Gone Sans".into(),
        style: "Black".into(),
        hash: "00".repeat(32),
    });
    let file = temp_file("import");
    std::fs::write(&file, serde_json::to_string(&export).unwrap()).unwrap();
    let path = file.display().to_string();

    let (exit, out, err) = fm_on(&api, &["sets", "import", &path]);
    assert_eq!((exit, err.as_str()), (Exit::Success, ""));
    assert!(out.contains("Imported set \"From a teammate\""), "{out}");
    assert!(out.contains("2 fonts found, 1 missing."), "{out}");
    assert!(line_of(&out, 1).contains("Inter Regular"), "{out}");
    assert!(line_of(&out, 6).contains("JetBrains Mono Regular"), "{out}");
    assert!(out.contains("Gone Sans") && out.contains("Black"), "{out}");

    let (exit, out, _) = fm_on(&api, &["--json", "sets", "import", &path]);
    let _ = std::fs::remove_file(&file);
    assert_eq!(exit, Exit::Success);
    let v = json(&out);
    assert_eq!(v["set"]["faces"], serde_json::json!([1, 6]));
    assert_eq!(v["missing"][0]["family"], "Gone Sans");
}

#[test]
fn sets_import_refuses_a_file_that_is_missing_or_not_an_export() {
    let api = MockApi::new();
    let sets = api.list_sets().unwrap().len();
    let missing = temp_file("no-such-export");
    let (exit, _, err) = fm_on(
        &api,
        &["--json", "sets", "import", &missing.display().to_string()],
    );
    assert_eq!(exit, Exit::NotFound);
    assert_eq!(json(&err)["code"], "notFound");

    let file = temp_file("not-an-export");
    std::fs::write(&file, "{\"fonts\": 3}").unwrap();
    let (exit, out, err) = fm_on(&api, &["sets", "import", &file.display().to_string()]);
    let _ = std::fs::remove_file(&file);
    assert_eq!((exit, out.as_str()), (Exit::Usage, ""));
    assert!(
        err.contains("is not a set exported by Font Manager"),
        "{err}"
    );
    assert_eq!(api.list_sets().unwrap().len(), sets, "no set was made");
}
