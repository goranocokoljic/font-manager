//! Similar-glyph search (E49.8, ADR 0023): [`Index::find_similar_glyph`].
//!
//! The candidates are the searchable faces (files that are not gone, sources
//! that are not paused) with a current descriptor of the character, less the
//! reference face; the other faces of its family stay, so a wall sorted by a
//! reference glyph keeps them. Each is ranked by
//! [`similarity::Prepared::distance`] to the reference, closest first;
//! equal distances keep face-id order, so a ranking never flickers.
//!
//! This reads the stored descriptors only: describing what is missing is the
//! caller's step ([`Index::similar_glyph_todo`]), done off the index lock.
//! The descriptors of a character are read once per database change and
//! kept with the search rows ([`super::SearchCache`]): a query after that
//! is one pass over memory (ADR 0023, the 150 ms budget).

use std::collections::{HashMap, HashSet};

use fm_api::FaceId;
use fm_api::search::{GlyphReference, SimilarGlyphFace, SimilarGlyphRequest};

use super::Index;
use crate::error::{FmError, FmResult};
use crate::glyphs::similarity::Prepared;
use crate::index::DescriptorTodo;

/// The request's code point as a character; anything else is not a valid
/// request.
pub(crate) fn check_code_point(code_point: u32) -> FmResult<char> {
    char::from_u32(code_point)
        .ok_or_else(|| FmError::invalid(format!("U+{code_point:04X} is not a character.")))
}

/// What similar-glyph queries keep between database changes, in the
/// search cache: reset whenever it is.
#[derive(Default)]
pub(in crate::index) struct GlyphCache {
    /// Characters with no face left to describe.
    complete: HashSet<u32>,
    /// The current descriptors of a character by face id, each with whether
    /// the face is searchable.
    stored: HashMap<u32, Vec<(u32, bool, Prepared)>>,
}

impl Index {
    /// The faces of read files whose descriptor of `code_point` is missing
    /// or out of date ([`Index::glyph_descriptor_todo`]); once that is none,
    /// remembered until the database changes, so a query of a described
    /// character does not ask the database again.
    pub fn similar_glyph_todo(&self, code_point: u32) -> FmResult<Vec<DescriptorTodo>> {
        self.refresh_search_cache()?;
        if self
            .search_cache
            .borrow()
            .glyphs
            .complete
            .contains(&code_point)
        {
            return Ok(Vec::new());
        }
        let todo = self.glyph_descriptor_todo(code_point)?;
        if todo.is_empty() {
            self.search_cache
                .borrow_mut()
                .glyphs
                .complete
                .insert(code_point);
        }
        Ok(todo)
    }

    /// The searchable faces whose stored glyph of `req.code_point` is
    /// closest to `req.face`'s, at most `req.limit`, each with the distance.
    /// An unknown face, or one with no descriptor of the character (it does
    /// not draw it, or it was not described yet), is `NotFound`.
    pub fn find_similar_glyph(&self, req: &SimilarGlyphRequest) -> FmResult<Vec<SimilarGlyphFace>> {
        let reference = GlyphReference {
            face: req.face,
            code_point: req.code_point,
        };
        let mut ranked: Vec<(u32, f32)> = self
            .glyph_distances(reference)?
            .into_iter()
            .filter(|(face, _)| *face != req.face.0)
            .collect();
        let closer = |a: &(u32, f32), b: &(u32, f32)| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0));
        let limit = req.limit as usize;
        if limit < ranked.len() {
            if limit == 0 {
                ranked.clear();
            } else {
                ranked.select_nth_unstable_by(limit - 1, closer);
                ranked.truncate(limit);
            }
        }
        ranked.sort_by(closer);
        ranked
            .into_iter()
            .map(|(face, distance)| {
                Ok(SimilarGlyphFace {
                    face: self.face_summary(FaceId(face))?,
                    distance,
                })
            })
            .collect()
    }
}

impl Index {
    /// The distance of every searchable face's stored glyph of
    /// `reference.code_point` to the reference face's, the reference itself
    /// included, in face-id order (E49.9: the likeness sort of a search).
    /// Faces with no descriptor of the character are not listed. An unknown
    /// reference face, or one with no descriptor of the character, is
    /// `NotFound`; a code point that is no character is `InvalidRequest`.
    pub(in crate::index) fn glyph_distances(
        &self,
        reference: GlyphReference,
    ) -> FmResult<Vec<(u32, f32)>> {
        let ch = check_code_point(reference.code_point)?;
        self.face_summary(reference.face)?;
        self.refresh_search_cache()?;
        if !self
            .search_cache
            .borrow()
            .glyphs
            .stored
            .contains_key(&reference.code_point)
        {
            let stored = self.current_glyph_descriptors(reference.code_point)?;
            let mut cache = self.search_cache.borrow_mut();
            let searchable: HashSet<u32> = cache.rows.iter().map(|r| r.id).collect();
            let stored = stored
                .into_iter()
                .map(|(face, d)| (face, searchable.contains(&face), Prepared::new(d)))
                .collect();
            cache.glyphs.stored.insert(reference.code_point, stored);
        }
        let cache = self.search_cache.borrow();
        let stored = cache
            .glyphs
            .stored
            .get(&reference.code_point)
            .ok_or_else(|| FmError::internal("glyph descriptors were not read"))?;
        let mine = stored
            .binary_search_by_key(&reference.face.0, |(face, _, _)| *face)
            .ok()
            .map(|at| &stored[at].2)
            .ok_or_else(|| {
                FmError::not_found(format!(
                    "The glyph '{ch}' (U+{:04X}) of face {}",
                    reference.code_point, reference.face
                ))
            })?;
        Ok(stored
            .iter()
            .filter(|(_, searchable, _)| *searchable)
            .map(|(face, _, d)| (*face, mine.distance(d)))
            .collect())
    }
}

#[cfg(test)]
mod tests;
