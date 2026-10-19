//! The Problems view (E9.4): what is wrong with the library, as entries, never
//! as a crash.
//!
//! - files whose health is not `Ok` (truncated, missing tables, a format the
//!   app cannot activate), among the files the library shows;
//! - source folders that are not there or cannot be opened;
//! - an index file that was damaged and replaced when it was opened
//!   ([`Index::open`]), for the session that replaced it;
//! - sidecar metadata that could not be merged ([`Index::sidecar_problems`]).

use std::fs;
use std::io;
use std::path::Path;

use fm_api::FaceId;
use fm_api::common::Health;
use fm_api::library::{Problem, ProblemKind, Source};

use super::files::path_from_sql;
use super::{Index, db_error};
use crate::error::FmResult;

/// Files the library shows whose health is not `Ok`, with their first face.
const UNHEALTHY: &str = "
SELECT f.path, f.health, MIN(face.id)
FROM font_file AS f
JOIN source AS s ON s.id = f.source_id
LEFT JOIN face ON face.file_id = f.id
WHERE f.gone = 0 AND s.paused = 0
  AND (NOT json_valid(f.health) OR json_extract(f.health, '$.kind') <> 'ok')
GROUP BY f.id
ORDER BY f.id";

impl Index {
    /// Every Problems entry: a replaced index first, then missing source
    /// folders, unhealthy files and sidecar conflicts. Only a database error
    /// fails; a damaged file or a folder that is gone is an entry.
    pub fn problems(&self) -> FmResult<Vec<Problem>> {
        let mut out = Vec::new();
        if let Some(rebuilt) = &self.rebuilt {
            let path = self
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            out.push(Problem {
                kind: ProblemKind::IndexRebuilt,
                face: None,
                path,
                other: rebuilt.moved_to.as_ref().map(|p| p.display().to_string()),
                message: "The library index was damaged and has been replaced. \
                          Your folders are being scanned again."
                    .to_owned(),
            });
        }
        out.extend(self.sources()?.iter().filter_map(missing_source));
        out.extend(self.unhealthy_files()?);
        out.extend(self.sidecar_problems()?);
        Ok(out)
    }

    fn unhealthy_files(&self) -> FmResult<Vec<Problem>> {
        let mut stmt = self.conn.prepare(UNHEALTHY).map_err(|e| db_error(&e))?;
        let mut rows = stmt.query([]).map_err(|e| db_error(&e))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| db_error(&e))? {
            let path = path_from_sql(row.get_ref(0).map_err(|e| db_error(&e))?)?;
            let health: String = row.get(1).map_err(|e| db_error(&e))?;
            let face: Option<u32> = row.get(2).map_err(|e| db_error(&e))?;
            let (kind, message) = match serde_json::from_str::<Health>(&health) {
                Ok(Health::Unsupported(reason)) => (ProblemKind::Unsupported, reason),
                Ok(Health::Broken(reason)) => (ProblemKind::Broken, reason),
                // Filtered out by the query; kept so the match is total.
                Ok(Health::Ok) => continue,
                Err(_) => (
                    ProblemKind::Broken,
                    "The file has not been read correctly yet.".to_owned(),
                ),
            };
            out.push(Problem {
                kind,
                face: face.map(FaceId),
                path: path.display().to_string(),
                other: None,
                message,
            });
        }
        Ok(out)
    }
}

/// An entry for a source whose folder is not there or cannot be opened.
fn missing_source(source: &Source) -> Option<Problem> {
    let message = match fs::metadata(Path::new(&source.path)) {
        Ok(meta) if meta.is_dir() => return None,
        Ok(_) => "The source is not a folder any more.",
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            "The source folder cannot be found. It may have been moved, renamed or be on a drive that is not connected."
        }
        Err(_) => "The source folder cannot be opened.",
    };
    Some(Problem {
        kind: ProblemKind::MissingSource,
        face: None,
        path: source.path.clone(),
        other: None,
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests;
