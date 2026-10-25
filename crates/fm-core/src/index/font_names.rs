//! The names a `<font>` argument is looked up by (E26.1): every face the
//! library shows with its PostScript name, full name and family and style
//! pairs, for [`fm_api::library::ResolveFontRequest::resolve`].

use fm_api::FaceId;
use fm_api::library::{FontMatch, FontNames};

use super::{Index, db_error};
use crate::error::FmResult;

/// Every visible face's names: the stored family and style, the PostScript
/// name, and from `face.parsed` the full name and the name ID 1/2 and 16/17
/// pairs (a stored family can come from the file name when the font has
/// none).
const FONT_NAMES: &str = "
SELECT face.id, face.family, face.subfamily, face.postscript_name, face.typographic_family,
       CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.fullName') END,
       CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.family') END,
       CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.subfamily') END,
       CASE WHEN json_valid(face.parsed)
            THEN json_extract(face.parsed, '$.typographicSubfamily') END
FROM face
JOIN font_file ON font_file.id = face.file_id
JOIN source ON source.id = font_file.source_id
WHERE font_file.gone = 0 AND source.paused = 0
ORDER BY face.id";

/// One row of [`FONT_NAMES`].
type NameRow = (
    u32,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

impl Index {
    /// The names of every face the library shows, by face id.
    pub fn font_names(&self) -> FmResult<Vec<FontNames>> {
        let mut stmt = self
            .conn
            .prepare_cached(FONT_NAMES)
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
                ))
            })
            .map_err(|e| db_error(&e))?
            .collect::<Result<Vec<NameRow>, _>>()
            .map_err(|e| db_error(&e))?;
        Ok(rows.into_iter().map(names).collect())
    }
}

/// A row as the names it holds; the typographic family goes with its own
/// style, or the stored one when the font has none.
fn names(row: NameRow) -> FontNames {
    let (
        id,
        family,
        style,
        postscript_name,
        typographic,
        full_name,
        name_family,
        name_style,
        typographic_style,
    ) = row;
    let mut families = vec![(family.clone(), style.clone())];
    for (f, s) in [(name_family, name_style), (typographic, typographic_style)] {
        if let Some(f) = f {
            let pair = (f, s.unwrap_or_else(|| style.clone()));
            if !families.contains(&pair) {
                families.push(pair);
            }
        }
    }
    FontNames {
        face: FontMatch {
            face: FaceId(id),
            family,
            style,
            postscript_name,
            full_name,
        },
        families,
    }
}
