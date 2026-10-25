//! Similar-font search (E23.2): [`Index::find_similar`].
//!
//! The candidates are the searchable faces (files that are not gone, sources
//! that are not paused), less the face asked about and every face of its
//! family as search groups it. Each is scored with
//! [`crate::similar::compare`] and the best `limit` come back, closest
//! first; equal scores keep face-id order, so a ranking never flickers.
//!
//! The profiles are read once per database change, with the search rows
//! ([`super::SearchCache`]): a search after that is a pass over memory.

use std::collections::HashMap;

use fm_api::FaceId;
use fm_api::common::{Classification, Metrics};
use fm_api::search::{SimilarFace, SimilarRequest};

use super::{Index, db_error, name_of};
use crate::error::{FmError, FmResult};
use crate::fingerprint::{self, Fingerprint};
use crate::grouping::family_key;
use crate::similar::{self, Profile, Similarity};

/// What a profile needs of every face.
const PROFILES: &str = "
SELECT face.id, face.weight, face.width, face.metrics,
       CASE WHEN json_valid(face.parsed)
            THEN json_extract(face.parsed, '$.classification.class') END,
       fp.vector, fp.x_height_ratio, fp.contrast, fp.stroke_width, fp.width_class,
       fp.advance_ratio
FROM face
LEFT JOIN face_fingerprint AS fp ON fp.face_id = face.id";

impl Index {
    /// The visible faces that look most like `req.face`, closest first, at
    /// most `req.limit`; never the face itself or a face of its family. An
    /// unknown face is `NotFound`.
    pub fn find_similar(&self, req: &SimilarRequest) -> FmResult<Vec<SimilarFace>> {
        let me = self.face_summary(req.face)?;
        let mine = family_key(&me.family_name);
        self.refresh_search_cache()?;
        if self.search_cache.borrow().profiles.is_none() {
            let profiles = self.profiles()?;
            self.search_cache.borrow_mut().profiles = Some(profiles);
        }
        let ranked: Vec<(u32, Similarity)> = {
            let cache = self.search_cache.borrow();
            let profiles = cache
                .profiles
                .as_ref()
                .ok_or_else(|| FmError::internal("similar-font profiles were not read"))?;
            let target = profiles
                .get(&req.face.0)
                .ok_or_else(|| FmError::not_found(format!("Face {}", req.face)))?;
            let mut ranked: Vec<(u32, Similarity)> = cache
                .rows
                .iter()
                .filter(|r| r.id != req.face.0 && r.family_key != mine)
                .filter_map(|r| Some((r.id, similar::compare(target, profiles.get(&r.id)?))))
                .collect();
            ranked.sort_by(|a, b| b.1.score.total_cmp(&a.1.score).then(a.0.cmp(&b.0)));
            ranked.truncate(req.limit as usize);
            ranked
        };
        ranked
            .into_iter()
            .map(|(id, found)| {
                Ok(SimilarFace {
                    face: self.face_summary(FaceId(id))?,
                    score: found.score,
                    reasons: found.reasons.into_iter().map(str::to_owned).collect(),
                })
            })
            .collect()
    }

    /// Every face's profile by id.
    fn profiles(&self) -> FmResult<HashMap<u32, Profile>> {
        let mut stmt = self
            .conn
            .prepare_cached(PROFILES)
            .map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out = HashMap::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let e = |e: rusqlite::Error| db_error(&e);
            let id: u32 = row.get(0).map_err(e)?;
            let metrics: Option<Metrics> = row
                .get::<_, Option<String>>(3)
                .map_err(e)?
                .and_then(|json| serde_json::from_str(&json).ok());
            let classification: Option<Classification> = row
                .get::<_, Option<String>>(4)
                .map_err(e)?
                .and_then(|c| name_of(&c));
            let fingerprint = match row.get::<_, Option<Vec<u8>>>(5).map_err(e)? {
                Some(bytes) => fingerprint::vector_from_bytes(&bytes).and_then(|vector| {
                    let measure = |i: usize| row.get::<_, Option<f64>>(i).ok().flatten();
                    Some(Fingerprint {
                        vector,
                        x_height_ratio: measure(6)? as f32,
                        contrast: measure(7)? as f32,
                        stroke_width: measure(8)? as f32,
                        width_class: measure(9)? as f32,
                        advance_ratio: measure(10)? as f32,
                    })
                }),
                None => None,
            };
            let weight: u16 = row.get(1).map_err(e)?;
            let width: u16 = row.get(2).map_err(e)?;
            out.insert(
                id,
                Profile::new(classification, weight, width, metrics.as_ref(), fingerprint),
            );
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
