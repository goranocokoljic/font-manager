//! What a font file says about its license (E15.1): the embedding permission
//! of `OS/2.fsType`, and the open license its name table states.
//!
//! An open license is recognized only from a statement, never from a
//! mention: the license description (name ID 13) must grant it ("licensed
//! under the SIL Open Font License"), or begin a line with the license's
//! own title ("Apache License, Version 2.0"), or the license URL (name ID
//! 14) must be the address of the license itself. A description that only
//! names a license ("unlike OFL fonts, this one is commercial") is not one.

use fm_api::common::Embedding;
use fm_api::library::OpenLicense;

/// The embedding permission `OS/2.fsType` grants.
///
/// Bits 0–3 are the usage permissions: none set is installable, bit 1
/// restricted, bit 2 preview and print, bit 3 editable. Fonts made before
/// OpenType 1.3 may set several; the OpenType spec says the least
/// restrictive one wins. Bit 0 is reserved, and the other bits (no
/// subsetting, bitmap embedding only) do not change the permission.
pub fn embedding(fs_type: u16) -> Embedding {
    if fs_type & 0x0008 != 0 {
        Embedding::Editable
    } else if fs_type & 0x0004 != 0 {
        Embedding::PreviewAndPrint
    } else if fs_type & 0x0002 != 0 {
        Embedding::Restricted
    } else {
        Embedding::Installable
    }
}

/// The open license a face's license description (name ID 13) or license
/// URL (name ID 14) states; `None` when neither states one.
pub fn open_license(description: Option<&str>, url: Option<&str>) -> Option<OpenLicense> {
    description
        .and_then(stated_in_text)
        .or_else(|| url.and_then(license_at))
}

/// Each license with the names a text calls it by, lowercase.
const NAMES: [(OpenLicense, &[&str]); 4] = [
    (
        OpenLicense::Ofl,
        &["sil open font license", "open font license", "ofl"],
    ),
    (OpenLicense::Apache, &["apache license", "apache 2.0"]),
    (
        OpenLicense::Ufl,
        &["ubuntu font licence", "ubuntu font license"],
    ),
    (OpenLicense::Mit, &["mit license", "mit"]),
];

/// The words that grant a license to what follows them.
const GRANTS: [&str; 6] = [
    "licensed under",
    "released under",
    "distributed under",
    "available under",
    "made available under",
    "under the terms of",
];

/// The words that, just before a grant, take it back ("not licensed under").
const NEGATIONS: [&str; 3] = ["not ", "never ", "n't "];

/// The license `text` states: a grant, a title line, or the MIT grant text.
fn stated_in_text(text: &str) -> Option<OpenLicense> {
    let text = text.to_lowercase();
    granted(&text).or_else(|| titled(&text)).or_else(|| {
        text.contains("permission is hereby granted, free of charge")
            .then_some(OpenLicense::Mit)
    })
}

/// The license a grant names, up to the end of its sentence.
fn granted(text: &str) -> Option<OpenLicense> {
    for grant in GRANTS {
        for (at, _) in text.match_indices(grant) {
            let before = &text[..at];
            if NEGATIONS.iter().any(|n| before.ends_with(n)) {
                continue;
            }
            let rest = &text[at + grant.len()..];
            let sentence = rest
                .split(['.', ';', '\n', '\r'])
                .next()
                .unwrap_or_default();
            if let Some(license) = named_in(sentence) {
                return Some(license);
            }
        }
    }
    None
}

/// The license whose title begins a line of `text`, followed by its version
/// ("SIL Open Font License Version 1.1", "Apache License\nVersion 2.0") or
/// alone on its line ("MIT License").
fn titled(text: &str) -> Option<OpenLicense> {
    let starts = std::iter::once(0).chain(text.match_indices(['\n', '\r']).map(|(at, _)| at));
    for at in starts {
        let line = text[at..].trim_start();
        for (license, names) in NAMES {
            let Some(after) = names.first().and_then(|title| line.strip_prefix(title)) else {
                continue;
            };
            let alone = after
                .split(['\n', '\r'])
                .next()
                .is_some_and(|l| l.trim().is_empty());
            let versioned = after
                .trim_start_matches([',', ' ', '\t', '\r', '\n'])
                .starts_with("version");
            if versioned || alone && license == OpenLicense::Mit {
                return Some(license);
            }
        }
    }
    None
}

/// The first license `sentence` names, as a whole word.
fn named_in(sentence: &str) -> Option<OpenLicense> {
    NAMES.iter().find_map(|(license, names)| {
        names
            .iter()
            .any(|name| has_word(sentence, name))
            .then_some(*license)
    })
}

/// `needle` occurs in `hay` with no letter or digit right before or after it.
fn has_word(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(at, _)| {
        let before = hay[..at].chars().next_back();
        let after = hay[at + needle.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// The license `url` is the address of: the license text or its home page,
/// not a site that merely talks about it.
fn license_at(url: &str) -> Option<OpenLicense> {
    let url = url.trim().to_lowercase();
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(&url);
    let url = url.strip_prefix("www.").unwrap_or(url);
    let url = url.trim_end_matches(['/', '.']);
    let ofl = url.starts_with("openfontlicense.org")
        || url.starts_with("scripts.sil.org/ofl")
        || url.starts_with("scripts.sil.org") && url.contains("id=ofl")
        || url.starts_with("opensource.org/licenses/ofl");
    let apache = url.starts_with("apache.org/licenses/license-2.0")
        || url.starts_with("opensource.org/licenses/apache-2.0");
    let ufl = url.starts_with("font.ubuntu.com/ufl")
        || url.starts_with("font.ubuntu.com/licence")
        || url.starts_with("ubuntu.com/legal/terms-and-policies/font-licence");
    let mit = url.starts_with("opensource.org/licenses/mit")
        || url.starts_with("opensource.org/license/mit");
    [
        (ofl, OpenLicense::Ofl),
        (apache, OpenLicense::Apache),
        (ufl, OpenLicense::Ufl),
        (mit, OpenLicense::Mit),
    ]
    .into_iter()
    .find_map(|(yes, license)| yes.then_some(license))
}

#[cfg(test)]
mod tests;
