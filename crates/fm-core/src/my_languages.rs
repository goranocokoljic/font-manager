//! The languages the user works in (E56.3, ADR 0027): preset ids of
//! `list_languages`, stored in `config.json` as `Settings::my_languages`.
//!
//! Every rule and guard of the Duplicates module that names "my languages"
//! goes through [`apply`], so with no language set each of them does nothing
//! and its answer says so ([`MyLanguagesStatus::NotSet`]).

use std::collections::BTreeSet;

use fm_api::settings::MyLanguagesStatus;

use crate::charsets;
use crate::error::FmResult;

/// The ids of every language preset this version ships.
pub fn known_ids() -> FmResult<BTreeSet<String>> {
    Ok(charsets::load()?.into_iter().map(|c| c.code).collect())
}

/// `ids` split into the ones this version has a preset for, in their order,
/// and the ones it has not (a file from a newer version, a preset removed, a
/// hand edit). An empty list reads no data file.
pub fn split_known(ids: &[String]) -> FmResult<(Vec<String>, Vec<String>)> {
    if ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let known = known_ids()?;
    Ok(ids.iter().cloned().partition(|id| known.contains(id)))
}

/// What a rule or guard that names "my languages" did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied<T> {
    /// Whether any language is set; `NotSet` means the rule did nothing.
    pub status: MyLanguagesStatus,
    /// The rule's result; `None` when no language is set.
    pub outcome: Option<T>,
}

/// Runs `rule` over the user's languages, or nothing when none is set.
pub fn apply<T>(my_languages: &[String], rule: impl FnOnce(&[String]) -> T) -> Applied<T> {
    match MyLanguagesStatus::of(my_languages) {
        MyLanguagesStatus::NotSet => Applied {
            status: MyLanguagesStatus::NotSet,
            outcome: None,
        },
        MyLanguagesStatus::Set => Applied {
            status: MyLanguagesStatus::Set,
            outcome: Some(rule(my_languages)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_language_set_a_rule_does_nothing_and_says_so() {
        let mut ran = false;
        let applied = apply(&[], |_| ran = true);
        assert_eq!(applied.status, MyLanguagesStatus::NotSet);
        assert_eq!(applied.outcome, None);
        assert!(!ran, "the rule must not run without a language");
    }

    #[test]
    fn with_languages_set_the_rule_runs_over_them() {
        let ids = vec!["de".to_owned(), "fr".to_owned()];
        let applied = apply(&ids, <[String]>::len);
        assert_eq!(applied.status, MyLanguagesStatus::Set);
        assert_eq!(applied.outcome, Some(2));
    }

    #[test]
    fn unknown_ids_are_split_off_and_the_order_is_kept() {
        let known = known_ids().expect("presets");
        let mut presets = known.iter().take(2).cloned();
        let (a, b) = (
            presets.next().expect("first preset"),
            presets.next().expect("second preset"),
        );
        let ids = vec![b.clone(), "zz-Gone".to_owned(), a.clone()];
        let (kept, dropped) = split_known(&ids).expect("split");
        assert_eq!(kept, [b, a]);
        assert_eq!(dropped, ["zz-Gone"]);
        assert_eq!(split_known(&[]).expect("empty"), (Vec::new(), Vec::new()));
    }
}
