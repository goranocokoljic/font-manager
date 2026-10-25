//! What each command asks the API: its arguments mapped to `fm-api`
//! requests, and the answers collected. No font logic (rule 1): filtering,
//! ranking and coverage are the core's; a command only pages through a list
//! and waits for its scan.

use std::io::Write;
use std::path::Path;

use fm_api::common::Page;
use fm_api::library::{
    AddSourceRequest, FaceDetail, FaceSummary, FamilySummary, ImportMode, ListFamiliesRequest,
    ScanRequest, Source,
};
use fm_api::search::{
    CoverageReport, CoverageRequest, FamilyHit, Filters, SearchRequest, SimilarFace,
    SimilarRequest, Supports,
};
use fm_api::{Api, ApiError, ErrorCode, Event};
use serde::Serialize;

use crate::output::Failure;
use crate::resolve;

/// Rows asked for per page when a command lists everything.
const PAGE: u32 = 500;

/// What a scan of one source found: its `ScanFinished` counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scanned {
    /// The source scanned.
    pub source: Source,
    /// Faces added.
    pub added: u32,
    /// Faces whose file changed and was read again.
    pub changed: u32,
    /// Faces removed.
    pub removed: u32,
    /// Problems found.
    pub problems: u32,
}

/// `fm scan [path]`: adds `path` as a source when given (a folder that is one
/// already is scanned again), then scans it, or every source that is not
/// paused. Progress goes to `progress` while each scan runs.
pub fn scan(
    api: &dyn Api,
    path: Option<&Path>,
    mut progress: Option<&mut dyn Write>,
) -> Result<Vec<Scanned>, Failure> {
    let sources = match path {
        Some(path) => vec![add_source(api, path)?],
        None => api
            .list_sources()?
            .into_iter()
            .filter(|s| !s.paused)
            .collect(),
    };
    let mut scanned = Vec::with_capacity(sources.len());
    for source in sources {
        let finished = scan_one(api, &source, &mut progress)?;
        scanned.push(finished);
    }
    Ok(scanned)
}

/// The source for `path`, added as one when it is not one yet.
fn add_source(api: &dyn Api, path: &Path) -> Result<Source, Failure> {
    let path = std::path::absolute(path)
        .map_err(|e| {
            ApiError::new(ErrorCode::Io, "The folder could not be found.")
                .with_detail(e.to_string())
        })?
        .display()
        .to_string();
    match api.add_source(AddSourceRequest {
        path: path.clone(),
        mode: ImportMode::IndexInPlace,
    }) {
        Err(e) if e.code == ErrorCode::Conflict => api
            .list_sources()?
            .into_iter()
            .find(|s| s.path == path)
            .ok_or_else(|| e.into()),
        added => Ok(added?),
    }
}

/// Scans `source` and waits for its `ScanFinished` (or `JobFailed`).
fn scan_one(
    api: &dyn Api,
    source: &Source,
    progress: &mut Option<&mut dyn Write>,
) -> Result<Scanned, Failure> {
    // Subscribed first: the scan's events may come before `scan` returns.
    let events = api.subscribe();
    let job = api.scan(ScanRequest {
        source: Some(source.id),
    })?;
    loop {
        let event = events.recv().map_err(|_| {
            ApiError::new(ErrorCode::Internal, "The scan stopped without finishing.")
        })?;
        match event {
            Event::ScanProgress {
                job: j,
                done,
                total,
            } if j == job => {
                if let Some(out) = progress.as_mut() {
                    // Progress that cannot be written does not stop the scan.
                    let _ = writeln!(out, "Scanning {}: {done} of {total} files", source.path);
                }
            }
            Event::ScanFinished {
                job: j,
                added,
                changed,
                removed,
                problems,
            } if j == job => {
                return Ok(Scanned {
                    source: source.clone(),
                    added,
                    changed,
                    removed,
                    problems,
                });
            }
            Event::JobFailed { job: j, error } if j == job => return Err(error.into()),
            _ => {}
        }
    }
}

/// `fm list`: every family, A to Z.
pub fn families(api: &dyn Api) -> Result<Vec<FamilySummary>, Failure> {
    let mut rows = Vec::new();
    loop {
        let page = api.list_families(ListFamiliesRequest {
            page: Page {
                offset: rows.len() as u32,
                limit: PAGE,
            },
            ..ListFamiliesRequest::default()
        })?;
        let last = page.items.is_empty();
        rows.extend(page.items);
        if last || rows.len() as u32 >= page.total {
            return Ok(rows);
        }
    }
}

/// `fm list --family <font>`: the faces of the families a font argument
/// names, as `resolve_font` matches it (a family name names every style).
pub fn faces(api: &dyn Api, font: &str) -> Result<Vec<FaceSummary>, Failure> {
    let matches = api.resolve_font(fm_api::library::ResolveFontRequest { font: font.into() })?;
    let mut families = Vec::new();
    for m in &matches.faces {
        let family = api.get_face(m.face)?.face.family;
        if !families.contains(&family) {
            families.push(family);
        }
    }
    let mut faces = Vec::new();
    for family in families {
        faces.extend(api.get_family(family)?.faces);
    }
    Ok(faces)
}

/// `fm search` flags as `SearchRequest` filters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFlags {
    /// `--supports`: language preset codes, every one required.
    pub supports: Vec<String>,
    /// `--feature`: OpenType feature tags, every one required.
    pub features: Vec<String>,
    /// `--variable`: variable fonts only.
    pub variable: bool,
}

impl SearchFlags {
    /// The filters these flags ask for.
    pub fn filters(&self) -> Filters {
        Filters {
            supports: (!self.supports.is_empty()).then(|| Supports::Languages {
                codes: self.supports.clone(),
            }),
            features: self.features.clone(),
            variable: self.variable.then_some(true),
            ..Filters::default()
        }
    }
}

/// `fm search`: every family hit of `query` with the flags' filters.
pub fn search(api: &dyn Api, query: &str, flags: &SearchFlags) -> Result<Vec<FamilyHit>, Failure> {
    let mut hits = Vec::new();
    loop {
        let page = api.search(SearchRequest {
            query: query.into(),
            filters: flags.filters(),
            rule: None,
            page: Page {
                offset: hits.len() as u32,
                limit: PAGE,
            },
        })?;
        let last = page.families.is_empty();
        hits.extend(page.families);
        if last || hits.len() as u32 >= page.total {
            return Ok(hits);
        }
    }
}

/// `fm info <font>`: the face's detail.
pub fn info(api: &dyn Api, font: &str) -> Result<FaceDetail, Failure> {
    let face = resolve(api, font)?.face;
    Ok(api.get_face(face)?)
}

/// `fm coverage <font>`: how the face covers `target`.
pub fn coverage(api: &dyn Api, font: &str, target: Supports) -> Result<CoverageReport, Failure> {
    let face = resolve(api, font)?.face;
    Ok(api.check_coverage(CoverageRequest { face, target })?)
}

/// `fm similar <font>`: the faces that look most like it.
pub fn similar(api: &dyn Api, font: &str, limit: u32) -> Result<Vec<SimilarFace>, Failure> {
    let face = resolve(api, font)?.face;
    Ok(api.find_similar(SimilarRequest { face, limit })?)
}
