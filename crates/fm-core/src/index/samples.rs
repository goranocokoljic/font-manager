//! Dynamic previews in the index (E4.5): the text each face previews for a
//! sample, from the `cmap` stored with the face (`face.cmap`, E3.7); and the
//! auto size factor of each face (E16.2), from its stored `face.metrics`.

use fm_api::FaceId;
use fm_api::common::Metrics;
use fm_api::preview::{SampleText, SampleTextRequest, SizeFactor, SizeFactorRequest};
use rusqlite::OptionalExtension;

use super::{Index, db_error};
use crate::coverage::CodePointSet;
use crate::error::{FmError, FmResult};
use crate::{metrics, sample};

impl Index {
    /// The stored `cmap` blob of the face `face`; `None` when the face was
    /// indexed before E3.7 stored it.
    fn cmap_blob(&self, face: FaceId) -> FmResult<Option<Vec<u8>>> {
        let blob: Option<Option<Vec<u8>>> = self
            .conn
            .query_row("SELECT cmap FROM face WHERE id = ?1", [face.0], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| db_error(&e))?;
        blob.ok_or_else(|| FmError::not_found(format!("Face {face}")))
    }

    /// The stored `cmap` of the face `face`; empty when the face was indexed
    /// before E3.7 stored it.
    pub fn face_cmap(&self, face: FaceId) -> FmResult<CodePointSet> {
        match self.cmap_blob(face)? {
            None => Ok(CodePointSet::default()),
            Some(bytes) => CodePointSet::from_bytes(&bytes)
                .ok_or_else(|| FmError::internal(format!("face {face}: stored cmap is damaged"))),
        }
    }

    /// The text each face of `req` previews for its sample text, in request
    /// order (see [`sample::dynamic_sample`]). A face whose stored `cmap`
    /// cannot be read is not judged (it previews the sample), like a face
    /// with no `cmap`; the other faces of the request are answered as usual.
    /// An unknown face or a database error fails the request.
    pub fn sample_texts(&self, req: &SampleTextRequest) -> FmResult<Vec<SampleText>> {
        req.faces
            .iter()
            .map(|&face| {
                let set = match self.cmap_blob(face)? {
                    None => CodePointSet::default(),
                    Some(bytes) => CodePointSet::from_bytes(&bytes).unwrap_or_else(|| {
                        tracing::warn!(%face, "stored cmap is damaged; previewing the sample");
                        CodePointSet::default()
                    }),
                };
                let got = sample::dynamic_sample(&set, &req.text);
                Ok(SampleText {
                    face,
                    text: got.text,
                    fallback: got.fallback,
                    missing: got.missing.into_iter().map(String::from).collect(),
                })
            })
            .collect()
    }

    /// The auto size factor of each face of `req`, in request order (see
    /// [`metrics::size_factor`]). A face whose `face.metrics` is missing or
    /// unreadable keeps factor 1; an unknown face or a database error fails
    /// the request.
    pub fn size_factors(&self, req: &SizeFactorRequest) -> FmResult<Vec<SizeFactor>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT metrics FROM face WHERE id = ?1")
            .map_err(|e| db_error(&e))?;
        req.faces
            .iter()
            .map(|&face| {
                let json: Option<String> = stmt
                    .query_row([face.0], |row| row.get(0))
                    .optional()
                    .map_err(|e| db_error(&e))?
                    .ok_or_else(|| FmError::not_found(format!("Face {face}")))?;
                let stored = json.and_then(|j| serde_json::from_str::<Metrics>(&j).ok());
                let factor = stored.map_or(1.0, |m| metrics::size_factor(&m, req.basis));
                Ok(SizeFactor { face, factor })
            })
            .collect()
    }
}
