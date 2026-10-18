//! `MockApi`: every service on in-memory sample data, for frontend
//! development and adapter tests before (and beside) the real core.
//!
//! It behaves like the contract says — per-face activation outcomes, conflicts,
//! the license guard, events — but touches no file and no OS: scans find
//! nothing new, exports report the files they would write, and previews are a
//! 1×1 transparent PNG.

use std::collections::{BTreeSet, HashMap};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, MutexGuard};

use crate::activation::{
    ActivateRequest, ActivationMode, ActivationOutcome, ActiveFace, Conflict, ConflictPolicy,
    ConflictWith, DeactivateRequest, OutcomeStatus, Target,
};
use crate::common::{
    Axis, AxisValue, Classification, Embedding, FeatureInfo, FontFormat, Health, Metrics,
    NamedInstance, Paged, Slant,
};
use crate::export::{
    ExportWebRequest, ExportWebResponse, LicenseInventory, LicenseInventoryRequest, WebFormat,
};
use crate::library::{
    AddSourceRequest, AppInfo, FaceDetail, FaceSummary, Family, FamilyActivation, FamilySort,
    FamilySummary, ImportMode, LicenseInfo, LicenseType, ListFamiliesRequest, OpenLicense,
    Platform, Problem, ProblemKind, ScanRequest, SetSourcePausedRequest, Source, SourceKind,
    StyleMark,
};
use crate::organization::{
    Collection, CollectionFacesRequest, CreateCollectionRequest, CreateSetRequest,
    ImportSetResponse, ProjectSet, RenameCollectionRequest, SetEntry, SetExport, Tag,
    TagFacesRequest, UntagFacesRequest, UserMetaUpdate,
};
use crate::preview::{
    FontSource, PreviewImage, PreviewRequest, SampleFallback, SampleText, SampleTextRequest,
};
use crate::search::{
    CoverageReport, CoverageRequest, FamilyHit, Filters, LanguageCoverage, LanguagePreset, Missing,
    SearchRequest, SearchResponse, SimilarFace, SimilarRequest, Supports, WeightRange,
};
use crate::settings::{AppFolders, Settings, SettingsUpdate};
use crate::workflows::{
    BriefRequest, Candidate, PrepareForWebRequest, PrepareForWebResponse, Shortlist,
};
use crate::{
    API_VERSION, ActivationService, ApiError, ApiResult, CollectionId, ErrorCode, Event,
    EventSource, ExportService, FaceId, FamilyId, JobId, LibraryService, OrganizationService,
    PreviewService, SearchService, SetId, SettingsService, SourceId, TagId, WorkflowService,
};

/// The mock's clock: every timestamp it makes.
pub const MOCK_NOW: &str = "2026-10-16T09:00:00Z";

/// Near-misses within this many characters still show in a coverage search.
const NEAR_MISS: usize = 5;

/// The largest page a list returns.
const MAX_PAGE: u32 = 500;

/// A 1×1 transparent PNG.
const PNG_1X1: [u8; 67] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

const LATIN: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,;:!?'\"-()";
const LATIN_EXT: &str = "ąćęłńóśźżčđšžĄĆĘŁŃÓŚŹŻČĐŠŽ";
const CYRILLIC: &str = "абвгдђежзијклљмнњопрстћуфхцчџшАБВГДЂЕЖЗИЈКЛЉМНЊОПРСТЋУФХЦЧЏШ";

/// (code, name, script, characters the language needs)
const LANGUAGES: [(&str, &str, &str, &str); 4] = [
    ("en", "English", "Latn", "abcdefghijklmnopqrstuvwxyz"),
    ("pl", "Polish", "Latn", "aąbcćdeęfghijklłmnńoóprsśtuwyzźż"),
    (
        "sr-Latn",
        "Serbian (Latin)",
        "Latn",
        "abcčćdđefghijklmnoprsštuvzž",
    ),
    (
        "sr-Cyrl",
        "Serbian (Cyrillic)",
        "Cyrl",
        "абвгдђежзијклљмнњопрстћуфхцчџш",
    ),
];

struct MockFamily {
    id: FamilyId,
    name: &'static str,
    foundry: &'static str,
    designer: &'static str,
    classification: Classification,
    added: &'static str,
}

struct MockFace {
    detail: FaceDetail,
    source: SourceId,
    chars: BTreeSet<char>,
    /// A system font with the same PostScript name, if any.
    system_clash: Option<&'static str>,
}

struct State {
    sources: Vec<Source>,
    families: Vec<MockFamily>,
    faces: Vec<MockFace>,
    active: HashMap<FaceId, ActiveFace>,
    collections: Vec<Collection>,
    members: HashMap<CollectionId, BTreeSet<FaceId>>,
    tags: Vec<Tag>,
    face_tags: HashMap<FaceId, BTreeSet<TagId>>,
    sets: Vec<ProjectSet>,
    settings: Settings,
    next_id: u32,
    subscribers: Vec<Sender<Event>>,
}

/// Every service on in-memory sample data. Cheap to create; each instance has
/// its own state.
#[derive(Debug)]
pub struct MockApi {
    state: Mutex<State>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("faces", &self.faces.len())
            .field("active", &self.active.len())
            .finish_non_exhaustive()
    }
}

impl Default for MockApi {
    fn default() -> Self {
        Self::new()
    }
}

impl MockApi {
    /// A mock with four sample families (seven faces) in one watched folder.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State::sample()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock cannot leave the sample data half-written
        // in a way that matters to a mock, so a poisoned lock is still used.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[allow(clippy::too_many_arguments)]
fn face(
    id: u32,
    family: (FamilyId, &str),
    style: &str,
    postscript: &str,
    (weight, slant): (u16, Slant),
    format: FontFormat,
    path: &str,
    variable: bool,
    embedding: Embedding,
    chars: &[&str],
) -> MockFace {
    let axes = if variable {
        vec![Axis {
            tag: "wght".into(),
            name: "Weight".into(),
            min: 100.0,
            default: 400.0,
            max: 900.0,
        }]
    } else {
        Vec::new()
    };
    let instances = if variable {
        [
            ("Thin", 100.0),
            ("Regular", 400.0),
            ("Bold", 700.0),
            ("Black", 900.0),
        ]
        .into_iter()
        .map(|(name, value)| NamedInstance {
            name: name.into(),
            coordinates: vec![AxisValue {
                tag: "wght".into(),
                value,
            }],
        })
        .collect()
    } else {
        Vec::new()
    };
    let health = match format {
        FontFormat::Type1 => {
            Health::Unsupported("Type 1 fonts cannot be activated; convert to OTF.".into())
        }
        _ => Health::Ok,
    };
    let open_license = (embedding == Embedding::Installable).then_some(OpenLicense::Ofl);
    let char_set: BTreeSet<char> = chars.iter().flat_map(|s| s.chars()).collect();
    MockFace {
        detail: FaceDetail {
            face: FaceSummary {
                id: FaceId(id),
                family: family.0,
                family_name: family.1.into(),
                style: style.into(),
                postscript_name: postscript.into(),
                weight,
                width: 5,
                slant,
                format,
                is_variable: variable,
                active: None,
                health,
            },
            path: path.into(),
            collection_index: 0,
            file_size: 120_000 + id * 1_000,
            hash: format!("{id:064x}"),
            version: Some("Version 1.000".into()),
            copyright: Some(format!("Copyright {} Authors", family.1)),
            designer: None,
            vendor_id: None,
            license: LicenseInfo {
                description: open_license.map(|_| "SIL Open Font License 1.1".into()),
                url: open_license.map(|_| "https://openfontlicense.org".into()),
                open_license,
                embedding,
                declared: None,
            },
            metrics: Metrics {
                units_per_em: 1000,
                ascender: 900,
                descender: -250,
                x_height: Some(520),
                cap_height: Some(720),
            },
            axes,
            instances,
            features: ["kern", "liga", "smcp", "onum", "ss01"]
                .iter()
                .take(if variable { 5 } else { 2 })
                .map(|t| FeatureInfo {
                    tag: (*t).into(),
                    name: None,
                })
                .collect(),
            scripts: if chars.contains(&CYRILLIC) {
                vec!["latn".into(), "cyrl".into()]
            } else {
                vec!["latn".into()]
            },
            tags: Vec::new(),
            favorite: false,
            rating: None,
            notes: None,
            legacy_family: Some(family.1.into()),
            legacy_style: Some(style.into()),
            foundry: None,
            italic_angle: Some(if slant == Slant::Upright { 0.0 } else { -10.0 }),
            code_point_count: Some(char_set.len() as u32),
        },
        source: SourceId(1),
        chars: char_set,
        system_clash: None,
    }
}

impl State {
    fn sample() -> Self {
        let fam = |id: u32, name, foundry, designer, classification, added| MockFamily {
            id: FamilyId(id),
            name,
            foundry,
            designer,
            classification,
            added,
        };
        let families = vec![
            fam(
                1,
                "Inter",
                "rsms",
                "Rasmus Andersson",
                Classification::Sans,
                "2026-10-01T10:00:00Z",
            ),
            fam(
                2,
                "Source Serif 4",
                "Adobe",
                "Frank Grießhammer",
                Classification::Serif,
                "2026-10-03T10:00:00Z",
            ),
            fam(
                3,
                "JetBrains Mono",
                "JetBrains",
                "Philipp Nurullin",
                Classification::Mono,
                "2026-10-05T10:00:00Z",
            ),
            fam(
                4,
                "Legacy Grotesk",
                "Old Foundry",
                "Unknown",
                Classification::Sans,
                "2026-10-07T10:00:00Z",
            ),
        ];
        let all = [LATIN, LATIN_EXT, CYRILLIC];
        let f = |i: usize| (families[i].id, families[i].name);
        let mut faces = vec![
            face(
                1,
                f(0),
                "Regular",
                "Inter-Regular",
                (400, Slant::Upright),
                FontFormat::Ttf,
                "/Users/demo/Fonts/Inter/Inter[wght].ttf",
                true,
                Embedding::Installable,
                &all,
            ),
            face(
                2,
                f(0),
                "Italic",
                "Inter-Italic",
                (400, Slant::Italic),
                FontFormat::Ttf,
                "/Users/demo/Fonts/Inter/Inter-Italic[wght].ttf",
                true,
                Embedding::Installable,
                &all,
            ),
            face(
                3,
                f(1),
                "Regular",
                "SourceSerif4-Regular",
                (400, Slant::Upright),
                FontFormat::Otf,
                "/Users/demo/Fonts/SourceSerif4/SourceSerif4-Regular.otf",
                false,
                Embedding::Installable,
                &[LATIN, LATIN_EXT],
            ),
            face(
                4,
                f(1),
                "Italic",
                "SourceSerif4-It",
                (400, Slant::Italic),
                FontFormat::Otf,
                "/Users/demo/Fonts/SourceSerif4/SourceSerif4-It.otf",
                false,
                Embedding::Installable,
                &[LATIN, LATIN_EXT],
            ),
            face(
                5,
                f(1),
                "Bold",
                "SourceSerif4-Bold",
                (700, Slant::Upright),
                FontFormat::Otf,
                "/Users/demo/Fonts/SourceSerif4/SourceSerif4-Bold.otf",
                false,
                Embedding::Installable,
                &[LATIN, LATIN_EXT],
            ),
            face(
                6,
                f(2),
                "Regular",
                "JetBrainsMono-Regular",
                (400, Slant::Upright),
                FontFormat::Ttf,
                "/Users/demo/Fonts/JetBrainsMono-Regular.ttf",
                false,
                Embedding::Installable,
                &[LATIN, CYRILLIC],
            ),
            face(
                7,
                f(3),
                "Regular",
                "LegacyGrotesk",
                (400, Slant::Upright),
                FontFormat::Type1,
                "/Users/demo/Fonts/legacy/LEGGR___.PFB",
                false,
                Embedding::Restricted,
                &[LATIN],
            ),
        ];
        faces[5].system_clash = Some("/System/Library/Fonts/JetBrainsMono-Regular.ttf");
        let sources = vec![
            Source {
                id: SourceId(1),
                path: "/Users/demo/Fonts".into(),
                kind: SourceKind::Watched,
                paused: false,
                face_count: 7,
            },
            Source {
                id: SourceId(2),
                path: "/System/Library/Fonts".into(),
                kind: SourceKind::System,
                paused: false,
                face_count: 0,
            },
        ];
        Self {
            sources,
            families,
            faces,
            active: HashMap::new(),
            collections: Vec::new(),
            members: HashMap::new(),
            tags: Vec::new(),
            face_tags: HashMap::new(),
            sets: Vec::new(),
            settings: Settings::default(),
            next_id: 100,
            subscribers: Vec::new(),
        }
    }

    fn next(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn emit(&mut self, event: Event) {
        self.subscribers.retain(|s| s.send(event.clone()).is_ok());
    }

    fn face(&self, id: FaceId) -> ApiResult<&MockFace> {
        self.faces
            .iter()
            .find(|f| f.detail.face.id == id)
            .ok_or_else(|| ApiError::not_found(format!("Face {id}")))
    }

    fn family(&self, id: FamilyId) -> ApiResult<&MockFamily> {
        self.families
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| ApiError::not_found(format!("Family {id}")))
    }

    fn visible(&self, f: &MockFace) -> bool {
        self.sources.iter().any(|s| s.id == f.source && !s.paused)
    }

    fn summary(&self, f: &MockFace) -> FaceSummary {
        let mut s = f.detail.face.clone();
        s.active = self.active.get(&s.id).map(|a| a.mode);
        s
    }

    fn detail(&self, f: &MockFace) -> FaceDetail {
        let mut d = f.detail.clone();
        d.face = self.summary(f);
        let family = self.family(d.face.family).ok();
        d.designer = family.map(|fam| fam.designer.into());
        d.foundry = family.map(|fam| fam.foundry.into());
        d.tags = self
            .face_tags
            .get(&d.face.id)
            .into_iter()
            .flatten()
            .filter_map(|t| self.tags.iter().find(|tag| tag.id == *t))
            .map(|t| t.name.clone())
            .collect();
        d
    }

    fn family_faces(&self, id: FamilyId) -> impl Iterator<Item = &MockFace> {
        self.faces
            .iter()
            .filter(move |f| f.detail.face.family == id && self.visible(f))
    }

    fn family_summary(&self, fam: &MockFamily) -> FamilySummary {
        let faces: Vec<_> = self.family_faces(fam.id).collect();
        let active = faces
            .iter()
            .filter(|f| self.active.contains_key(&f.detail.face.id))
            .count();
        let mut formats = Vec::new();
        for f in &faces {
            if !formats.contains(&f.detail.face.format) {
                formats.push(f.detail.face.format);
            }
        }
        let mut styles: Vec<StyleMark> = faces
            .iter()
            .map(|f| StyleMark {
                weight: f.detail.face.weight,
                italic: f.detail.face.slant != Slant::Upright,
            })
            .collect();
        styles.sort_by_key(|s| (s.weight, s.italic));
        // A variable face spans its `wght` axis; a static one sits at its weight.
        let weight_range = faces
            .iter()
            .flat_map(|f| {
                let axis = f.detail.axes.iter().find(|a| a.tag == "wght");
                let w = f.detail.face.weight;
                [
                    axis.map_or(w, |a| a.min as u16),
                    axis.map_or(w, |a| a.max as u16),
                ]
            })
            .fold(None, |acc: Option<WeightRange>, w| {
                Some(acc.map_or(WeightRange { min: w, max: w }, |r| WeightRange {
                    min: r.min.min(w),
                    max: r.max.max(w),
                }))
            });
        let mut tags: Vec<String> = faces
            .iter()
            .flat_map(|f| self.face_tags.get(&f.detail.face.id).into_iter().flatten())
            .filter_map(|t| self.tags.iter().find(|tag| tag.id == *t))
            .map(|t| t.name.clone())
            .collect();
        tags.sort_by_key(|t| t.to_lowercase());
        tags.dedup();
        let first = faces.first();
        FamilySummary {
            id: fam.id,
            name: fam.name.into(),
            style_count: faces.len() as u32,
            formats,
            foundry: Some(fam.foundry.into()),
            classification: Some(fam.classification),
            is_variable: faces.iter().any(|f| f.detail.face.is_variable),
            activation: match active {
                0 => FamilyActivation::Inactive,
                n if n == faces.len() => FamilyActivation::Active,
                _ => FamilyActivation::Partial,
            },
            added: fam.added.into(),
            styles,
            weight_range,
            is_color: false, // the sample has no color fonts
            version: first.and_then(|f| f.detail.version.clone()),
            location: first
                .and_then(|f| self.sources.iter().find(|s| s.id == f.source))
                .map(|s| s.path.clone()),
            open_license: faces.iter().find_map(|f| f.detail.license.open_license),
            declared_license: faces.iter().find_map(|f| f.detail.license.declared),
            tags,
        }
    }

    /// Characters of `target` the face lacks, and per-language detail.
    fn coverage(
        &self,
        f: &MockFace,
        target: &Supports,
    ) -> ApiResult<(Vec<String>, Vec<LanguageCoverage>)> {
        let lacks = |chars: &str| -> Vec<String> {
            let mut seen = BTreeSet::new();
            chars
                .chars()
                .filter(|c| !c.is_whitespace() && !f.chars.contains(c) && seen.insert(*c))
                .map(String::from)
                .collect()
        };
        match target {
            Supports::Text { text } => Ok((lacks(text), Vec::new())),
            Supports::Languages { codes } => {
                let mut all = Vec::new();
                let mut per = Vec::new();
                for code in codes {
                    let (_, _, _, chars) =
                        LANGUAGES.iter().find(|l| l.0 == code).ok_or_else(|| {
                            ApiError::invalid(format!("Unknown language preset \"{code}\"."))
                        })?;
                    let missing = lacks(chars);
                    let total = chars.chars().count() as u32;
                    per.push(LanguageCoverage {
                        code: code.clone(),
                        covered: total - missing.len() as u32,
                        total,
                        missing: missing.clone(),
                        has_locl: code == "sr-Cyrl" && f.chars.contains(&'б'),
                    });
                    for m in missing {
                        if !all.contains(&m) {
                            all.push(m);
                        }
                    }
                }
                Ok((all, per))
            }
        }
    }

    fn in_collection(&self, f: &MockFace, id: CollectionId, depth: u8) -> bool {
        let Some(c) = self.collections.iter().find(|c| c.id == id) else {
            return false;
        };
        match &c.rules {
            Some(rules) if depth < 4 => self.matches(f, rules, depth + 1).unwrap_or(false),
            Some(_) => false,
            None => self
                .members
                .get(&id)
                .is_some_and(|m| m.contains(&f.detail.face.id)),
        }
    }

    fn matches(&self, f: &MockFace, q: &Filters, depth: u8) -> ApiResult<bool> {
        let s = &f.detail.face;
        let fam = self.family(s.family)?;
        let tags = self.face_tags.get(&s.id);
        let ok = (q.classification.is_empty() || q.classification.contains(&fam.classification))
            && q.weight.is_none_or(|w| (w.min..=w.max).contains(&s.weight))
            && q.width.is_none_or(|w| (w.min..=w.max).contains(&s.width))
            && q.slant.is_none_or(|sl| sl == s.slant)
            && q.variable.is_none_or(|v| v == s.is_variable)
            && q.color.is_none_or(|c| !c) // the sample has no color fonts
            && (q.formats.is_empty() || q.formats.contains(&s.format))
            && q.features.iter().all(|t| f.detail.features.iter().any(|x| &x.tag == t))
            && q.license.is_none_or(|l| f.detail.license.declared == Some(l))
            && q.foundry.as_deref().is_none_or(|x| x == fam.foundry)
            && (q.sources.is_empty() || q.sources.contains(&f.source))
            && q.tags.iter().all(|t| tags.is_some_and(|set| set.contains(t)))
            && q.collection.is_none_or(|c| self.in_collection(f, c, depth))
            && q.active.is_none_or(|a| a == self.active.contains_key(&s.id))
            && q.favorite.is_none_or(|v| v == f.detail.favorite)
            && q.min_rating.is_none_or(|r| f.detail.rating.is_some_and(|x| x >= r))
            && q.problems.is_none_or(|p| p == (s.health != Health::Ok));
        Ok(ok)
    }

    fn collection_view(&self, c: &Collection) -> Collection {
        let mut c = c.clone();
        c.face_count = self
            .faces
            .iter()
            .filter(|f| self.in_collection(f, c.id, 0))
            .count() as u32;
        c
    }

    fn target_faces(&self, target: &Target) -> ApiResult<Vec<FaceId>> {
        match target {
            Target::Faces { faces } => {
                for id in faces {
                    self.face(*id)?;
                }
                Ok(faces.clone())
            }
            Target::Family { family } => {
                self.family(*family)?;
                Ok(self
                    .family_faces(*family)
                    .map(|f| f.detail.face.id)
                    .collect())
            }
            Target::Collection { collection } => {
                if !self.collections.iter().any(|c| c.id == *collection) {
                    return Err(ApiError::not_found(format!("Collection {collection}")));
                }
                Ok(self
                    .faces
                    .iter()
                    .filter(|f| self.in_collection(f, *collection, 0))
                    .map(|f| f.detail.face.id)
                    .collect())
            }
            Target::Set { set } => self
                .sets
                .iter()
                .find(|s| s.id == *set)
                .map(|s| s.faces.clone())
                .ok_or_else(|| ApiError::not_found(format!("Set {set}"))),
        }
    }

    fn conflict(&self, f: &MockFace) -> Option<ConflictWith> {
        if let Some(path) = f.system_clash {
            return Some(ConflictWith::System { path: path.into() });
        }
        self.faces
            .iter()
            .find(|o| {
                o.detail.face.id != f.detail.face.id
                    && o.detail.face.postscript_name == f.detail.face.postscript_name
                    && self.active.contains_key(&o.detail.face.id)
            })
            .map(|o| ConflictWith::Active {
                face: o.detail.face.id,
            })
    }

    fn page<T: Clone>(items: &[T], offset: u32, limit: u32) -> Paged<T> {
        Paged {
            items: items
                .iter()
                .skip(offset as usize)
                .take(limit.min(MAX_PAGE) as usize)
                .cloned()
                .collect(),
            total: items.len() as u32,
        }
    }
}

impl LibraryService for MockApi {
    fn app_info(&self) -> ApiResult<AppInfo> {
        Ok(AppInfo {
            name: "Font Manager (mock)".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            api_version: API_VERSION,
            platform: if cfg!(windows) {
                Platform::Windows
            } else if cfg!(target_os = "macos") {
                Platform::Macos
            } else {
                Platform::Linux
            },
        })
    }

    fn list_sources(&self) -> ApiResult<Vec<Source>> {
        Ok(self.state().sources.clone())
    }

    fn add_source(&self, req: AddSourceRequest) -> ApiResult<Source> {
        if req.path.trim().is_empty() {
            return Err(ApiError::invalid("Choose a folder to add."));
        }
        if let ImportMode::CopyTo { folder } = &req.mode
            && folder.trim().is_empty()
        {
            return Err(ApiError::invalid(
                "Choose a library folder to copy the fonts into.",
            ));
        }
        let mut st = self.state();
        if st.sources.iter().any(|s| s.path == req.path) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "This folder is already in the library.",
            ));
        }
        let source = Source {
            id: SourceId(st.next()),
            path: req.path,
            kind: SourceKind::Watched,
            paused: false,
            face_count: 0,
        };
        st.sources.push(source.clone());
        Ok(source)
    }

    fn remove_source(&self, source: SourceId) -> ApiResult<()> {
        let mut st = self.state();
        let s = st
            .sources
            .iter()
            .find(|s| s.id == source)
            .ok_or_else(|| ApiError::not_found(format!("Source {source}")))?;
        if s.kind == SourceKind::System {
            return Err(ApiError::new(
                ErrorCode::PermissionDenied,
                "System fonts cannot be removed.",
            ));
        }
        let removed: Vec<FaceId> = st
            .faces
            .iter()
            .filter(|f| f.source == source)
            .map(|f| f.detail.face.id)
            .collect();
        st.faces.retain(|f| f.source != source);
        for id in &removed {
            st.active.remove(id);
        }
        st.sources.retain(|s| s.id != source);
        st.emit(Event::LibraryChanged {
            source: Some(source),
            added: Vec::new(),
            removed,
            updated: Vec::new(),
        });
        Ok(())
    }

    fn set_source_paused(&self, req: SetSourcePausedRequest) -> ApiResult<Source> {
        let mut st = self.state();
        let s = st
            .sources
            .iter_mut()
            .find(|s| s.id == req.source)
            .ok_or_else(|| ApiError::not_found(format!("Source {}", req.source)))?;
        s.paused = req.paused;
        Ok(s.clone())
    }

    fn scan(&self, req: ScanRequest) -> ApiResult<JobId> {
        let mut st = self.state();
        let files = match req.source {
            Some(id) => {
                if !st.sources.iter().any(|s| s.id == id) {
                    return Err(ApiError::not_found(format!("Source {id}")));
                }
                st.faces.iter().filter(|f| f.source == id).count()
            }
            None => st.faces.len(),
        } as u32;
        let job = JobId(st.next());
        let problems = st
            .faces
            .iter()
            .filter(|f| f.detail.face.health != Health::Ok)
            .count() as u32;
        st.emit(Event::ScanStarted {
            job,
            source: req.source,
        });
        st.emit(Event::ScanProgress {
            job,
            done: files,
            total: files,
        });
        st.emit(Event::ScanFinished {
            job,
            added: 0,
            changed: 0,
            removed: 0,
            problems,
        });
        Ok(job)
    }

    fn rebuild_index(&self) -> ApiResult<JobId> {
        // The sample data is the "folders": a rebuild re-reads all of it and
        // keeps every id and user choice, like the core.
        self.scan(ScanRequest::default())
    }

    fn list_families(&self, req: ListFamiliesRequest) -> ApiResult<Paged<FamilySummary>> {
        let st = self.state();
        let mut rows: Vec<FamilySummary> = st
            .families
            .iter()
            .filter(|fam| {
                st.family_faces(fam.id)
                    .any(|f| req.source.is_none_or(|s| s == f.source))
            })
            .map(|fam| st.family_summary(fam))
            .collect();
        match (req.order, req.sort) {
            (Some(order), _) => order.sort(&mut rows),
            (None, FamilySort::Name) => rows.sort_by_key(|r| r.name.to_lowercase()),
            (None, FamilySort::DateAdded) => rows.sort_by(|a, b| b.added.cmp(&a.added)),
            (None, FamilySort::StyleCount) => {
                rows.sort_by_key(|r| std::cmp::Reverse(r.style_count));
            }
        }
        Ok(State::page(&rows, req.page.offset, req.page.limit))
    }

    fn get_family(&self, family: FamilyId) -> ApiResult<Family> {
        let st = self.state();
        let fam = st.family(family)?;
        let mut faces: Vec<FaceSummary> = st.family_faces(family).map(|f| st.summary(f)).collect();
        faces.sort_by_key(|f| (f.weight, f.width, f.slant != Slant::Upright));
        Ok(Family {
            summary: st.family_summary(fam),
            designer: Some(fam.designer.into()),
            faces,
        })
    }

    fn get_face(&self, face: FaceId) -> ApiResult<FaceDetail> {
        let st = self.state();
        let f = st.face(face)?;
        Ok(st.detail(f))
    }

    fn list_problems(&self) -> ApiResult<Vec<Problem>> {
        let st = self.state();
        let mut out = Vec::new();
        for f in &st.faces {
            if let Health::Unsupported(reason) | Health::Broken(reason) = &f.detail.face.health {
                out.push(Problem {
                    kind: if matches!(f.detail.face.health, Health::Broken(_)) {
                        ProblemKind::Broken
                    } else {
                        ProblemKind::Unsupported
                    },
                    face: Some(f.detail.face.id),
                    path: f.detail.path.clone(),
                    other: None,
                    message: reason.clone(),
                });
            }
            if let Some(path) = f.system_clash {
                out.push(Problem {
                    kind: ProblemKind::SystemConflict,
                    face: Some(f.detail.face.id),
                    path: f.detail.path.clone(),
                    other: Some(path.into()),
                    message: format!(
                        "{} has the same PostScript name as a system font.",
                        f.detail.face.postscript_name
                    ),
                });
            }
        }
        Ok(out)
    }
}

impl SearchService for MockApi {
    fn search(&self, req: SearchRequest) -> ApiResult<SearchResponse> {
        let st = self.state();
        let words: Vec<String> = req
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut hits: Vec<FamilyHit> = Vec::new();
        for fam in &st.families {
            let mut faces = Vec::new();
            let mut best_missing: Option<Vec<String>> = None;
            for f in st.family_faces(fam.id) {
                let s = &f.detail.face;
                let hay = format!(
                    "{} {} {} {} {}",
                    s.family_name, s.style, s.postscript_name, fam.foundry, fam.designer
                )
                .to_lowercase();
                if !words.iter().all(|w| hay.contains(w)) || !st.matches(f, &req.filters, 0)? {
                    continue;
                }
                if let Some(target) = &req.filters.supports {
                    let (missing, _) = st.coverage(f, target)?;
                    if missing.len() > NEAR_MISS {
                        continue;
                    }
                    if best_missing
                        .as_ref()
                        .is_none_or(|b| missing.len() < b.len())
                    {
                        best_missing = Some(missing);
                    }
                }
                faces.push(s.id);
            }
            if faces.is_empty() {
                continue;
            }
            let name_hit = words.iter().any(|w| fam.name.to_lowercase().contains(w));
            hits.push(FamilyHit {
                family: st.family_summary(fam),
                score: if name_hit { 1.0 } else { 0.5 }
                    - best_missing.as_ref().map_or(0.0, |m| m.len() as f32 * 0.1),
                faces,
                missing: best_missing.map(|chars| Missing {
                    has_locl: false,
                    chars,
                }),
            });
        }
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.family.name.cmp(&b.family.name))
        });
        let page = State::page(&hits, req.page.offset, req.page.limit);
        Ok(SearchResponse {
            families: page.items,
            total: page.total,
            counts: None,
        })
    }

    fn check_coverage(&self, req: CoverageRequest) -> ApiResult<CoverageReport> {
        let st = self.state();
        let f = st.face(req.face)?;
        let (missing, languages) = st.coverage(f, &req.target)?;
        Ok(CoverageReport {
            face: req.face,
            complete: missing.is_empty(),
            missing,
            languages,
        })
    }

    fn list_languages(&self) -> ApiResult<Vec<LanguagePreset>> {
        Ok(LANGUAGES
            .iter()
            .map(|(code, name, script, _)| LanguagePreset {
                code: (*code).into(),
                name: (*name).into(),
                script: (*script).into(),
            })
            .collect())
    }

    fn find_similar(&self, req: SimilarRequest) -> ApiResult<Vec<SimilarFace>> {
        let st = self.state();
        let me = st.face(req.face)?;
        let my_class = st.family(me.detail.face.family)?.classification;
        let mut out = Vec::new();
        for f in st
            .faces
            .iter()
            .filter(|f| f.detail.face.id != req.face && st.visible(f))
        {
            let class = st.family(f.detail.face.family)?.classification;
            let mut score: f32 = 0.2;
            let mut reasons = Vec::new();
            if class == my_class {
                score += 0.5;
                reasons.push("same classification".to_string());
            }
            if f.detail.face.weight == me.detail.face.weight {
                score += 0.2;
                reasons.push("same weight".to_string());
            }
            if f.detail.metrics.x_height == me.detail.metrics.x_height {
                score += 0.1;
                reasons.push("matching x-height".to_string());
            }
            out.push(SimilarFace {
                face: st.summary(f),
                score: score.min(1.0),
                reasons,
            });
        }
        out.sort_by(|a, b| b.score.total_cmp(&a.score));
        out.truncate(req.limit as usize);
        Ok(out)
    }
}

impl PreviewService for MockApi {
    fn render_preview(&self, req: PreviewRequest) -> ApiResult<PreviewImage> {
        if !(8.0..=300.0).contains(&req.size_px) {
            return Err(ApiError::invalid(
                "Preview size must be between 8 and 300 px.",
            ));
        }
        self.state().face(req.face)?;
        Ok(PreviewImage {
            width: 1,
            height: 1,
            png: PNG_1X1.to_vec(),
        })
    }

    fn font_source(&self, face: FaceId) -> ApiResult<FontSource> {
        let st = self.state();
        let f = st.face(face)?;
        Ok(FontSource {
            face,
            path: f.detail.path.clone(),
            collection_index: f.detail.collection_index,
            hash: f.detail.hash.clone(),
        })
    }

    /// The core's rule (`fm_core::sample`) on the mock's languages: the
    /// sample when the face maps it, else lower-case letters of the complete
    /// language whose script the face maps most, else its own characters.
    fn sample_texts(&self, req: SampleTextRequest) -> ApiResult<Vec<SampleText>> {
        let st = self.state();
        req.faces
            .iter()
            .map(|&id| {
                let f = st.face(id)?;
                let mut missing: Vec<String> = Vec::new();
                for c in req.text.chars() {
                    let s = c.to_string();
                    if !c.is_whitespace()
                        && !c.is_control()
                        && !f.chars.contains(&c)
                        && !missing.contains(&s)
                    {
                        missing.push(s);
                    }
                }
                if missing.is_empty() {
                    return Ok(SampleText {
                        face: id,
                        text: req.text.clone(),
                        fallback: None,
                        missing,
                    });
                }
                let mut best: Option<(usize, &str, &str)> = None;
                for (_, _, script, chars) in LANGUAGES {
                    if !chars.chars().all(|c| f.chars.contains(&c)) {
                        continue;
                    }
                    let score = LANGUAGES
                        .iter()
                        .filter(|l| l.2 == script)
                        .flat_map(|l| l.3.chars())
                        .collect::<BTreeSet<char>>()
                        .iter()
                        .filter(|c| f.chars.contains(c))
                        .count();
                    if best.is_none_or(|(top, _, _)| score > top) {
                        best = Some((score, script, chars));
                    }
                }
                let (text, fallback) = match best {
                    Some((_, script, chars)) => (
                        chars.chars().take(26).collect(),
                        SampleFallback::Script {
                            script: script.into(),
                        },
                    ),
                    None => {
                        let mut list: Vec<char> = f
                            .chars
                            .iter()
                            .copied()
                            .filter(|c| !c.is_whitespace())
                            .collect();
                        list.sort_by_key(|c| !c.is_alphanumeric());
                        let text = list
                            .iter()
                            .take(24)
                            .map(char::to_string)
                            .collect::<Vec<_>>()
                            .join(" ");
                        (text, SampleFallback::Symbols)
                    }
                };
                Ok(SampleText {
                    face: id,
                    text,
                    fallback: Some(fallback),
                    missing,
                })
            })
            .collect()
    }
}

impl ActivationService for MockApi {
    fn activate(&self, req: ActivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
        let mut st = self.state();
        let ids = st.target_faces(&req.target)?;
        let mut out = Vec::new();
        let mut activated = Vec::new();
        for id in ids {
            let f = st.face(id)?;
            let outcome = if let Health::Unsupported(reason) | Health::Broken(reason) =
                &f.detail.face.health
            {
                OutcomeStatus::Skipped {
                    reason: reason.clone(),
                }
            } else if st.active.get(&id).is_some_and(|a| a.mode == req.mode) {
                OutcomeStatus::Unchanged
            } else if let Some(with) = st
                .conflict(f)
                .filter(|_| req.on_conflict == ConflictPolicy::KeepExisting)
            {
                OutcomeStatus::Conflict { with }
            } else {
                activated.push(id);
                OutcomeStatus::Done
            };
            out.push(ActivationOutcome { face: id, outcome });
        }
        for id in &activated {
            st.active.insert(
                *id,
                ActiveFace {
                    face: *id,
                    mode: req.mode,
                    since: MOCK_NOW.into(),
                    document: None,
                },
            );
        }
        if !activated.is_empty() {
            st.emit(Event::ActivationChanged {
                activated,
                deactivated: Vec::new(),
            });
        }
        Ok(out)
    }

    fn deactivate(&self, req: DeactivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
        let mut st = self.state();
        let ids = match &req.target {
            Some(t) => st.target_faces(t)?,
            None => {
                let mut all: Vec<FaceId> = st.active.keys().copied().collect();
                all.sort();
                all
            }
        };
        let mut out = Vec::new();
        let mut deactivated = Vec::new();
        for id in ids {
            let outcome = if st.active.remove(&id).is_some() {
                deactivated.push(id);
                OutcomeStatus::Done
            } else {
                OutcomeStatus::Skipped {
                    reason: "The app did not activate this font.".into(),
                }
            };
            out.push(ActivationOutcome { face: id, outcome });
        }
        if !deactivated.is_empty() {
            st.emit(Event::ActivationChanged {
                activated: Vec::new(),
                deactivated,
            });
        }
        Ok(out)
    }

    fn list_active(&self) -> ApiResult<Vec<ActiveFace>> {
        let st = self.state();
        let mut all: Vec<ActiveFace> = st.active.values().cloned().collect();
        all.sort_by_key(|a| a.face);
        Ok(all)
    }

    fn conflicts(&self, target: Target) -> ApiResult<Vec<Conflict>> {
        let st = self.state();
        let mut out = Vec::new();
        for id in st.target_faces(&target)? {
            let f = st.face(id)?;
            if let Some(with) = st.conflict(f) {
                out.push(Conflict {
                    face: id,
                    postscript_name: f.detail.face.postscript_name.clone(),
                    with,
                });
            }
        }
        Ok(out)
    }
}

impl OrganizationService for MockApi {
    fn list_collections(&self) -> ApiResult<Vec<Collection>> {
        let st = self.state();
        Ok(st
            .collections
            .iter()
            .map(|c| st.collection_view(c))
            .collect())
    }

    fn create_collection(&self, req: CreateCollectionRequest) -> ApiResult<Collection> {
        let name = req.name.trim();
        if name.is_empty() {
            return Err(ApiError::invalid("Give the collection a name."));
        }
        let mut st = self.state();
        if let Some(p) = req.parent
            && !st.collections.iter().any(|c| c.id == p)
        {
            return Err(ApiError::not_found(format!("Collection {p}")));
        }
        if st
            .collections
            .iter()
            .any(|c| c.parent == req.parent && c.name == name)
        {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!("A collection named \"{name}\" already exists here."),
            ));
        }
        let c = Collection {
            id: CollectionId(st.next()),
            parent: req.parent,
            name: name.into(),
            rules: req.rules,
            face_count: 0,
        };
        st.collections.push(c.clone());
        Ok(st.collection_view(&c))
    }

    fn rename_collection(&self, req: RenameCollectionRequest) -> ApiResult<Collection> {
        let name = req.name.trim();
        if name.is_empty() {
            return Err(ApiError::invalid("Give the collection a name."));
        }
        let mut st = self.state();
        let c = st
            .collections
            .iter_mut()
            .find(|c| c.id == req.collection)
            .ok_or_else(|| ApiError::not_found(format!("Collection {}", req.collection)))?;
        c.name = name.into();
        let c = c.clone();
        Ok(st.collection_view(&c))
    }

    fn delete_collection(&self, collection: CollectionId) -> ApiResult<()> {
        let mut st = self.state();
        if !st.collections.iter().any(|c| c.id == collection) {
            return Err(ApiError::not_found(format!("Collection {collection}")));
        }
        let mut doomed = vec![collection];
        let mut i = 0;
        while let Some(&id) = doomed.get(i) {
            doomed.extend(
                st.collections
                    .iter()
                    .filter(|c| c.parent == Some(id))
                    .map(|c| c.id),
            );
            i += 1;
        }
        st.collections.retain(|c| !doomed.contains(&c.id));
        for id in doomed {
            st.members.remove(&id);
        }
        Ok(())
    }

    fn add_to_collection(&self, req: CollectionFacesRequest) -> ApiResult<Collection> {
        edit_members(self, req, true)
    }

    fn remove_from_collection(&self, req: CollectionFacesRequest) -> ApiResult<Collection> {
        edit_members(self, req, false)
    }

    fn list_tags(&self) -> ApiResult<Vec<Tag>> {
        let st = self.state();
        Ok(st.tags.iter().map(|t| tag_view(&st, t)).collect())
    }

    fn tag_faces(&self, req: TagFacesRequest) -> ApiResult<Vec<Tag>> {
        let mut st = self.state();
        for id in &req.faces {
            st.face(*id)?;
        }
        let mut touched = Vec::new();
        for name in req.tags.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            let id = match st.tags.iter().find(|t| t.name.eq_ignore_ascii_case(name)) {
                Some(t) => t.id,
                None => {
                    let id = TagId(st.next());
                    st.tags.push(Tag {
                        id,
                        name: name.into(),
                        face_count: 0,
                    });
                    id
                }
            };
            for face in &req.faces {
                st.face_tags.entry(*face).or_default().insert(id);
            }
            touched.push(id);
        }
        st.emit(Event::LibraryChanged {
            source: None,
            added: Vec::new(),
            removed: Vec::new(),
            updated: req.faces.clone(),
        });
        Ok(touched
            .iter()
            .filter_map(|id| st.tags.iter().find(|t| t.id == *id))
            .map(|t| tag_view(&st, t))
            .collect())
    }

    fn untag_faces(&self, req: UntagFacesRequest) -> ApiResult<()> {
        let mut st = self.state();
        for face in &req.faces {
            if let Some(set) = st.face_tags.get_mut(face) {
                for t in &req.tags {
                    set.remove(t);
                }
            }
        }
        st.emit(Event::LibraryChanged {
            source: None,
            added: Vec::new(),
            removed: Vec::new(),
            updated: req.faces,
        });
        Ok(())
    }

    fn update_user_meta(&self, req: UserMetaUpdate) -> ApiResult<()> {
        if req.rating.is_some_and(|r| r > 5) {
            return Err(ApiError::invalid(
                "A rating is 1 to 5 stars, or 0 to clear it.",
            ));
        }
        let mut st = self.state();
        for id in &req.faces {
            st.face(*id)?;
        }
        for f in st
            .faces
            .iter_mut()
            .filter(|f| req.faces.contains(&f.detail.face.id))
        {
            if let Some(v) = req.favorite {
                f.detail.favorite = v;
            }
            if let Some(r) = req.rating {
                f.detail.rating = (r > 0).then_some(r);
            }
            if let Some(n) = &req.notes {
                f.detail.notes = (!n.is_empty()).then(|| n.clone());
            }
        }
        st.emit(Event::LibraryChanged {
            source: None,
            added: Vec::new(),
            removed: Vec::new(),
            updated: req.faces,
        });
        Ok(())
    }

    fn list_sets(&self) -> ApiResult<Vec<ProjectSet>> {
        Ok(self.state().sets.clone())
    }

    fn create_set(&self, req: CreateSetRequest) -> ApiResult<ProjectSet> {
        let name = req.name.trim();
        if name.is_empty() {
            return Err(ApiError::invalid("Give the set a name."));
        }
        let mut st = self.state();
        for id in &req.faces {
            st.face(*id)?;
        }
        let set = ProjectSet {
            id: SetId(st.next()),
            name: name.into(),
            faces: req.faces,
        };
        st.sets.push(set.clone());
        Ok(set)
    }

    fn delete_set(&self, set: SetId) -> ApiResult<()> {
        let mut st = self.state();
        let before = st.sets.len();
        st.sets.retain(|s| s.id != set);
        if st.sets.len() == before {
            return Err(ApiError::not_found(format!("Set {set}")));
        }
        Ok(())
    }

    fn export_set(&self, set: SetId) -> ApiResult<SetExport> {
        let st = self.state();
        let s = st
            .sets
            .iter()
            .find(|s| s.id == set)
            .ok_or_else(|| ApiError::not_found(format!("Set {set}")))?;
        let fonts = s
            .faces
            .iter()
            .map(|id| {
                st.face(*id).map(|f| SetEntry {
                    family: f.detail.face.family_name.clone(),
                    style: f.detail.face.style.clone(),
                    hash: f.detail.hash.clone(),
                })
            })
            .collect::<ApiResult<_>>()?;
        Ok(SetExport {
            name: s.name.clone(),
            fonts,
        })
    }

    fn import_set(&self, export: SetExport) -> ApiResult<ImportSetResponse> {
        let mut st = self.state();
        let mut faces = Vec::new();
        let mut missing = Vec::new();
        for entry in export.fonts {
            match st.faces.iter().find(|f| f.detail.hash == entry.hash) {
                Some(f) => faces.push(f.detail.face.id),
                None => missing.push(entry),
            }
        }
        let set = ProjectSet {
            id: SetId(st.next()),
            name: export.name,
            faces,
        };
        st.sets.push(set.clone());
        Ok(ImportSetResponse { set, missing })
    }
}

fn edit_members(api: &MockApi, req: CollectionFacesRequest, add: bool) -> ApiResult<Collection> {
    let mut st = api.state();
    let c = st
        .collections
        .iter()
        .find(|c| c.id == req.collection)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("Collection {}", req.collection)))?;
    if c.rules.is_some() {
        return Err(ApiError::invalid(
            "A smart collection fills itself from its rules.",
        ));
    }
    for id in &req.faces {
        st.face(*id)?;
    }
    let members = st.members.entry(c.id).or_default();
    for id in req.faces {
        if add {
            members.insert(id);
        } else {
            members.remove(&id);
        }
    }
    Ok(st.collection_view(&c))
}

fn tag_view(st: &State, t: &Tag) -> Tag {
    let mut t = t.clone();
    t.face_count = st.face_tags.values().filter(|s| s.contains(&t.id)).count() as u32;
    t
}

impl ExportService for MockApi {
    fn export_web(&self, req: ExportWebRequest) -> ApiResult<ExportWebResponse> {
        if req.faces.is_empty() || req.formats.is_empty() {
            return Err(ApiError::invalid("Pick at least one font and one format."));
        }
        if req.out_dir.trim().is_empty() {
            return Err(ApiError::invalid("Choose an output folder."));
        }
        let st = self.state();
        let dir = req.out_dir.trim_end_matches(['/', '\\']);
        let mut files = Vec::new();
        let mut css = String::new();
        for id in &req.faces {
            let f = st.face(*id)?;
            let lic = &f.detail.license;
            if lic.embedding == Embedding::Restricted && lic.declared != Some(LicenseType::Web) {
                return Err(ApiError::new(
                    ErrorCode::PermissionDenied,
                    format!(
                        "{} {} does not allow web embedding. Declare a web license to export it.",
                        f.detail.face.family_name, f.detail.face.style
                    ),
                ));
            }
            let s = &f.detail.face;
            let mut srcs = Vec::new();
            for fmt in &req.formats {
                let ext = match fmt {
                    WebFormat::Woff2 => "woff2",
                    WebFormat::Woff => "woff",
                };
                let file = format!("{}.{ext}", s.postscript_name);
                files.push(format!("{dir}/{file}"));
                srcs.push(format!("url(\"{file}\") format(\"{ext}\")"));
            }
            css.push_str(&format!(
                "@font-face {{\n  font-family: \"{}\";\n  font-style: {};\n  font-weight: {};\n  font-display: swap;\n  src: {};\n}}\n",
                s.family_name,
                if s.slant == Slant::Upright { "normal" } else { "italic" },
                if s.is_variable { "100 900".to_string() } else { s.weight.to_string() },
                srcs.join(", "),
            ));
        }
        if req.css {
            files.push(format!("{dir}/fonts.css"));
        }
        Ok(ExportWebResponse {
            files,
            css: req.css.then_some(css),
        })
    }

    fn license_inventory(&self, req: LicenseInventoryRequest) -> ApiResult<LicenseInventory> {
        let st = self.state();
        if let Some(c) = req.collection
            && !st.collections.iter().any(|x| x.id == c)
        {
            return Err(ApiError::not_found(format!("Collection {c}")));
        }
        let mut csv = String::from("family,foundry,styles,open_license,embedding,declared\n");
        let mut rows = 0;
        for fam in &st.families {
            let faces: Vec<_> = st
                .family_faces(fam.id)
                .filter(|f| req.collection.is_none_or(|c| st.in_collection(f, c, 0)))
                .collect();
            let Some(first) = faces.first() else { continue };
            let lic = &first.detail.license;
            csv.push_str(&format!(
                "{},{},{},{},{:?},{}\n",
                fam.name,
                fam.foundry,
                faces.len(),
                lic.open_license.map_or(String::new(), |l| format!("{l:?}")),
                lic.embedding,
                lic.declared.map_or(String::new(), |d| format!("{d:?}")),
            ));
            rows += 1;
        }
        Ok(LicenseInventory { csv, rows })
    }
}

impl WorkflowService for MockApi {
    fn pick_fonts_for_brief(&self, req: BriefRequest) -> ApiResult<Shortlist> {
        let st = self.state();
        let target = Supports::Languages {
            codes: req.languages.clone(),
        };
        let mut candidates = Vec::new();
        for fam in &st.families {
            let Some(f) = st
                .family_faces(fam.id)
                .find(|f| f.detail.face.health == Health::Ok)
            else {
                continue;
            };
            if !req.languages.is_empty() && !st.coverage(f, &target)?.0.is_empty() {
                continue;
            }
            if let Some(l) = req.license
                && !(f.detail.license.open_license.is_some()
                    || f.detail.license.declared == Some(l))
            {
                continue;
            }
            let mut reasons = vec![format!("{:?} design", fam.classification).to_lowercase()];
            if !req.languages.is_empty() {
                reasons.push(format!("covers {}", req.languages.join(", ")));
            }
            if f.detail.license.open_license.is_some() {
                reasons.push("open license".into());
            }
            candidates.push(Candidate {
                family: st.family_summary(fam),
                reasons,
            });
        }
        candidates.truncate(req.limit.unwrap_or(5) as usize);
        Ok(Shortlist { candidates })
    }

    fn prepare_for_web(&self, req: PrepareForWebRequest) -> ApiResult<PrepareForWebResponse> {
        let faces = self
            .get_family(req.family)?
            .faces
            .iter()
            .map(|f| f.id)
            .collect::<Vec<_>>();
        let export = self.export_web(ExportWebRequest {
            faces: faces.clone(),
            formats: vec![WebFormat::Woff2],
            subset: req.subset,
            out_dir: req.out_dir,
            css: true,
        })?;
        let activated = self.activate(ActivateRequest {
            target: Target::Faces { faces },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })?;
        Ok(PrepareForWebResponse { activated, export })
    }
}

impl SettingsService for MockApi {
    fn get_settings(&self) -> ApiResult<Settings> {
        Ok(self.state().settings)
    }

    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings> {
        let mut st = self.state();
        st.settings = st.settings.apply(&req)?;
        Ok(st.settings)
    }

    fn app_folders(&self) -> ApiResult<AppFolders> {
        Ok(AppFolders {
            config: "/home/demo/.config/font-manager".into(),
            data: "/home/demo/.local/share/font-manager".into(),
            cache: "/home/demo/.cache/font-manager".into(),
            logs: "/home/demo/.local/state/font-manager/logs".into(),
        })
    }
}

impl EventSource for MockApi {
    fn subscribe(&self) -> mpsc::Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        self.state().subscribers.push(tx);
        rx
    }
}
