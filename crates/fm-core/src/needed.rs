//! The fonts a document needs (E29.2): the design-app plugins name each font
//! by its PostScript name, full name, or family and style, and the library
//! answers with the face that stands for it.
//!
//! - **Rules.** The names of a needed font are tried in the order of
//!   [`NeededFontRule`]: the PostScript name, then the full name, then the
//!   family and style, first as the typographic family and subfamily (name
//!   IDs 16 and 17), then as the legacy family and subfamily (1 and 2). The
//!   first rule that finds a face wins. Names compare ignoring case and
//!   runs of white space ([`family_key`]); a family without a style asks for
//!   its "Regular" style.
//! - **Several faces.** When a rule finds several faces (copies of one font
//!   in two folders, or OTF next to TTF), an active face wins, then a
//!   healthy one, then the lowest face id. A broken face is never matched.
//!
//! The faces come from [`Index::needed_faces`](crate::index::Index), one
//! read of the face table per request, so 100 needed fonts cost one query
//! and 100 hash lookups.

use std::collections::HashMap;

use fm_api::FaceId;
use fm_api::activation::{NeededFont, NeededFontMatch, NeededFontRule};

use crate::grouping::{DEFAULT_STYLE, family_key};

/// One face as the matching sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NeededFace {
    /// The face.
    pub id: FaceId,
    /// Its PostScript name; empty when the font has none.
    pub postscript_name: String,
    /// Its full name (name ID 4).
    pub full_name: Option<String>,
    /// Its typographic family and subfamily (name ID 16, and 17 or else 2),
    /// when the font has a typographic family.
    pub typographic: Option<(String, String)>,
    /// Its legacy family and subfamily (name IDs 1 and 2), when known.
    pub legacy: Option<(String, String)>,
    /// Its health is `Ok`.
    pub healthy: bool,
    /// Its health is `Broken`: it is never matched.
    pub broken: bool,
    /// The app has it active.
    pub active: bool,
}

/// The faces by each rule's key, best face first.
#[derive(Debug, Default)]
pub(crate) struct NeededIndex {
    by: HashMap<(NeededFontRule, String), FaceId>,
}

impl NeededIndex {
    /// The lookup over `faces`; broken faces are left out.
    pub(crate) fn new(faces: &[NeededFace]) -> Self {
        let mut ranked: Vec<&NeededFace> = faces.iter().filter(|f| !f.broken).collect();
        // Best first, so the first face stored under a key is the one it names.
        ranked.sort_by_key(|f| (!f.active, !f.healthy, f.id));
        let mut by = HashMap::new();
        for face in ranked {
            let mut put = |rule, key: Option<String>| {
                if let Some(key) = key.filter(|k| !k.is_empty()) {
                    by.entry((rule, key)).or_insert(face.id);
                }
            };
            put(
                NeededFontRule::PostscriptName,
                Some(family_key(&face.postscript_name)),
            );
            put(
                NeededFontRule::FullName,
                face.full_name.as_deref().map(family_key),
            );
            put(
                NeededFontRule::TypographicFamily,
                face.typographic.as_ref().and_then(pair_key),
            );
            put(
                NeededFontRule::Family,
                face.legacy.as_ref().and_then(pair_key),
            );
        }
        Self { by }
    }

    /// The face `font` names and the rule that found it; `None` when the
    /// library has none.
    pub(crate) fn find(&self, font: &NeededFont) -> Option<(FaceId, NeededFontRule)> {
        let name = |n: &Option<String>| n.as_deref().map(family_key).filter(|k| !k.is_empty());
        let family = name(&font.family).map(|family| {
            let style = name(&font.style).unwrap_or_else(|| family_key(DEFAULT_STYLE));
            key(&family, &style)
        });
        [
            (NeededFontRule::PostscriptName, name(&font.postscript_name)),
            (NeededFontRule::FullName, name(&font.full_name)),
            (NeededFontRule::TypographicFamily, family.clone()),
            (NeededFontRule::Family, family),
        ]
        .into_iter()
        .find_map(|(rule, key)| {
            key.and_then(|k| self.by.get(&(rule, k)))
                .map(|&face| (face, rule))
        })
    }
}

/// Matches every needed font against `faces`: the matched ones and the
/// unmatched ones, each in request order.
pub(crate) fn match_fonts(
    fonts: Vec<NeededFont>,
    faces: &[NeededFace],
) -> (Vec<NeededFontMatch>, Vec<NeededFont>) {
    let index = NeededIndex::new(faces);
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    for font in fonts {
        match index.find(&font) {
            Some((face, matched_by)) => matched.push(NeededFontMatch {
                font,
                face,
                matched_by,
            }),
            None => unmatched.push(font),
        }
    }
    (matched, unmatched)
}

/// The key of a family and style pair; `None` without a family.
fn pair_key((family, style): &(String, String)) -> Option<String> {
    let family = family_key(family);
    if family.is_empty() {
        return None;
    }
    let style = Some(family_key(style))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| family_key(DEFAULT_STYLE));
    Some(key(&family, &style))
}

/// Family and style in one key; `\u{1f}` (unit separator) is in no name.
fn key(family: &str, style: &str) -> String {
    format!("{family}\u{1f}{style}")
}

#[cfg(test)]
mod tests;
