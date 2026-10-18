//! Per-language coverage in the index (E3.7): the `face.languages` column.
//!
//! The scan writes a face's coverage with the face ([`to_sql`]). It depends
//! on the language presets of `data/charsets`, which ship with the app, so
//! `index_meta.charsets` records the fingerprint of the data it was computed
//! with. When the app opens an index computed with other data,
//! [`Index::refresh_languages`] computes every face's coverage again from
//! what the index already holds (`face.cmap`, the `locl` languages in
//! `face.parsed`), without reading a font file.

use std::collections::BTreeMap;

use fm_api::FaceId;
use rusqlite::{OptionalExtension, params};

use super::{Index, db_error};
use crate::charsets;
use crate::coverage::{self, CodePointSet, FaceLanguage};
use crate::error::{FmError, FmResult};

/// The `index_meta` key of the preset-data fingerprint.
const CHARSETS_KEY: &str = "charsets";

/// `rows` as stored: `{"code": [covered, total, has_locl as 0 or 1]}`.
pub(super) fn to_sql(rows: &[FaceLanguage]) -> String {
    let map: BTreeMap<&str, [u32; 3]> = rows
        .iter()
        .map(|l| (l.code.as_str(), [l.covered, l.total, u32::from(l.has_locl)]))
        .collect();
    // A map of strings to integers always serializes.
    serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_owned())
}

/// The rows of [`to_sql`], by code; `None` for text it cannot have written.
fn from_sql(text: &str) -> Option<Vec<FaceLanguage>> {
    let map: BTreeMap<String, [u32; 3]> = serde_json::from_str(text).ok()?;
    map.into_iter()
        .map(|(code, [covered, total, locl])| {
            (covered <= total && locl <= 1).then_some(FaceLanguage {
                code,
                covered,
                total,
                has_locl: locl == 1,
            })
        })
        .collect()
}

impl Index {
    /// The stored coverage of the face `face`, by preset code. A preset the
    /// face covers nothing of (and has no `locl` for) is not listed; a face
    /// not computed yet (indexed by an older version) lists nothing.
    pub fn face_languages(&self, face: FaceId) -> FmResult<Vec<FaceLanguage>> {
        let text: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT languages FROM face WHERE id = ?1",
                [face.0],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        let Some(text) = text else {
            return Err(FmError::not_found(format!("Face {face}")));
        };
        match text {
            None => Ok(Vec::new()),
            Some(text) => from_sql(&text).ok_or_else(|| {
                FmError::internal(format!("face {face}: stored language coverage is damaged"))
            }),
        }
    }

    /// Computes every face's coverage again when the preset data differs
    /// from the data it was computed with; returns the number of faces
    /// computed (0 when the data is the same).
    ///
    /// A face whose `cmap` or `face.parsed` cannot be read is set to `NULL`
    /// (not computed); the next scan reads its file again (see
    /// `Index::load_files`) and writes it.
    pub(crate) fn refresh_languages(&mut self) -> FmResult<usize> {
        let current = charsets::fingerprint();
        let stored: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = ?1",
                [CHARSETS_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        if stored.as_deref() == Some(current.as_str()) {
            return Ok(0);
        }
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        let faces = {
            let mut stmt = tx
                .prepare("SELECT id, cmap, parsed FROM face")
                .map_err(|e| db_error(&e))?;
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|e| db_error(&e))?
        };
        let mut computed = 0;
        {
            let mut update = tx
                .prepare("UPDATE face SET languages = ?2 WHERE id = ?1")
                .map_err(|e| db_error(&e))?;
            for (id, cmap, parsed) in faces {
                let set = cmap.as_deref().and_then(CodePointSet::from_bytes);
                let locl = parsed.as_deref().and_then(locl_languages);
                let languages = set.zip(locl).map(|(set, locl)| {
                    computed += 1;
                    to_sql(&coverage::languages(&set, &locl))
                });
                update
                    .execute(params![id, languages])
                    .map_err(|e| db_error(&e))?;
            }
        }
        tx.execute(
            "INSERT INTO index_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![CHARSETS_KEY, current],
        )
        .map_err(|e| db_error(&e))?;
        tx.commit().map_err(|e| db_error(&e))?;
        if computed > 0 {
            tracing::info!(
                faces = computed,
                "language coverage computed for new preset data"
            );
        }
        Ok(computed)
    }
}

/// `loclLanguages` of a `face.parsed` JSON; `None` when it has none.
fn locl_languages(parsed: &str) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(parsed).ok()?;
    serde_json::from_value(value.get("loclLanguages")?.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_round_trips_and_damage_is_refused() {
        let rows = vec![
            FaceLanguage {
                code: "en".into(),
                covered: 52,
                total: 52,
                has_locl: false,
            },
            FaceLanguage {
                code: "ro".into(),
                covered: 58,
                total: 60,
                has_locl: true,
            },
        ];
        let text = to_sql(&rows);
        assert_eq!(text, r#"{"en":[52,52,0],"ro":[58,60,1]}"#);
        assert_eq!(from_sql(&text), Some(rows));
        assert_eq!(from_sql("{}"), Some(Vec::new()));
        for bad in [
            "",
            "[]",
            r#"{"en":[53,52,0]}"#,
            r#"{"en":[1,2,2]}"#,
            r#"{"en":[1,2]}"#,
        ] {
            assert_eq!(from_sql(bad), None, "{bad}");
        }
    }

    #[test]
    fn locl_comes_from_the_parsed_face() {
        assert_eq!(
            locl_languages(r#"{"loclLanguages": ["latn/ROM"]}"#),
            Some(vec!["latn/ROM".to_owned()])
        );
        assert_eq!(locl_languages(r#"{"features": []}"#), None);
        assert_eq!(locl_languages("not json"), None);
    }
}
