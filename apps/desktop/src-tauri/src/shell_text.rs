//! The native shell's labels (the Help menu, the help window's title, the tray)
//! in the language the UI shows (E19.2). They come from the UI's own message
//! catalogs (`src/locales/<tag>.json`, the `shell` group), so a new locale needs
//! no Rust change for its words; the UI tells the shell its locale through the
//! `set_shell_locale` command at start and whenever the Language setting changes.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use fm_api::settings::Language;
use fm_api::{ApiError, ApiResult};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

/// The catalogs the app ships, by locale tag (the keys of `CATALOGS` in
/// `src/i18n/index.ts`).
const SOURCES: &[(&str, &str)] = &[
    ("en", include_str!("../../src/locales/en.json")),
    ("de", include_str!("../../src/locales/de.json")),
];

/// The locale every missing label falls back to.
pub const FALLBACK_LOCALE: &str = "en";

fn catalogs() -> &'static HashMap<&'static str, Value> {
    static CATALOGS: OnceLock<HashMap<&'static str, Value>> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        SOURCES
            .iter()
            .filter_map(|(tag, json)| match serde_json::from_str(json) {
                Ok(value) => Some((*tag, value)),
                Err(e) => {
                    tracing::warn!(error = %e, locale = tag, "a message catalog is not valid JSON");
                    None
                }
            })
            .collect()
    })
}

/// Whether the app ships a catalog for `tag`.
pub fn is_known(tag: &str) -> bool {
    catalogs().contains_key(tag)
}

fn lookup(tag: &str, key: &str) -> Option<&'static Value> {
    catalogs()
        .get(tag)?
        .pointer(&format!("/{}", key.replace('.', "/")))
}

/// The label `key` (`shell.tray.open`) in `tag`, else in English, else the key.
pub fn text(tag: &str, key: &str) -> String {
    [tag, FALLBACK_LOCALE]
        .iter()
        .find_map(|t| lookup(t, key)?.as_str())
        .map_or_else(|| key.to_owned(), str::to_owned)
}

/// The counted label `key` for `n`, `{count}` filled in: the `zero` form for 0
/// when the catalog has one, `one` for 1, else `other`. That is the plural rule
/// of every shipped locale; the UI uses `Intl.PluralRules`.
pub fn count(tag: &str, key: &str, n: usize) -> String {
    for t in [tag, FALLBACK_LOCALE] {
        let Some(forms) = lookup(t, key).and_then(Value::as_object) else {
            continue;
        };
        let form = match n {
            0 => forms.get("zero").or_else(|| forms.get("other")),
            1 => forms.get("one").or_else(|| forms.get("other")),
            _ => forms.get("other"),
        };
        if let Some(form) = form.and_then(Value::as_str) {
            return form.replace("{count}", &n.to_string());
        }
    }
    key.to_owned()
}

/// The locale the shell starts in: the chosen language, or English for
/// "System" until the UI, which reads the OS's languages, says otherwise.
pub fn initial_locale(language: Language) -> String {
    serde_json::to_value(language)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .filter(|tag| is_known(tag))
        .unwrap_or_else(|| FALLBACK_LOCALE.to_owned())
}

/// The locale the shell's labels are in; managed state.
#[derive(Debug)]
pub struct ShellLocale(Mutex<String>);

impl ShellLocale {
    /// Labels in `tag`.
    pub fn new(tag: impl Into<String>) -> Self {
        Self(Mutex::new(tag.into()))
    }

    /// The locale now.
    pub fn get(&self) -> String {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, tag: &str) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = tag.to_owned();
    }
}

/// The locale of `app`'s shell labels; English when none is managed.
pub fn current<R: Runtime>(app: &AppHandle<R>) -> String {
    app.try_state::<ShellLocale>()
        .map_or_else(|| FALLBACK_LOCALE.to_owned(), |s| s.get())
}

/// Shows the shell's labels in `tag`: the Help menu, an open help window and the
/// tray. `InvalidRequest` for a locale the app has no catalog for.
pub fn apply<R: Runtime>(app: &AppHandle<R>, tag: &str) -> ApiResult<()> {
    if !is_known(tag) {
        return Err(ApiError::invalid("This language is not available.")
            .with_detail(format!("no catalog for locale {tag:?}")));
    }
    match app.try_state::<ShellLocale>() {
        Some(state) => state.set(tag),
        None => {
            app.manage(ShellLocale::new(tag));
        }
    }
    crate::help::relabel(app, tag);
    crate::tray::relabel(app, tag);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_come_from_the_locale_then_english_then_the_key() {
        assert_eq!(text("en", "shell.tray.open"), "Open Font Manager");
        assert_eq!(text("de", "shell.tray.open"), "Font Manager öffnen");
        assert_eq!(
            text("de", "shell.helpPages.keyboard-shortcuts"),
            "Tastenkürzel"
        );
        assert_eq!(text("xx", "shell.tray.quit"), "Quit Font Manager");
        assert_eq!(text("de", "shell.nope"), "shell.nope");
    }

    #[test]
    fn counts_pick_zero_one_and_other() {
        let key = "shell.tray.active";
        assert_eq!(count("en", key, 0), "No fonts active");
        assert_eq!(count("en", key, 1), "1 font active");
        assert_eq!(count("en", key, 12), "12 fonts active");
        assert_eq!(count("de", key, 0), "Keine Schriften aktiv");
        assert_eq!(count("de", key, 1), "1 Schrift aktiv");
        assert_eq!(count("de", key, 3), "3 Schriften aktiv");
        assert_eq!(count("de", "shell.nope", 3), "shell.nope");
    }

    #[test]
    fn english_labels_match_the_core_and_the_help_pages() {
        use fm_core::lifecycle;
        assert_eq!(text("en", "shell.tray.open"), lifecycle::MENU_OPEN);
        assert_eq!(
            text("en", "shell.tray.deactivateSession"),
            lifecycle::MENU_DEACTIVATE_SESSION
        );
        assert_eq!(text("en", "shell.tray.quit"), lifecycle::MENU_QUIT);
        for n in [0, 1, 7] {
            assert_eq!(
                count("en", "shell.tray.active", n),
                lifecycle::active_label(n)
            );
        }
        for page in crate::help::HELP_PAGES {
            assert_eq!(
                text("en", &format!("shell.helpPages.{}", page.slug)),
                page.title
            );
        }
    }

    #[test]
    fn every_shipped_locale_has_every_shell_label() {
        let keys = [
            "shell.helpWindowTitle",
            "shell.tray.tooltip",
            "shell.tray.open",
            "shell.tray.deactivateSession",
            "shell.tray.quit",
        ];
        for (tag, _) in SOURCES {
            assert!(is_known(tag), "{tag} parses");
            for key in keys {
                assert!(
                    lookup(tag, key).and_then(Value::as_str).is_some(),
                    "{tag}: {key}"
                );
            }
            for page in crate::help::HELP_PAGES {
                let key = format!("shell.helpPages.{}", page.slug);
                assert!(lookup(tag, &key).is_some(), "{tag}: {key}");
            }
            assert!(lookup(tag, "shell.tray.active.other").is_some(), "{tag}");
        }
    }

    #[test]
    fn the_initial_locale_is_the_chosen_one_or_english_for_system() {
        assert_eq!(initial_locale(Language::System), "en");
        assert_eq!(initial_locale(Language::En), "en");
        assert_eq!(initial_locale(Language::De), "de");
    }
}
