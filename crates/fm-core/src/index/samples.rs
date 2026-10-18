//! Dynamic previews in the index (E4.5): the text each face previews for a
//! sample, from the `cmap` stored with the face (`face.cmap`, E3.7).

use fm_api::FaceId;
use fm_api::preview::{SampleText, SampleTextRequest};
use rusqlite::OptionalExtension;

use super::{Index, db_error};
use crate::coverage::CodePointSet;
use crate::error::{FmError, FmResult};
use crate::sample;

impl Index {
    /// The stored `cmap` of the face `face`; empty when the face was indexed
    /// before E3.7 stored it.
    pub fn face_cmap(&self, face: FaceId) -> FmResult<CodePointSet> {
        let blob: Option<Option<Vec<u8>>> = self
            .conn
            .query_row("SELECT cmap FROM face WHERE id = ?1", [face.0], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| db_error(&e))?;
        let Some(blob) = blob else {
            return Err(FmError::not_found(format!("Face {face}")));
        };
        match blob {
            None => Ok(CodePointSet::default()),
            Some(bytes) => CodePointSet::from_bytes(&bytes)
                .ok_or_else(|| FmError::internal(format!("face {face}: stored cmap is damaged"))),
        }
    }

    /// The text each face of `req` previews for its sample text, in request
    /// order (see [`sample::dynamic_sample`]).
    pub fn sample_texts(&self, req: &SampleTextRequest) -> FmResult<Vec<SampleText>> {
        req.faces
            .iter()
            .map(|&face| {
                let got = sample::dynamic_sample(&self.face_cmap(face)?, &req.text);
                Ok(SampleText {
                    face,
                    text: got.text,
                    fallback: got.fallback,
                    missing: got.missing.into_iter().map(String::from).collect(),
                })
            })
            .collect()
    }
}
