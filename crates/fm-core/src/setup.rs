//! First-run setup (E55.2, ADR 0026): the rule that says which of the
//! setup's questions are still open.
//!
//! The facts come from the library's folders, the catalogs' on/off state, the
//! settings and the answers kept in `config.json`; nothing here reads the
//! network or scans. `fm_api::mock::MockApi` and the TypeScript mock hold a
//! copy of this rule; `fixtures/setup/status-cases.json` is run against all
//! three, so they give the same answer for the same state.

use std::collections::BTreeMap;

use fm_api::library::SourceKind;
use fm_api::setup::{SetupAnswer, SetupStatus, SetupStep};

/// What the setup status is decided from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupFacts {
    /// The library has a watched folder or a library folder (the OS font
    /// folders do not count: the user did not choose them).
    pub has_font_folder: bool,
    /// At least one online catalog is on.
    pub catalog_on: bool,
    /// `Settings::download_folder` is set.
    pub download_folder_set: bool,
    /// This copy updates itself (`UpdateSupport::offered`).
    pub updates_offered: bool,
    /// `Settings::check_updates_automatically` is on.
    pub check_updates_automatically: bool,
    /// The steps the user answered or skipped, from `config.json`.
    pub answers: BTreeMap<SetupStep, SetupAnswer>,
}

/// Whether a source of this kind is a font folder the user chose.
pub fn is_font_folder(kind: SourceKind) -> bool {
    matches!(kind, SourceKind::Watched | SourceKind::LibraryFolder)
}

/// Whether `step` is open for these facts. An answered and a skipped step are
/// alike: neither is asked again. The download folder has no answer of its
/// own; it is open until the folder is chosen.
pub fn is_open(step: SetupStep, facts: &SetupFacts) -> bool {
    let unanswered = !facts.answers.contains_key(&step);
    match step {
        SetupStep::FontFolders => !facts.has_font_folder && unanswered,
        SetupStep::Catalogs => !facts.catalog_on && unanswered,
        SetupStep::DownloadFolder => facts.catalog_on && !facts.download_folder_set,
        SetupStep::Updates => {
            facts.updates_offered && !facts.check_updates_automatically && unanswered
        }
    }
}

/// The steps open for these facts, in the order the setup asks them.
pub fn open_steps(facts: &SetupFacts) -> SetupStatus {
    SetupStatus {
        open: SetupStep::ALL
            .into_iter()
            .filter(|&step| is_open(step, facts))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// One row of `fixtures/setup/status-cases.json`.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        name: String,
        state: State,
        open: Vec<SetupStep>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct State {
        sources: Vec<SourceKind>,
        catalog_on: bool,
        download_folder: bool,
        updates_offered: bool,
        check_automatically: bool,
        answers: BTreeMap<SetupStep, SetupAnswer>,
    }

    fn cases() -> Vec<(String, SetupFacts, Vec<SetupStep>)> {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../../fixtures/setup/status-cases.json"))
                .expect("status cases");
        cases
            .into_iter()
            .map(|c| {
                let facts = SetupFacts {
                    has_font_folder: c.state.sources.iter().any(|&k| is_font_folder(k)),
                    catalog_on: c.state.catalog_on,
                    download_folder_set: c.state.download_folder,
                    updates_offered: c.state.updates_offered,
                    check_updates_automatically: c.state.check_automatically,
                    answers: c.state.answers,
                };
                (c.name, facts, c.open)
            })
            .collect()
    }

    #[test]
    fn every_shared_case_gets_its_open_steps() {
        let cases = cases();
        assert!(cases.len() >= 10, "the table is there");
        for (name, facts, open) in cases {
            assert_eq!(open_steps(&facts).open, open, "{name}");
        }
    }

    #[test]
    fn each_step_closes_on_its_own_fact_and_no_other() {
        let open = SetupFacts {
            updates_offered: true,
            ..SetupFacts::default()
        };
        let all = |f: &SetupFacts| open_steps(f).open;
        assert_eq!(
            all(&open),
            vec![
                SetupStep::FontFolders,
                SetupStep::Catalogs,
                SetupStep::Updates
            ]
        );
        let with = |f: SetupFacts| all(&f);
        assert_eq!(
            with(SetupFacts {
                has_font_folder: true,
                ..open.clone()
            }),
            vec![SetupStep::Catalogs, SetupStep::Updates]
        );
        assert_eq!(
            with(SetupFacts {
                check_updates_automatically: true,
                ..open.clone()
            }),
            vec![SetupStep::FontFolders, SetupStep::Catalogs]
        );
        assert_eq!(
            with(SetupFacts {
                updates_offered: false,
                ..open.clone()
            }),
            vec![SetupStep::FontFolders, SetupStep::Catalogs]
        );
        // A catalog on closes the catalogs step and opens the download folder.
        assert_eq!(
            with(SetupFacts {
                catalog_on: true,
                ..open.clone()
            }),
            vec![
                SetupStep::FontFolders,
                SetupStep::DownloadFolder,
                SetupStep::Updates
            ]
        );
        assert_eq!(
            with(SetupFacts {
                catalog_on: true,
                download_folder_set: true,
                ..open.clone()
            }),
            vec![SetupStep::FontFolders, SetupStep::Updates]
        );
        // An answer to the download folder (a hand edit) does not close it.
        assert!(is_open(
            SetupStep::DownloadFolder,
            &SetupFacts {
                catalog_on: true,
                answers: BTreeMap::from([(SetupStep::DownloadFolder, SetupAnswer::Skipped)]),
                ..open.clone()
            }
        ));
        for step in [
            SetupStep::FontFolders,
            SetupStep::Catalogs,
            SetupStep::Updates,
        ] {
            for answer in [SetupAnswer::Answered, SetupAnswer::Skipped] {
                let facts = SetupFacts {
                    answers: BTreeMap::from([(step, answer)]),
                    ..open.clone()
                };
                assert!(!is_open(step, &facts), "{step:?} {answer:?} closes it");
                assert_eq!(all(&facts).len(), 2, "{step:?} closes only itself");
            }
        }
    }

    #[test]
    fn only_folders_the_user_chose_are_font_folders() {
        assert!(is_font_folder(SourceKind::Watched));
        assert!(is_font_folder(SourceKind::LibraryFolder));
        assert!(!is_font_folder(SourceKind::System));
    }
}
