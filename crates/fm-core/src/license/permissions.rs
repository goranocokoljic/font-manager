//! What a license lets the user do, in plain words (E42.12).
//!
//! The wording is `data/licenses/permissions.json`, embedded at build time. The
//! choice of rule is here and follows [`web_use`], so the lines never disagree
//! with the "OK for web" answer next to them.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use fm_api::common::Embedding;
use fm_api::library::{LicensePermissions, LicensePermissionsSource, LicenseType, OpenLicense};
use serde::Deserialize;

use super::{WebReason, web_use};
use crate::error::{FmError, FmResult};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./permissions.schema.json";

/// The data file, embedded at build time.
const FILE: (&str, &str) = (
    "permissions.json",
    include_str!("../../../../data/licenses/permissions.json"),
);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Rule {
    may: Vec<String>,
    may_not: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenRules {
    ofl: Rule,
    apache: Rule,
    ufl: Rule,
    mit: Rule,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeclaredRules {
    desktop: Rule,
    web: Rule,
    app: Rule,
    trial: Rule,
    personal_only: Rule,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EmbeddingRules {
    restricted: Rule,
    preview_and_print: Rule,
    editable: Rule,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Rules {
    #[serde(rename = "$schema")]
    schema: String,
    open_licenses: OpenRules,
    declared: DeclaredRules,
    embedding: EmbeddingRules,
}

impl Rules {
    fn open(&self, license: OpenLicense) -> &Rule {
        match license {
            OpenLicense::Ofl => &self.open_licenses.ofl,
            OpenLicense::Apache => &self.open_licenses.apache,
            OpenLicense::Ufl => &self.open_licenses.ufl,
            OpenLicense::Mit => &self.open_licenses.mit,
        }
    }

    /// The rule of a declared type; `Unknown` says nothing and has none.
    fn declared(&self, license: LicenseType) -> Option<&Rule> {
        match license {
            LicenseType::Desktop => Some(&self.declared.desktop),
            LicenseType::Web => Some(&self.declared.web),
            LicenseType::App => Some(&self.declared.app),
            LicenseType::Trial => Some(&self.declared.trial),
            LicenseType::PersonalOnly => Some(&self.declared.personal_only),
            LicenseType::Unknown => None,
        }
    }

    /// The rule of an embedding permission. `Installable` has none: `fsType` 0
    /// is also what a file with no `OS/2` table reads as, so it says nothing.
    fn embedding(&self, embedding: Embedding) -> Option<&Rule> {
        match embedding {
            Embedding::Restricted => Some(&self.embedding.restricted),
            Embedding::PreviewAndPrint => Some(&self.embedding.preview_and_print),
            Embedding::Editable => Some(&self.embedding.editable),
            Embedding::Installable => None,
        }
    }

    fn named(&self) -> [(&'static str, &Rule); 12] {
        [
            ("openLicenses.ofl", &self.open_licenses.ofl),
            ("openLicenses.apache", &self.open_licenses.apache),
            ("openLicenses.ufl", &self.open_licenses.ufl),
            ("openLicenses.mit", &self.open_licenses.mit),
            ("declared.desktop", &self.declared.desktop),
            ("declared.web", &self.declared.web),
            ("declared.app", &self.declared.app),
            ("declared.trial", &self.declared.trial),
            ("declared.personalOnly", &self.declared.personal_only),
            ("embedding.restricted", &self.embedding.restricted),
            (
                "embedding.previewAndPrint",
                &self.embedding.preview_and_print,
            ),
            ("embedding.editable", &self.embedding.editable),
        ]
    }
}

/// The rules, loaded once. The data is embedded and its tests reject a broken
/// file, so a load error is a build defect: it is logged and no face gets lines.
static RULES: LazyLock<Option<Rules>> = LazyLock::new(|| match parse(FILE.0, FILE.1) {
    Ok(rules) => Some(rules),
    Err(e) => {
        tracing::error!(error = %e, "license permissions could not be loaded");
        None
    }
});

/// Parses and checks the data file: every rule of `permissions.schema.json`.
fn parse(file: &str, json: &str) -> FmResult<Rules> {
    let rules: Rules = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if rules.schema != SCHEMA_REF {
        return Err(damaged(file, &format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    for (name, rule) in rules.named() {
        check(file, name, rule)?;
    }
    Ok(rules)
}

/// One rule: at least one line, none blank or padded, none in a list twice.
fn check(file: &str, name: &str, rule: &Rule) -> FmResult<()> {
    if rule.may.is_empty() && rule.may_not.is_empty() {
        return Err(damaged(file, &format!("{name}: the rule has no lines")));
    }
    for lines in [&rule.may, &rule.may_not] {
        let mut seen = BTreeSet::new();
        for line in lines {
            if line.trim().is_empty() || line.trim() != line {
                return Err(damaged(
                    file,
                    &format!("{name}: \"{line}\" is empty or has white space at an end"),
                ));
            }
            if !seen.insert(line) {
                return Err(damaged(
                    file,
                    &format!("{name}: \"{line}\" is listed twice"),
                ));
            }
        }
    }
    Ok(())
}

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/licenses/{file}: {detail}"))
}

/// What a face's license lets the user do; `None` when nothing is known.
///
/// The order is the one of [`web_use`], first match wins:
///
/// 1. A declared Web, Desktop, Trial or Personal-only type gives its own lines.
/// 2. Restricted or preview-and-print embedding gives its lines, also over an
///    open license. A declared App type gives its lines instead: the user says
///    they hold that license.
/// 3. A recognized open license gives its lines, also over a declared App type,
///    because the open license allows more.
/// 4. Declared App gives its lines; else editable embedding gives its lines.
/// 5. Nothing else is known: installable embedding, or declared Unknown.
pub fn permissions(
    embedding: Embedding,
    open_license: Option<OpenLicense>,
    declared: Option<LicenseType>,
) -> Option<LicensePermissions> {
    from_rules(RULES.as_ref()?, embedding, open_license, declared)
}

fn from_rules(
    rules: &Rules,
    embedding: Embedding,
    open_license: Option<OpenLicense>,
    declared: Option<LicenseType>,
) -> Option<LicensePermissions> {
    let (rule, source) = match web_use(embedding, open_license, declared) {
        WebReason::DeclaredWeb => (&rules.declared.web, LicensePermissionsSource::Declared),
        WebReason::Declared(t) => (rules.declared(t)?, LicensePermissionsSource::Declared),
        WebReason::Open(l) => (rules.open(l), LicensePermissionsSource::OpenLicense),
        WebReason::Embedding(_) | WebReason::NoWebLicense => {
            if declared == Some(LicenseType::App) {
                (&rules.declared.app, LicensePermissionsSource::Declared)
            } else {
                (
                    rules.embedding(embedding)?,
                    LicensePermissionsSource::EmbeddingBits,
                )
            }
        }
    };
    Some(LicensePermissions {
        may: rule.may.clone(),
        may_not: rule.may_not.clone(),
        source,
    })
}

#[cfg(test)]
mod tests;
