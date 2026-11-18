//! First-run setup (E55.2, ADR 0026): which of the setup's questions are
//! still open, and the user's answer to one of them.
//!
//! The rule that decides when a step is open is in `fm-core`
//! (`fm_core::setup::open_steps`); the answers are kept in `config.json`.

use serde::{Deserialize, Serialize};

/// A question of the first-run setup, in the order the setup asks them.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SetupStep {
    /// The folders that hold the user's fonts. Open while the library has no
    /// watched folder and no library folder and the step was never answered
    /// or skipped.
    FontFolders,
    /// Whether to use the online catalogs. Open while no catalog is on and the
    /// step was never answered or skipped.
    Catalogs,
    /// Where catalog installs go. Open while a catalog is on and the download
    /// folder is not set; it closes only when the folder is chosen.
    DownloadFolder,
    /// Check for updates automatically. Open while this copy updates itself,
    /// the automatic check is off and the step was never answered or skipped.
    Updates,
}

impl SetupStep {
    /// Every step, in the order the setup asks them.
    pub const ALL: [Self; 4] = [
        Self::FontFolders,
        Self::Catalogs,
        Self::DownloadFolder,
        Self::Updates,
    ];
}

/// How the user closed a setup step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SetupAnswer {
    /// The user made the step's choice (even "no catalogs" or "no automatic
    /// check").
    Answered,
    /// The user went past the step without a choice; it is not asked again.
    Skipped,
}

/// The setup steps still open.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    /// The open steps, in the order the setup asks them; empty when the setup
    /// has nothing to ask.
    pub open: Vec<SetupStep>,
}

/// Record the user's answer to one setup step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerSetupStepRequest {
    /// The step. [`SetupStep::DownloadFolder`] is refused: it is answered by
    /// choosing the folder (`update_settings` with `download_folder`).
    pub step: SetupStep,
    /// Answered or skipped; either way the step is not asked again.
    pub answer: SetupAnswer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_format_is_camel_case() {
        let req = AnswerSetupStepRequest {
            step: SetupStep::FontFolders,
            answer: SetupAnswer::Skipped,
        };
        assert_eq!(
            serde_json::to_value(&req).expect("json"),
            serde_json::json!({ "step": "fontFolders", "answer": "skipped" })
        );
        let status = SetupStatus {
            open: vec![SetupStep::DownloadFolder, SetupStep::Updates],
        };
        assert_eq!(
            serde_json::to_value(&status).expect("json"),
            serde_json::json!({ "open": ["downloadFolder", "updates"] })
        );
    }

    #[test]
    fn all_lists_the_steps_in_the_order_the_setup_asks_them() {
        let mut sorted = SetupStep::ALL;
        sorted.sort();
        assert_eq!(sorted, SetupStep::ALL);
        assert_eq!(SetupStep::ALL[1], SetupStep::Catalogs);
        assert_eq!(SetupStep::ALL[2], SetupStep::DownloadFolder);
    }
}
