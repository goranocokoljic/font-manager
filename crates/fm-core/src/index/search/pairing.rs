//! Pairing suggestions (E23.4): [`Index::suggest_pairings`].
//!
//! The rules are [`crate::pairing`]'s; the candidates are the searchable
//! rows held in memory ([`SearchCache`]), so a call costs one pass over them
//! and a summary read per suggestion, well inside the 50 ms budget.
//!
//! The pairing model (E33.2, [`crate::pairing::model`]) scores every pair
//! the rules let through; its score is blended into the rules' score. The
//! model reads the faces' fingerprints from the similar-font profiles, held
//! in the same cache. [`Index::train_pairing_model`] fits it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use fm_api::FaceId;
use fm_api::common::Slant;
use fm_api::search::{PairingRequest, PairingSuggestion};

use super::{Index, Row};
use crate::error::{FmError, FmResult};
use crate::grouping::family_key;
use crate::pairing::model::{
    self, Curated, FaceFeatures, Features, MODEL_REASON, MODEL_SAYS_PAIR, MODEL_SHARE, Model,
    NEGATIVES_PER_POSITIVE, Rng,
};
use crate::pairing::{PairFacts, Pairing, pair};
use crate::similar::Profile;

impl Index {
    /// The faces of other families that pair with `req.face`, best first,
    /// at most `req.limit`. Each family stands by one face, the closest to
    /// `req.face` in weight, then slant, then the lowest id; the rules judge
    /// that face, and the pairing model's score is blended into theirs
    /// ([`MODEL_SHARE`]). Ties go by family name, then id. A face that is
    /// not visible (unknown, its file gone, its source paused) is
    /// `NotFound`.
    pub fn suggest_pairings(&self, req: &PairingRequest) -> FmResult<Vec<PairingSuggestion>> {
        self.suggest_with(req, Some(model::embedded()?))
    }

    /// [`Index::suggest_pairings`] with `model`, or the rules alone.
    pub(crate) fn suggest_with(
        &self,
        req: &PairingRequest,
        model: Option<&Model>,
    ) -> FmResult<Vec<PairingSuggestion>> {
        self.refresh_search_cache()?;
        if model.is_some() {
            self.read_profiles()?;
        }
        let picked: Vec<(FaceId, Pairing)> = {
            let cache = self.search_cache.borrow();
            let profiles = cache.profiles.as_ref();
            let base = cache
                .rows
                .iter()
                .find(|r| r.id == req.face.0)
                .ok_or_else(|| FmError::not_found(format!("Face {}", req.face)))?;
            let mine = face_features(base, profiles);
            let mut ranked: Vec<(&Row, Pairing)> = representatives(&cache.rows, base)
                .into_iter()
                .filter_map(|row| {
                    let theirs = face_features(row, profiles);
                    let mut p = pair(&mine.facts, &theirs.facts)?;
                    if let Some(model) = model {
                        let chance = model.probability(&model::features(&mine, &theirs));
                        p.score = (1.0 - MODEL_SHARE) * p.score.min(1.0) + MODEL_SHARE * chance;
                        if chance >= MODEL_SAYS_PAIR {
                            p.reasons.push(MODEL_REASON.to_owned());
                        }
                    }
                    Some((row, p))
                })
                .collect();
            ranked.sort_by(|(a, p), (b, q)| {
                q.score
                    .total_cmp(&p.score)
                    // The grouping key: the family name in lower case.
                    .then_with(|| a.family_key.cmp(&b.family_key))
                    .then(a.id.cmp(&b.id))
            });
            ranked.truncate(req.limit as usize);
            ranked
                .into_iter()
                .map(|(row, p)| (FaceId(row.id), p))
                .collect()
        };
        // The summaries read the search cache again: its borrow ended above.
        picked
            .into_iter()
            .map(|(face, p)| {
                Ok(PairingSuggestion {
                    face: self.face_summary(face)?,
                    score: p.score.min(1.0),
                    reasons: p.reasons,
                })
            })
            .collect()
    }

    /// Fits the pairing model on this index (E33.2). For each curated pair
    /// whose two families are visible, and each direction, the base is the
    /// first family's [`regular_face`](Index::regular_face) and the
    /// candidates are the other families' faces as
    /// [`Index::suggest_pairings`] picks them: the partner is a positive,
    /// [`NEGATIVES_PER_POSITIVE`] candidates the rules let through that are
    /// no curated partner of the base are negatives, drawn with `seed`.
    /// Pairs whose families are not in the index are skipped.
    pub fn train_pairing_model(&self, curated: &[Curated], seed: u64) -> FmResult<Model> {
        self.refresh_search_cache()?;
        self.read_profiles()?;
        let cache = self.search_cache.borrow();
        let profiles = cache.profiles.as_ref();
        let mut partners: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for c in curated {
            let [a, b] = c.families.each_ref().map(|f| family_key(f));
            partners.entry(a.clone()).or_default().insert(b.clone());
            partners.entry(b).or_default().insert(a);
        }
        let mut rng = Rng::new(seed);
        let mut examples: Vec<(Features, bool)> = Vec::new();
        for c in curated {
            let [a, b] = &c.families;
            for (from, to) in [(a, b), (b, a)] {
                let (from, to) = (family_key(from), family_key(to));
                let Some(base) = regular_row(&cache.rows, &from) else {
                    continue;
                };
                let mine = face_features(base, profiles);
                let reps = representatives(&cache.rows, base);
                let Some(partner) = reps.iter().find(|r| r.family_key == to) else {
                    continue;
                };
                examples.push((
                    model::features(&mine, &face_features(partner, profiles)),
                    true,
                ));
                let others: Vec<&Row> = reps
                    .iter()
                    .copied()
                    .filter(|r| !partners[&from].contains(&r.family_key))
                    .filter(|r| pair(&mine.facts, &facts(r)).is_some())
                    .collect();
                for r in rng.sample(&others, NEGATIVES_PER_POSITIVE) {
                    examples.push((model::features(&mine, &face_features(r, profiles)), false));
                }
            }
        }
        Ok(model::fit(&examples, seed))
    }

    /// The face that stands for a visible family in training: the one
    /// closest to weight 400, upright before slanted, then the lowest id.
    #[cfg(test)]
    pub(crate) fn regular_face(&self, family: &str) -> FmResult<Option<FaceId>> {
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        Ok(regular_row(&cache.rows, &family_key(family)).map(|r| FaceId(r.id)))
    }
}

fn regular_row<'a>(rows: &'a [Row], key: &str) -> Option<&'a Row> {
    rows.iter().filter(|r| r.family_key == key).min_by_key(|r| {
        (
            r.text.weight.abs_diff(400),
            r.text.slant != Slant::Upright,
            r.id,
        )
    })
}

/// Each other family's face closest to `base` in weight, then slant, then
/// the lowest id; in family-key order.
fn representatives<'a>(rows: &'a [Row], base: &Row) -> Vec<&'a Row> {
    let closeness = |r: &Row| {
        (
            r.text.weight.abs_diff(base.text.weight),
            r.text.slant != base.text.slant,
            r.id,
        )
    };
    let mut nearest: HashMap<&str, &Row> = HashMap::new();
    for row in rows.iter().filter(|r| r.family_key != base.family_key) {
        let held = nearest.entry(&row.family_key).or_insert(row);
        if closeness(row) < closeness(held) {
            *held = row;
        }
    }
    let mut out: Vec<&Row> = nearest.into_values().collect();
    out.sort_by(|a, b| a.family_key.cmp(&b.family_key).then(a.id.cmp(&b.id)));
    out
}

fn face_features<'a>(
    row: &'a Row,
    profiles: Option<&'a HashMap<u32, Profile>>,
) -> FaceFeatures<'a> {
    FaceFeatures {
        facts: facts(row),
        weight: row.text.weight,
        width: row.text.width,
        fingerprint: profiles
            .and_then(|p| p.get(&row.id))
            .and_then(|p| p.fingerprint.as_ref())
            .map(|f| f.vector.as_slice()),
    }
}

fn facts(row: &Row) -> PairFacts<'_> {
    PairFacts {
        family: &row.family,
        designer: row.designer.as_deref(),
        class: row.facts.classification,
        x_height: row.x_height,
    }
}

#[cfg(test)]
mod tests;
