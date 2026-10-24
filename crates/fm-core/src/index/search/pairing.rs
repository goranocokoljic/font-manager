//! Pairing suggestions (E23.4): [`Index::suggest_pairings`].
//!
//! The rules are [`crate::pairing`]'s; the candidates are the searchable
//! rows held in memory ([`SearchCache`]), so a call costs one pass over them
//! and a summary read per suggestion, well inside the 50 ms budget.

use std::collections::HashMap;

use fm_api::FaceId;
use fm_api::search::{PairingRequest, PairingSuggestion};

use super::{Index, Row};
use crate::error::{FmError, FmResult};
use crate::pairing::{PairFacts, Pairing, pair};

impl Index {
    /// The faces of other families that pair with `req.face`, best first,
    /// at most `req.limit`. Each family stands by one face, the closest to
    /// `req.face` in weight, then slant, then the lowest id; the rules judge
    /// that face. Ties go by family name, then id. A face that is not
    /// visible (unknown, its file gone, its source paused) is `NotFound`.
    pub fn suggest_pairings(&self, req: &PairingRequest) -> FmResult<Vec<PairingSuggestion>> {
        self.refresh_search_cache()?;
        let picked: Vec<(FaceId, Pairing)> = {
            let cache = self.search_cache.borrow();
            let base = cache
                .rows
                .iter()
                .find(|r| r.id == req.face.0)
                .ok_or_else(|| FmError::not_found(format!("Face {}", req.face)))?;
            let mine = facts(base);
            let closeness = |r: &Row| {
                (
                    r.text.weight.abs_diff(base.text.weight),
                    r.text.slant != base.text.slant,
                    r.id,
                )
            };
            // Each family's face closest to the base stands for it.
            let mut nearest: HashMap<&str, &Row> = HashMap::new();
            for row in cache
                .rows
                .iter()
                .filter(|r| r.family_key != base.family_key)
            {
                let held = nearest.entry(&row.family_key).or_insert(row);
                if closeness(row) < closeness(held) {
                    *held = row;
                }
            }
            let mut ranked: Vec<(&Row, Pairing)> = nearest
                .into_values()
                .filter_map(|row| Some((row, pair(&mine, &facts(row))?)))
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
