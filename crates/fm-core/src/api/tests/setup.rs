//! The setup status through the API (E55.2): an install from an earlier
//! version, whose `config.json` has no setup answers, is asked only what its
//! folders, catalogs and settings leave open; an answer is kept in
//! `config.json` for the next start. No request reaches the network.

use fm_api::catalog::{CatalogId, SetCatalogEnabledRequest};
use fm_api::setup::{AnswerSetupStepRequest, SetupAnswer, SetupStep};
use fm_api::update::UpdateSupport;

use super::*;

fn offered() -> UpdateSupport {
    UpdateSupport {
        offered: true,
        reason: None,
        release_host: Some("github.com".into()),
    }
}

/// What an earlier version may have set up.
#[derive(Clone, Copy)]
struct Earlier {
    watched_folder: bool,
    catalog_on: bool,
    download_folder: bool,
    check_automatically: bool,
}

const EVERYTHING: Earlier = Earlier {
    watched_folder: true,
    catalog_on: true,
    download_folder: true,
    check_automatically: true,
};

const NOTHING: Earlier = Earlier {
    watched_folder: false,
    catalog_on: false,
    download_folder: false,
    check_automatically: false,
};

/// A library as an earlier version left it: `config.json` written by hand
/// without `setupAnswers`, the folder added but never scanned, the catalog
/// switched on in its state file; a copy that updates itself.
fn earlier(label: &str, had: Earlier) -> Lib {
    let mut lib = Lib::new(label);
    lib.api = lib.api.with_update_support(offered());
    let mut config = serde_json::json!({ "theme": "dark", "mcpEnabled": false });
    if had.download_folder {
        let folder = lib.dir.path().join("Font Fu Fonts");
        config["downloadFolder"] = folder.display().to_string().into();
    }
    if had.check_automatically {
        config["checkUpdatesAutomatically"] = true.into();
    }
    std::fs::write(lib.paths.config_file(), config.to_string()).expect("config");
    if had.watched_folder {
        lib.api
            .add_source(AddSourceRequest {
                path: lib.root.display().to_string(),
                mode: ImportMode::IndexInPlace,
            })
            .expect("source");
    }
    if had.catalog_on {
        lib.api
            .set_catalog_enabled(SetCatalogEnabledRequest {
                id: CatalogId::GoogleFonts,
                enabled: true,
            })
            .expect("catalog on");
    }
    lib
}

fn open(lib: &Lib) -> Vec<SetupStep> {
    lib.api.setup_status().expect("setup status").open
}

#[test]
fn an_earlier_install_with_everything_set_up_has_no_open_step() {
    let lib = earlier("setup-everything", EVERYTHING);
    assert_eq!(open(&lib), Vec::<SetupStep>::new());
    assert_eq!(lib.fetcher.calls(), 0, "no network request");
    assert!(!holds_scan_files(&lib), "no scan");
}

#[test]
fn an_earlier_install_that_lacks_some_things_has_only_those_open() {
    let cases = [
        (
            "setup-no-folder",
            Earlier {
                watched_folder: false,
                ..EVERYTHING
            },
            vec![SetupStep::FontFolders],
        ),
        (
            "setup-no-catalog",
            Earlier {
                catalog_on: false,
                ..EVERYTHING
            },
            vec![SetupStep::Catalogs],
        ),
        (
            "setup-no-download-folder",
            Earlier {
                download_folder: false,
                ..EVERYTHING
            },
            vec![SetupStep::DownloadFolder],
        ),
        (
            "setup-no-check",
            Earlier {
                check_automatically: false,
                ..EVERYTHING
            },
            vec![SetupStep::Updates],
        ),
        (
            "setup-folder-only",
            Earlier {
                watched_folder: true,
                ..NOTHING
            },
            vec![SetupStep::Catalogs, SetupStep::Updates],
        ),
        (
            "setup-nothing",
            NOTHING,
            vec![
                SetupStep::FontFolders,
                SetupStep::Catalogs,
                SetupStep::Updates,
            ],
        ),
    ];
    for (label, had, expected) in cases {
        let lib = earlier(label, had);
        assert_eq!(open(&lib), expected, "{label}");
        assert_eq!(lib.fetcher.calls(), 0, "{label}: no network request");
    }
}

#[test]
fn a_library_folder_counts_as_a_font_folder_and_the_os_folders_do_not() {
    let lib = earlier(
        "setup-library-folder",
        Earlier {
            watched_folder: false,
            ..EVERYTHING
        },
    );
    let system = lib.dir.path().join("System Fonts");
    lib.api
        .lock()
        .add_source(&system.display().to_string(), SourceKind::System)
        .expect("system source");
    assert_eq!(
        open(&lib),
        vec![SetupStep::FontFolders],
        "the OS folders do not count"
    );
    lib.api
        .lock()
        .add_source(
            &lib.paths.library.display().to_string(),
            SourceKind::LibraryFolder,
        )
        .expect("library folder");
    assert_eq!(open(&lib), Vec::<SetupStep>::new());
}

#[test]
fn where_the_copy_does_not_update_itself_the_updates_step_is_never_open() {
    let mut lib = Lib::new("setup-no-shell");
    // Without a shell (the CLI's direct link) nothing says the copy updates itself.
    assert_eq!(
        open(&lib),
        vec![SetupStep::FontFolders, SetupStep::Catalogs]
    );
    lib.api = lib.api.with_update_support(UpdateSupport {
        offered: false,
        reason: Some("Flatpak".into()),
        release_host: Some("github.com".into()),
    });
    assert_eq!(
        open(&lib),
        vec![SetupStep::FontFolders, SetupStep::Catalogs]
    );
    lib.api = lib.api.with_update_support(offered());
    assert_eq!(
        open(&lib),
        vec![
            SetupStep::FontFolders,
            SetupStep::Catalogs,
            SetupStep::Updates
        ]
    );
}

#[test]
fn a_skipped_step_is_kept_in_config_json_and_not_asked_at_the_next_start() {
    let lib = earlier("setup-skip", NOTHING);
    let answer = |step, answer| {
        lib.api
            .answer_setup_step(AnswerSetupStepRequest { step, answer })
            .expect("answer")
            .open
    };
    assert_eq!(
        answer(SetupStep::FontFolders, SetupAnswer::Skipped),
        vec![SetupStep::Catalogs, SetupStep::Updates]
    );
    assert_eq!(
        answer(SetupStep::Updates, SetupAnswer::Answered),
        vec![SetupStep::Catalogs]
    );

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(lib.paths.config_file()).expect("file"))
            .expect("json");
    assert_eq!(
        json["setupAnswers"],
        serde_json::json!({ "fontFolders": "skipped", "updates": "answered" })
    );
    assert_eq!(json["theme"], "dark", "the earlier keys are kept");

    // The next start: a new API over the same folders.
    let index = Index::open(&lib.paths.data.join(INDEX_FILE))
        .expect("index")
        .index;
    let next = CoreApi::new(
        index,
        Arc::new(MockActivator::new()) as Arc<dyn FontActivator>,
        SettingsStore::new(&lib.paths),
        None,
    )
    .with_update_support(offered());
    assert_eq!(
        next.setup_status().expect("status").open,
        vec![SetupStep::Catalogs]
    );
}

#[test]
fn the_download_folder_is_answered_by_choosing_it() {
    let lib = earlier(
        "setup-download-folder",
        Earlier {
            download_folder: false,
            ..EVERYTHING
        },
    );
    let err = lib
        .api
        .answer_setup_step(AnswerSetupStepRequest {
            step: SetupStep::DownloadFolder,
            answer: SetupAnswer::Skipped,
        })
        .expect_err("refused");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert_eq!(open(&lib), vec![SetupStep::DownloadFolder]);
    lib.api
        .update_settings(SettingsUpdate {
            download_folder: Some(lib.dir.path().join("Fonts here").display().to_string()),
            ..SettingsUpdate::default()
        })
        .expect("chosen");
    assert_eq!(open(&lib), Vec::<SetupStep>::new());
}
