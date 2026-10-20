//! The GSUB features that produce or replace one glyph (E11.2), for the
//! glyph detail.
//!
//! Every lookup a feature lists is walked, nested lookups of contextual
//! lookups included, at the default location (feature variations are not
//! applied):
//!
//! - a feature **produces** the glyph when a lookup substitutes it for other
//!   glyphs: the output of a single, multiple, alternate or reverse
//!   substitution, or the ligature glyph of a ligature (`liga` for `ﬁ`);
//! - a feature **replaces** the glyph when a lookup substitutes another
//!   glyph for it: an input of those substitutions, any component of a
//!   ligature included (`smcp`, `ss01`, `liga` for `f`). A substitution of a
//!   glyph by itself is neither.
//!
//! Malformed GSUB data is never a panic: the walk stops and both lists are
//! empty.

use std::collections::{BTreeSet, HashMap};

use fm_api::common::FeatureInfo;
use read_fonts::tables::gsub::{SingleSubst, SubstitutionLookupList, SubstitutionSubtables};
use read_fonts::tables::layout::{ChainedSequenceContext, SequenceContext};
use read_fonts::types::GlyphId16;
use read_fonts::{FontRef, ReadError, TableProvider};
use skrifa::GlyphId;

use crate::features::feature_info;

/// How deep contextual lookups may nest; deeper ones are not walked.
const MAX_NESTING: u8 = 16;

/// The GSUB features that produce a glyph and the ones that replace it, by
/// tag, with friendly names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlyphFeatures {
    /// Features that produce the glyph from other glyphs.
    pub produced_by: Vec<FeatureInfo>,
    /// Features that replace the glyph with another.
    pub replaced_by: Vec<FeatureInfo>,
}

/// The features of `font`'s GSUB that produce or replace `glyph`. A font
/// without GSUB, or with a malformed one, has none.
pub fn glyph_features(font: &FontRef<'_>, glyph: GlyphId) -> GlyphFeatures {
    walk(font, glyph).unwrap_or_default()
}

fn walk(font: &FontRef<'_>, glyph: GlyphId) -> Result<GlyphFeatures, ReadError> {
    // GSUB addresses glyphs with 16 bits: a larger id is in no lookup.
    let Ok(glyph) = GlyphId16::try_from(glyph) else {
        return Ok(GlyphFeatures::default());
    };
    let gsub = match font.gsub() {
        Err(ReadError::TableIsMissing(_)) => return Ok(GlyphFeatures::default()),
        gsub => gsub?,
    };
    let features = match gsub.feature_list() {
        Err(ReadError::NullOffset) => return Ok(GlyphFeatures::default()),
        features => features?,
    };
    let mut lookups = Lookups {
        list: gsub.lookup_list()?,
        glyph,
        memo: HashMap::new(),
    };
    let mut produced = BTreeSet::new();
    let mut replaced = BTreeSet::new();
    for record in features.feature_records() {
        let feature = record.feature(features.offset_data())?;
        let mut role = Role::default();
        for index in feature.lookup_list_indices() {
            role = role.or(lookups.role(index.get(), 0)?);
        }
        let tag = record.feature_tag().to_string();
        if role.produces {
            produced.insert(tag.clone());
        }
        if role.replaces {
            replaced.insert(tag);
        }
    }
    let infos = |tags: BTreeSet<String>| tags.iter().map(|t| feature_info(t)).collect();
    Ok(GlyphFeatures {
        produced_by: infos(produced),
        replaced_by: infos(replaced),
    })
}

/// What a lookup does to the glyph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Role {
    produces: bool,
    replaces: bool,
}

impl Role {
    fn or(self, other: Self) -> Self {
        Self {
            produces: self.produces || other.produces,
            replaces: self.replaces || other.replaces,
        }
    }

    /// Notes one substitution of `from` by `to`.
    fn substitution(&mut self, glyph: GlyphId16, from: GlyphId16, to: &[GlyphId16]) {
        if to == [from] {
            return;
        }
        self.replaces |= from == glyph;
        self.produces |= from != glyph && to.contains(&glyph);
    }
}

/// The lookup list with the role of each lookup walked so far.
struct Lookups<'a> {
    list: SubstitutionLookupList<'a>,
    glyph: GlyphId16,
    memo: HashMap<u16, Role>,
}

impl Lookups<'_> {
    fn role(&mut self, index: u16, depth: u8) -> Result<Role, ReadError> {
        if let Some(role) = self.memo.get(&index) {
            return Ok(*role);
        }
        if depth > MAX_NESTING {
            return Ok(Role::default());
        }
        // A lookup that nests itself sees no role for itself, not a loop.
        self.memo.insert(index, Role::default());
        let lookup = self.list.lookups().get(usize::from(index))?;
        let g = self.glyph;
        let mut role = Role::default();
        match lookup.subtables()? {
            SubstitutionSubtables::Single(subtables) => {
                for table in subtables.iter() {
                    match table? {
                        SingleSubst::Format1(t) => {
                            let delta = t.delta_glyph_id();
                            for from in t.coverage()?.iter() {
                                // The delta is added modulo 65536.
                                let to = GlyphId16::new(from.to_u16().wrapping_add_signed(delta));
                                role.substitution(g, from, &[to]);
                            }
                        }
                        SingleSubst::Format2(t) => {
                            let to = t.substitute_glyph_ids();
                            for (from, to) in t.coverage()?.iter().zip(to) {
                                role.substitution(g, from, &[to.get()]);
                            }
                        }
                    }
                }
            }
            SubstitutionSubtables::Multiple(subtables) => {
                for t in subtables.iter() {
                    let t = t?;
                    for (from, seq) in t.coverage()?.iter().zip(t.sequences().iter()) {
                        let to: Vec<GlyphId16> = seq?
                            .substitute_glyph_ids()
                            .iter()
                            .map(|g| g.get())
                            .collect();
                        role.substitution(g, from, &to);
                    }
                }
            }
            SubstitutionSubtables::Alternate(subtables) => {
                for t in subtables.iter() {
                    let t = t?;
                    for (from, set) in t.coverage()?.iter().zip(t.alternate_sets().iter()) {
                        for to in set?.alternate_glyph_ids() {
                            role.substitution(g, from, &[to.get()]);
                        }
                    }
                }
            }
            SubstitutionSubtables::Ligature(subtables) => {
                for t in subtables.iter() {
                    let t = t?;
                    for (first, set) in t.coverage()?.iter().zip(t.ligature_sets().iter()) {
                        for ligature in set?.ligatures().iter() {
                            let ligature = ligature?;
                            let to = ligature.ligature_glyph();
                            role.substitution(g, first, &[to]);
                            for component in ligature.component_glyph_ids() {
                                role.substitution(g, component.get(), &[to]);
                            }
                        }
                    }
                }
            }
            SubstitutionSubtables::Reverse(subtables) => {
                for t in subtables.iter() {
                    let t = t?;
                    let to = t.substitute_glyph_ids();
                    for (from, to) in t.coverage()?.iter().zip(to) {
                        role.substitution(g, from, &[to.get()]);
                    }
                }
            }
            SubstitutionSubtables::Contextual(subtables) => {
                for t in subtables.iter() {
                    for nested in context_lookups(&t?)? {
                        role = role.or(self.role(nested, depth + 1)?);
                    }
                }
            }
            SubstitutionSubtables::ChainContextual(subtables) => {
                for t in subtables.iter() {
                    for nested in chain_context_lookups(&t?)? {
                        role = role.or(self.role(nested, depth + 1)?);
                    }
                }
            }
            SubstitutionSubtables::EmptyExtension => {}
        }
        self.memo.insert(index, role);
        Ok(role)
    }
}

/// The lookups a contextual subtable applies.
fn context_lookups(t: &SequenceContext<'_>) -> Result<Vec<u16>, ReadError> {
    let mut out = Vec::new();
    match t {
        SequenceContext::Format1(t) => {
            for set in t.seq_rule_sets().iter().flatten() {
                for rule in set?.seq_rules().iter() {
                    out.extend(
                        rule?
                            .seq_lookup_records()
                            .iter()
                            .map(|r| r.lookup_list_index()),
                    );
                }
            }
        }
        SequenceContext::Format2(t) => {
            for set in t.class_seq_rule_sets().iter().flatten() {
                for rule in set?.class_seq_rules().iter() {
                    out.extend(
                        rule?
                            .seq_lookup_records()
                            .iter()
                            .map(|r| r.lookup_list_index()),
                    );
                }
            }
        }
        SequenceContext::Format3(t) => {
            out.extend(t.seq_lookup_records().iter().map(|r| r.lookup_list_index()));
        }
    }
    Ok(out)
}

/// The lookups a chained contextual subtable applies.
fn chain_context_lookups(t: &ChainedSequenceContext<'_>) -> Result<Vec<u16>, ReadError> {
    let mut out = Vec::new();
    match t {
        ChainedSequenceContext::Format1(t) => {
            for set in t.chained_seq_rule_sets().iter().flatten() {
                for rule in set?.chained_seq_rules().iter() {
                    out.extend(
                        rule?
                            .seq_lookup_records()
                            .iter()
                            .map(|r| r.lookup_list_index()),
                    );
                }
            }
        }
        ChainedSequenceContext::Format2(t) => {
            for set in t.chained_class_seq_rule_sets().iter().flatten() {
                for rule in set?.chained_class_seq_rules().iter() {
                    out.extend(
                        rule?
                            .seq_lookup_records()
                            .iter()
                            .map(|r| r.lookup_list_index()),
                    );
                }
            }
        }
        ChainedSequenceContext::Format3(t) => {
            out.extend(t.seq_lookup_records().iter().map(|r| r.lookup_list_index()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
