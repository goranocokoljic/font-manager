//! The faces `fonts_needed` matches (E29.2): every face the library shows,
//! with the names it is matched by, its health and whether it is active.

use fm_api::FaceId;

use super::{Index, db_error};
use crate::error::FmResult;
use crate::needed::NeededFace;

/// Every visible face (its file found, its source not paused). The legacy
/// names are the stored family and style unless the face has a typographic
/// family: then they differ, and only then are they read from `face.parsed`.
/// The health kind is read only for a face that is not healthy.
const NEEDED_FACES: &str = "
SELECT face.id, face.postscript_name, face.full_name, face.family, face.subfamily,
       face.typographic_family, face.healthy,
       CASE WHEN face.healthy = 0 AND json_valid(face.parsed)
            THEN json_extract(face.parsed, '$.health.kind') END,
       CASE WHEN face.typographic_family IS NOT NULL AND json_valid(face.parsed)
            THEN json_extract(face.parsed, '$.family') END,
       CASE WHEN face.typographic_family IS NOT NULL AND json_valid(face.parsed)
            THEN json_extract(face.parsed, '$.subfamily') END,
       EXISTS (SELECT 1 FROM activation WHERE activation.face_id = face.id)
FROM face
JOIN font_file ON font_file.id = face.file_id
JOIN source ON source.id = font_file.source_id
WHERE font_file.gone = 0 AND source.paused = 0
ORDER BY face.id";

/// One row of [`NEEDED_FACES`].
type Row = (
    u32,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
);

impl Index {
    /// The faces a needed font can name, by face id.
    pub(crate) fn needed_faces(&self) -> FmResult<Vec<NeededFace>> {
        let mut stmt = self
            .conn
            .prepare_cached(NEEDED_FACES)
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<Row>, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(rows.into_iter().map(needed_face).collect())
    }
}

fn needed_face(row: Row) -> NeededFace {
    let (
        id,
        postscript_name,
        full_name,
        family,
        subfamily,
        typographic,
        healthy,
        health_kind,
        name_family,
        name_subfamily,
        active,
    ) = row;
    let (typographic, legacy) = match typographic {
        // The stored subfamily is name ID 17, or 2 without it.
        Some(t) => (
            Some((t, subfamily)),
            name_family.map(|f| (f, name_subfamily.unwrap_or_default())),
        ),
        None => (None, Some((family, subfamily))),
    };
    NeededFace {
        id: FaceId(id),
        postscript_name,
        full_name,
        typographic,
        legacy,
        healthy,
        broken: health_kind.as_deref() == Some("broken"),
        active,
    }
}
