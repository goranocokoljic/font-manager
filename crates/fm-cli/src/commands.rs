//! What each command asks the API: its arguments mapped to `fm-api`
//! requests, and the answers collected. No font logic (rule 1): filtering,
//! ranking and coverage are the core's; a command only pages through a list
//! and waits for its scan.

use std::io::Write;
use std::path::{Path, PathBuf};

use fm_api::activation::{
    ActivateRequest, ActivationMode, ActivationOutcome, ConflictPolicy, DeactivateRequest,
    OutcomeStatus, Target,
};
use fm_api::common::Page;
use fm_api::export::{
    ExportWebRequest, ExportWebResponse, SpecimenFormat, SpecimenRequest, SpecimenResponse,
    WebFormat,
};
use fm_api::library::{
    AddSourceRequest, FaceDetail, FaceSummary, FamilySummary, FontMatchKind, ImportMode,
    ListFamiliesRequest, ResolveFontRequest, ScanRequest, Source,
};
use fm_api::organization::{ImportSetResponse, ProjectSet, SetExport};
use fm_api::preview::PreviewRequest;
use fm_api::search::{
    CoverageReport, CoverageRequest, FamilyHit, Filters, SearchRequest, SimilarFace,
    SimilarRequest, Supports,
};
use fm_api::{Api, ApiError, CollectionId, ErrorCode, Event, FaceId, FamilyId};
use serde::Serialize;

use crate::output::Failure;
use crate::{Exit, resolve};

/// Rows asked for per page when a command lists everything.
const PAGE: u32 = 500;

/// What a scan of one source found: its `ScanFinished` counts, or why it
/// failed.
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
    /// The scan failed (the folder is gone, say); the counts are 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl Scanned {
    /// The exit code of a run of scans: that of the first failed one, else 0.
    pub fn exit(scans: &[Self]) -> Exit {
        scans
            .iter()
            .find_map(|s| s.error.as_ref())
            .map_or(Exit::Success, |e| Exit::of(e.code))
    }
}

/// `fm scan [path]`: adds `path` as a source when given (a folder that is one
/// already is scanned again), then scans it, or every source that is not
/// paused. A source whose scan fails does not stop the others, as in the
/// core's own scan of every source. Progress goes to `progress`.
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
    Ok(sources
        .into_iter()
        .map(|source| {
            scan_one(api, &source, &mut progress).unwrap_or_else(|error| Scanned {
                source,
                added: 0,
                changed: 0,
                removed: 0,
                problems: 0,
                error: Some(error),
            })
        })
        .collect())
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
) -> Result<Scanned, ApiError> {
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
                    error: None,
                });
            }
            Event::JobFailed { job: j, error } if j == job => return Err(error),
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
    let mut faces = Vec::new();
    for family in families_of(api, font)? {
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

/// What the argument of `fm activate` and `fm deactivate` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetKind {
    /// A font argument (the default).
    #[default]
    Font,
    /// A collection: its id or its name.
    Collection,
    /// A project set: its id or its name.
    Set,
}

/// The `Target` an argument names. A font argument that matches by family
/// name targets every face it matches (a family name names every style);
/// any other font match must be one face, as for `fm info`.
pub fn target(api: &dyn Api, kind: TargetKind, arg: &str) -> Result<Target, Failure> {
    match kind {
        TargetKind::Font => {
            let matches = api.resolve_font(ResolveFontRequest { font: arg.into() })?;
            if matches.matched_by == FontMatchKind::FamilyStyle {
                let faces = matches.faces.iter().map(|m| m.face).collect();
                return Ok(Target::Faces { faces });
            }
            let candidates = matches.faces.clone();
            let face = matches
                .single(arg)
                .map_err(|error| Failure::Ambiguous { error, candidates })?
                .face;
            Ok(Target::Faces { faces: vec![face] })
        }
        TargetKind::Collection => {
            let collections = api.list_collections()?;
            let id = named(
                arg,
                "collection",
                collections.iter().map(|c| (c.id.0, c.name.as_str())),
            )?;
            Ok(Target::Collection {
                collection: CollectionId(id),
            })
        }
        TargetKind::Set => Ok(Target::Set {
            set: find_set(api, arg)?.id,
        }),
    }
}

/// The project set `arg` names: its id or its name.
fn find_set(api: &dyn Api, arg: &str) -> Result<ProjectSet, Failure> {
    let sets = api.list_sets()?;
    let id = named(arg, "set", sets.iter().map(|s| (s.id.0, s.name.as_str())))?;
    sets.into_iter()
        .find(|s| s.id.0 == id)
        .ok_or_else(|| not_called(arg, "set").into())
}

/// The id of the item `arg` names among `items` (id, name): the one whose
/// id it is, else the one whose name equals it, ignoring case and
/// surrounding space. Several of that name is `InvalidRequest` listing
/// their ids, as an ambiguous font argument is.
fn named<'a>(
    arg: &str,
    noun: &str,
    items: impl Iterator<Item = (u32, &'a str)> + Clone,
) -> Result<u32, ApiError> {
    let wanted = arg.trim().to_lowercase();
    if wanted.is_empty() {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!("Name a {noun}: its id or its name."),
        ));
    }
    if let Ok(id) = wanted.parse::<u32>()
        && items.clone().any(|(i, _)| i == id)
    {
        return Ok(id);
    }
    let hits: Vec<(u32, &str)> = items
        .filter(|(_, name)| name.trim().to_lowercase() == wanted)
        .collect();
    match hits.as_slice() {
        [] => Err(not_called(arg, noun)),
        [(id, _)] => Ok(*id),
        _ => {
            let lines: Vec<String> = hits.iter().map(|(id, n)| format!("{id}\t{n}")).collect();
            Err(ApiError::new(
                ErrorCode::InvalidRequest,
                format!(
                    "\"{arg}\" matches {} {noun}s. Use one of their ids instead.",
                    hits.len()
                ),
            )
            .with_detail(lines.join("\n")))
        }
    }
}

/// No `noun` in the library is called `arg`.
fn not_called(arg: &str, noun: &str) -> ApiError {
    ApiError::new(
        ErrorCode::NotFound,
        format!("No {noun} in the library is called \"{arg}\"."),
    )
}

/// `fm activate`: activates what `arg` names, for the session or permanently.
/// A clash is reported, never replaced: the command line cannot ask.
pub fn activate(
    api: &dyn Api,
    kind: TargetKind,
    arg: &str,
    permanent: bool,
) -> Result<Vec<ActivationOutcome>, Failure> {
    let target = target(api, kind, arg)?;
    let mode = if permanent {
        ActivationMode::Permanent
    } else {
        ActivationMode::Session
    };
    Ok(api.activate(ActivateRequest {
        target,
        mode,
        on_conflict: ConflictPolicy::KeepExisting,
    })?)
}

/// `fm deactivate`: deactivates what `arg` names. The API leaves a face the
/// app did not register alone and reports it `Skipped` (rule 3).
pub fn deactivate(
    api: &dyn Api,
    kind: TargetKind,
    arg: &str,
) -> Result<Vec<ActivationOutcome>, Failure> {
    let target = target(api, kind, arg)?;
    Ok(api.deactivate(DeactivateRequest {
        target: Some(target),
        except: None,
    })?)
}

/// The exit code of an activation or deactivation: 0 when every face is
/// done or already was, 4 when any was refused (a conflict, skipped) or
/// failed.
pub fn activation_exit(outcomes: &[ActivationOutcome]) -> Exit {
    let refused = outcomes
        .iter()
        .any(|o| !matches!(o.outcome, OutcomeStatus::Done | OutcomeStatus::Unchanged));
    if refused {
        Exit::Refused
    } else {
        Exit::Success
    }
}

/// Why `fm activate` and `fm deactivate` do nothing in direct-link mode: a
/// session activation would end with the `fm` process.
pub fn needs_the_app() -> Failure {
    ApiError::new(
        ErrorCode::Unsupported,
        "Font Manager must be running to activate fonts; start it and try again",
    )
    .into()
}

/// `fm sets export <set>`: the set as portable JSON, also written to `out`
/// when given (a file the user named).
pub fn export_set(api: &dyn Api, set: &str, out: Option<&Path>) -> Result<SetExport, Failure> {
    let set = find_set(api, set)?;
    let export = api.export_set(set.id)?;
    if let Some(path) = out {
        let json = serde_json::to_string_pretty(&export).map_err(|e| {
            ApiError::new(ErrorCode::Internal, "The set could not be written as JSON.")
                .with_detail(e.to_string())
        })?;
        std::fs::write(path, json + "\n").map_err(|e| {
            ApiError::new(
                ErrorCode::Io,
                format!("The set could not be written to {}.", path.display()),
            )
            .with_detail(e.to_string())
        })?;
    }
    Ok(export)
}

/// `fm sets import <file>`: reads a set export and imports it as a new set.
pub fn import_set(api: &dyn Api, file: &Path) -> Result<ImportSetResponse, Failure> {
    let text = std::fs::read_to_string(file).map_err(|e| {
        let code = if e.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::Io
        };
        ApiError::new(code, format!("{} could not be read.", file.display()))
            .with_detail(e.to_string())
    })?;
    let export: SetExport = serde_json::from_str(&text).map_err(|e| {
        ApiError::new(
            ErrorCode::InvalidRequest,
            format!("{} is not a set exported by Font Manager.", file.display()),
        )
        .with_detail(e.to_string())
    })?;
    Ok(api.import_set(export)?)
}

/// The families a font argument names, as `resolve_font` matches it, in
/// match order: a family name names its family, a face names its own.
fn families_of(api: &dyn Api, font: &str) -> Result<Vec<FamilyId>, Failure> {
    let matches = api.resolve_font(ResolveFontRequest { font: font.into() })?;
    let mut families = Vec::new();
    for m in &matches.faces {
        let family = api.get_face(m.face)?.face.family;
        if !families.contains(&family) {
            families.push(family);
        }
    }
    Ok(families)
}

/// The one family a font argument names; several is `InvalidRequest` with
/// the faces it matched (exit 2), as an ambiguous `<font>` is.
fn one_family(api: &dyn Api, font: &str) -> Result<FamilyId, Failure> {
    let families = families_of(api, font)?;
    match families.as_slice() {
        [family] => Ok(*family),
        _ => {
            let candidates = api
                .resolve_font(ResolveFontRequest { font: font.into() })?
                .faces;
            Err(Failure::Ambiguous {
                error: ApiError::new(
                    ErrorCode::InvalidRequest,
                    format!(
                        "\"{font}\" matches {} families. Name one family.",
                        families.len()
                    ),
                ),
                candidates,
            })
        }
    }
}

/// The path the user gave, as the API takes it. A path that is not valid
/// Unicode is refused: a lossy copy of it would be a path the user did not
/// give.
fn user_path(path: &Path) -> Result<String, ApiError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        ApiError::new(
            ErrorCode::InvalidRequest,
            "The output path is not valid Unicode. Choose another path.",
        )
        .with_detail(path.to_string_lossy().into_owned())
    })
}

/// `InvalidRequest` when `path` exists and `force` is off: `fm` replaces a
/// file only when asked to, as the core refuses an existing web font.
fn refuse_existing(path: &Path, force: bool) -> Result<(), ApiError> {
    if !force && std::fs::symlink_metadata(path).is_ok() {
        return Err(already_exists(path));
    }
    Ok(())
}

fn already_exists(path: &Path) -> ApiError {
    ApiError::new(
        ErrorCode::InvalidRequest,
        format!(
            "{} already exists. Use --force to replace it.",
            path.display()
        ),
    )
    .with_detail(path.display().to_string())
}

/// `fm export <family>`: every face of the families the argument names,
/// written as web fonts (and `fonts.css`) to `out_dir`, the folder the
/// user gave. Files of the same name are replaced only with `force`.
pub fn export_web(
    api: &dyn Api,
    family: &str,
    formats: Vec<WebFormat>,
    css: bool,
    out_dir: &Path,
    force: bool,
) -> Result<ExportWebResponse, Failure> {
    let out_dir = user_path(out_dir)?;
    let faces = faces(api, family)?.into_iter().map(|f| f.id).collect();
    Ok(api.export_web(ExportWebRequest {
        faces,
        formats,
        subset: None,
        out_dir,
        css,
        overwrite: force,
    })?)
}

/// `fm specimen <family>`: the family's specimen written to `out`, the
/// file the user gave; an existing file only with `force`.
pub fn specimen(
    api: &dyn Api,
    family: &str,
    format: SpecimenFormat,
    out: &Path,
    force: bool,
) -> Result<SpecimenResponse, Failure> {
    let out_path = user_path(out)?;
    let family = one_family(api, family)?;
    refuse_existing(out, force)?;
    Ok(api.export_specimen(SpecimenRequest {
        family,
        format,
        out_path,
    })?)
}

/// Where `fm preview` writes the PNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewOut {
    /// `--out -`: the bytes to stdout.
    Stdout,
    /// A file the user gave.
    File(PathBuf),
}

/// The `fm preview` settings.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewFlags {
    /// The text to render.
    pub text: String,
    /// Size in pixels.
    pub size: f32,
    /// Text color, CSS hex.
    pub fg: Option<String>,
    /// Background color, CSS hex.
    pub bg: Option<String>,
}

/// What `fm preview` rendered and where it went. The PNG bytes are not
/// serialized: `--json` prints the path and the size only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Previewed {
    /// The face rendered.
    pub face: FaceId,
    /// The file written, absolute; absent when the PNG went to stdout.
    pub path: Option<String>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Size of the PNG in bytes.
    pub bytes: usize,
    /// The PNG, for stdout.
    #[serde(skip)]
    pub png: Vec<u8>,
}

/// `fm preview <font>`: the face's rendering of the text, written to the
/// file the user gave (a new one, or any with `force`), or kept for stdout.
pub fn preview(
    api: &dyn Api,
    font: &str,
    flags: &PreviewFlags,
    out: &PreviewOut,
    force: bool,
) -> Result<Previewed, Failure> {
    let face = resolve(api, font)?.face;
    if let PreviewOut::File(path) = out {
        user_path(path)?;
        refuse_existing(path, force)?;
    }
    let image = api.render_preview(PreviewRequest {
        face,
        text: flags.text.clone(),
        size_px: flags.size,
        variations: Vec::new(),
        features: Vec::new(),
        foreground: flags.fg.clone(),
        background: flags.bg.clone(),
    })?;
    let path = match out {
        PreviewOut::Stdout => None,
        PreviewOut::File(path) => Some(write_png(path, &image.png, force)?),
    };
    Ok(Previewed {
        face,
        path,
        width: image.width,
        height: image.height,
        bytes: image.png.len(),
        png: image.png,
    })
}

/// Writes `png` to `path`: a new file, or over an existing one with
/// `force`. Answers the absolute path.
fn write_png(path: &Path, png: &[u8], force: bool) -> Result<String, ApiError> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        // A file made since the check is not replaced either.
        options.create_new(true);
    }
    let failed = |e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            return already_exists(path);
        }
        ApiError::new(
            ErrorCode::Io,
            format!("The preview could not be written to {}.", path.display()),
        )
        .with_detail(e.to_string())
    };
    let mut file = options.open(path).map_err(failed)?;
    if let Err(e) = file.write_all(png) {
        drop(file);
        if !force {
            // The file this command just made, never one of the user's.
            let _ = std::fs::remove_file(path);
        }
        return Err(failed(e));
    }
    let absolute = std::path::absolute(path).map_err(failed)?;
    Ok(absolute.display().to_string())
}
