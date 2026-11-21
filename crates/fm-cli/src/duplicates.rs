//! `fm duplicates` (E56.9): the Duplicates module on the command line. Each
//! subcommand maps its flags to one `fm-api` request, pages through the
//! answer or waits for the apply job, and prints it. No rule is written
//! here (rule 1): the sets, the keeper and its reasons, the plan and its
//! counts ([`CleanupPlan::summary`]) are the API's; the text says the
//! sentences the API carries.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use fm_api::cleanup::{
    ApplyCleanupRequest, CleanupAction, CleanupPlan, CleanupScope, CleanupSession,
    PlanCleanupRequest, RemoveAs, StayReason, UndoCleanupRequest,
};
use fm_api::common::{Health, Page};
use fm_api::library::{
    CompareCopiesRequest, CopyComparison, DuplicateLevel, DuplicateScope, DuplicateSet,
    DuplicateSetFilter, FolderPair, FolderPairSide, ListDuplicateSetsRequest,
    ListFolderPairsRequest, TrashMoveStatus,
};
use fm_api::{Api, ApiError, ErrorCode, Event, ResolutionId, SourceId};

use crate::output::{Failure, json_name};
use crate::{Exit, resolve};

/// Rows asked for per page.
const PAGE: u32 = 500;

/// The `fm duplicates` commands.
#[derive(Debug, Subcommand)]
pub enum DuplicatesCommand {
    /// List the duplicate sets: identical files, the same version in files
    /// that differ, and other versions of one font, each with the copy the
    /// keeper policy keeps and why.
    List {
        #[command(flatten)]
        scope: ScopeFlags,
        /// Only sets of this level (repeat for several).
        #[arg(long, value_enum)]
        level: Vec<Level>,
    },
    /// List pairs of folders that hold copies of the same fonts.
    Folders {
        #[command(flatten)]
        scope: ScopeFlags,
        /// The fewest shared font files a pair needs to be listed (5 by
        /// default).
        #[arg(long, value_name = "N")]
        min_shared: Option<u32>,
    },
    /// Show what differs between two copies: languages, glyphs, features,
    /// axes and metrics. A copy is a face id (`fm duplicates list` shows
    /// them) or a font name.
    Compare {
        /// The first copy.
        a: String,
        /// The second copy; the differences read from the first to it.
        b: String,
    },
    /// Plan a cleanup: per file, whether it goes to the trash, is hidden or
    /// stays, and why. Changes nothing; save it with --out and apply it with
    /// `fm duplicates apply`.
    Plan {
        #[command(flatten)]
        scope: PlanScopeFlags,
        /// Hide the extra copies instead: their files stay where they are
        /// and leave the library.
        #[arg(long)]
        hide: bool,
        /// Hand the activation of an active identical copy to the kept copy,
        /// so the active copy can leave too.
        #[arg(long)]
        move_activations: bool,
        /// Write the plan as JSON to this file.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Replace the file when it exists.
        #[arg(long, requires = "out")]
        force: bool,
    },
    /// Apply a plan saved by `fm duplicates plan --out`. Prints what it
    /// will do and asks first. Files go to the OS trash or are hidden; none
    /// is deleted for good, and `fm duplicates undo` puts them back.
    Apply {
        /// The plan file.
        plan: PathBuf,
        /// Do not ask.
        #[arg(long)]
        yes: bool,
        /// The session's name; one from the date by default.
        #[arg(long)]
        name: Option<String>,
    },
    /// List the cleanup sessions, newest first, with their ids for undo.
    Sessions,
    /// Put the files of a session back, all of them or the ones named.
    Undo {
        /// The session id (`fm duplicates sessions`).
        session: u32,
        /// Only this file (repeat for several).
        #[arg(long, value_name = "FILE")]
        path: Vec<PathBuf>,
    },
}

/// A level of sameness, as the command line names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Level {
    /// The same file at different paths.
    Identical,
    /// One font and one version in files that differ.
    SameVersion,
    /// One font in files whose versions differ.
    OtherVersion,
}

impl From<Level> for DuplicateLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Identical => Self::Identical,
            Level::SameVersion => Self::SameVersion,
            Level::OtherVersion => Self::OtherVersion,
        }
    }
}

/// Which files a listing looks at; the whole library by default.
#[derive(Debug, Clone, Default, Args)]
pub struct ScopeFlags {
    /// Only sets with a copy in this source (its id).
    #[arg(long, value_name = "ID", conflicts_with = "folder")]
    pub source: Option<u32>,
    /// Only sets with a copy in this folder or its subfolders.
    #[arg(long, value_name = "DIR")]
    pub folder: Option<PathBuf>,
}

impl ScopeFlags {
    fn scope(&self) -> Result<DuplicateScope, ApiError> {
        Ok(match (&self.source, &self.folder) {
            (Some(id), _) => DuplicateScope::Source {
                source: SourceId(*id),
            },
            (_, Some(dir)) => DuplicateScope::Folder {
                path: absolute(dir)?,
            },
            _ => DuplicateScope::Library,
        })
    }
}

/// Which sets a plan covers; the whole library by default.
#[derive(Debug, Clone, Default, Args)]
pub struct PlanScopeFlags {
    /// Only the sets with a copy in this source (its id); only its copies
    /// can leave.
    #[arg(long, value_name = "ID", conflicts_with_all = ["folder", "set", "keep_folder"])]
    pub source: Option<u32>,
    /// Only the sets with a copy in this folder; only its copies can leave.
    #[arg(long, value_name = "DIR", conflicts_with_all = ["set", "keep_folder"])]
    pub folder: Option<PathBuf>,
    /// Only this set, by its id (repeat for several).
    #[arg(long, value_name = "SET", conflicts_with = "keep_folder")]
    pub set: Vec<String>,
    /// A folder pair: keep this folder's copies.
    #[arg(long, value_name = "DIR", requires = "remove_folder")]
    pub keep_folder: Option<PathBuf>,
    /// A folder pair: remove this folder's copies of the files the kept
    /// folder holds.
    #[arg(long, value_name = "DIR", requires = "keep_folder")]
    pub remove_folder: Option<PathBuf>,
    /// A folder pair: also plan copies of the same or another version,
    /// through the keeper policy; identical copies only by default.
    #[arg(long, requires = "keep_folder")]
    pub other_levels: bool,
}

impl PlanScopeFlags {
    fn scope(&self) -> Result<CleanupScope, ApiError> {
        if let (Some(keep), Some(remove)) = (&self.keep_folder, &self.remove_folder) {
            return Ok(CleanupScope::FolderPair {
                keep: absolute(keep)?,
                remove: absolute(remove)?,
                other_levels: self.other_levels,
            });
        }
        if !self.set.is_empty() {
            return Ok(CleanupScope::Sets {
                ids: self.set.clone(),
            });
        }
        Ok(match (&self.source, &self.folder) {
            (Some(id), _) => CleanupScope::Source {
                source: SourceId(*id),
            },
            (_, Some(dir)) => CleanupScope::Folder {
                path: absolute(dir)?,
            },
            _ => CleanupScope::Library,
        })
    }
}

/// A path the user gave, absolute against `fm`'s working folder (the app
/// compares absolute paths) and valid Unicode.
fn absolute(path: &Path) -> Result<String, ApiError> {
    let path = std::path::absolute(path).map_err(|e| {
        ApiError::new(ErrorCode::Io, "The path could not be found.").with_detail(e.to_string())
    })?;
    path.to_str().map(str::to_owned).ok_or_else(|| {
        ApiError::new(
            ErrorCode::InvalidRequest,
            "The path is not valid Unicode. Choose another path.",
        )
        .with_detail(path.to_string_lossy().into_owned())
    })
}

/// `fm duplicates list`: every set of the scope at the levels asked for.
pub fn list(
    api: &dyn Api,
    scope: &ScopeFlags,
    levels: &[Level],
) -> Result<Vec<DuplicateSet>, Failure> {
    let scope = scope.scope()?;
    let filter = DuplicateSetFilter {
        levels: levels.iter().copied().map(Into::into).collect(),
        ..DuplicateSetFilter::default()
    };
    let mut sets = Vec::new();
    loop {
        let page = api.list_duplicate_sets(ListDuplicateSetsRequest {
            scope: scope.clone(),
            page: Page {
                offset: count(sets.len()),
                limit: PAGE,
            },
            filter: filter.clone(),
            ..ListDuplicateSetsRequest::default()
        })?;
        let last = page.items.is_empty();
        sets.extend(page.items);
        if last || count(sets.len()) >= page.total {
            return Ok(sets);
        }
    }
}

/// `fm duplicates folders`: every folder pair of the scope.
pub fn folders(
    api: &dyn Api,
    scope: &ScopeFlags,
    min_shared: Option<u32>,
) -> Result<Vec<FolderPair>, Failure> {
    let scope = scope.scope()?;
    let mut pairs = Vec::new();
    loop {
        let page = api.list_folder_pairs(ListFolderPairsRequest {
            scope: scope.clone(),
            min_shared,
            page: Page {
                offset: count(pairs.len()),
                limit: PAGE,
            },
        })?;
        let last = page.items.is_empty();
        pairs.extend(page.items);
        if last || count(pairs.len()) >= page.total {
            return Ok(pairs);
        }
    }
}

/// `fm duplicates compare <a> <b>`: the two copies side by side.
pub fn compare(api: &dyn Api, a: &str, b: &str) -> Result<CopyComparison, Failure> {
    let a = resolve(api, a)?.face;
    let b = resolve(api, b)?.face;
    Ok(api.compare_copies(CompareCopiesRequest { a, b })?)
}

/// The `fm duplicates plan` settings.
#[derive(Debug, Clone, Default)]
pub struct PlanFlags {
    /// The sets to plan.
    pub scope: PlanScopeFlags,
    /// Hide instead of the trash.
    pub hide: bool,
    /// Move activations of identical copies.
    pub move_activations: bool,
}

/// `fm duplicates plan`: the plan, also written to `out` when given (a new
/// file, or any with `force`).
pub fn plan(
    api: &dyn Api,
    flags: &PlanFlags,
    out: Option<&Path>,
    force: bool,
) -> Result<CleanupPlan, Failure> {
    if let Some(path) = out
        && !force
        && std::fs::symlink_metadata(path).is_ok()
    {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!(
                "{} already exists. Use --force to replace it.",
                path.display()
            ),
        )
        .into());
    }
    let plan = api.plan_cleanup(PlanCleanupRequest {
        scope: flags.scope.scope()?,
        keepers: Vec::new(),
        remove_as: if flags.hide {
            RemoveAs::Hide
        } else {
            RemoveAs::Trash
        },
        move_activations: flags.move_activations,
    })?;
    if let Some(path) = out {
        let json = serde_json::to_string_pretty(&plan).map_err(|e| {
            ApiError::new(
                ErrorCode::Internal,
                "The plan could not be written as JSON.",
            )
            .with_detail(e.to_string())
        })?;
        std::fs::write(path, json + "\n").map_err(|e| {
            ApiError::new(
                ErrorCode::Io,
                format!("The plan could not be written to {}.", path.display()),
            )
            .with_detail(e.to_string())
        })?;
    }
    Ok(plan)
}

/// The plan in `file`, as `fm duplicates plan --out` wrote it.
pub fn read_plan(file: &Path) -> Result<CleanupPlan, Failure> {
    let text = std::fs::read_to_string(file).map_err(|e| {
        let code = if e.kind() == io::ErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::Io
        };
        ApiError::new(code, format!("{} could not be read.", file.display()))
            .with_detail(e.to_string())
    })?;
    let plan: CleanupPlan = serde_json::from_str(&text).map_err(|e| {
        ApiError::new(
            ErrorCode::InvalidRequest,
            format!("{} is not a cleanup plan made by Font Fu.", file.display()),
        )
        .with_detail(e.to_string())
    })?;
    plan.check()?;
    Ok(plan)
}

/// Why `fm duplicates apply --json` needs `--yes`: it prints one JSON value
/// and cannot ask.
pub fn needs_yes() -> Failure {
    ApiError::new(
        ErrorCode::InvalidRequest,
        "With --json, fm duplicates apply cannot ask. Add --yes to apply the plan.",
    )
    .into()
}

/// Prints the plan's summary and the question to `out`, and reads the
/// answer from `input`: true for `y` or `yes`. No answer (the end of the
/// input) is no.
pub fn ask(plan: &CleanupPlan, input: &mut dyn BufRead, out: &mut dyn Write) -> io::Result<bool> {
    summary(plan, out)?;
    let what = if plan.leaving().next().is_none() {
        "Nothing leaves the library."
    } else {
        "Files go to the trash or are hidden; none is deleted for good, and \
         fm duplicates undo puts them back."
    };
    write!(out, "{what}\nApply this plan? [y/N] ")?;
    out.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

/// `fm duplicates apply`: applies `plan` as one session and waits for the
/// job; progress goes to `progress`. The session as the API lists it.
pub fn apply(
    api: &dyn Api,
    plan: CleanupPlan,
    name: Option<String>,
    mut progress: Option<&mut dyn Write>,
) -> Result<CleanupSession, Failure> {
    // Subscribed first: the job's events may come before `apply_cleanup` returns.
    let events = api.subscribe();
    let job = api.apply_cleanup(ApplyCleanupRequest {
        plan,
        name,
        choices: Vec::new(),
    })?;
    loop {
        let event = events.recv().map_err(|_| {
            ApiError::new(
                ErrorCode::Internal,
                "The cleanup stopped without finishing.",
            )
        })?;
        match event {
            Event::CleanupProgress {
                job: j,
                session,
                done,
                total,
                finished,
                ..
            } if j == job => {
                if let Some(out) = progress.as_mut()
                    && !finished
                {
                    // Progress that cannot be written does not stop the cleanup.
                    let _ = writeln!(out, "Cleaning up: {done} of {total} files");
                }
                if finished {
                    let id = session.ok_or_else(|| {
                        ApiError::new(ErrorCode::Internal, "The cleanup could not start.")
                    })?;
                    return Ok(session_by_id(api, id)?);
                }
            }
            Event::JobFailed { job: j, error } if j == job => return Err(error.into()),
            _ => {}
        }
    }
}

fn session_by_id(api: &dyn Api, id: ResolutionId) -> Result<CleanupSession, ApiError> {
    api.list_cleanup_sessions()?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| ApiError::not_found(format!("Session {id}")))
}

/// `fm duplicates undo <session>`: puts the session's files back, or the
/// ones named.
pub fn undo(api: &dyn Api, session: u32, paths: &[PathBuf]) -> Result<CleanupSession, Failure> {
    let paths = paths
        .iter()
        .map(|p| absolute(p))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(api.undo_cleanup_session(UndoCleanupRequest {
        session: ResolutionId(session),
        paths,
    })?)
}

/// The exit code of an apply or an undo: 0 when every file did what was
/// asked, 4 when a file was skipped or failed.
pub fn session_exit(session: &CleanupSession) -> Exit {
    let refused = session
        .files
        .iter()
        .any(|f| matches!(f.status, TrashMoveStatus::Skipped | TrashMoveStatus::Failed));
    if refused {
        Exit::Refused
    } else {
        Exit::Success
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// A byte count in words: `1.2 MB`, `340 KB`, `12 bytes`.
fn size(bytes: u32) -> String {
    let b = f64::from(bytes);
    if b >= 1_000_000.0 {
        format!("{:.1} MB", b / 1_000_000.0)
    } else if b >= 1_000.0 {
        format!("{:.0} KB", b / 1_000.0)
    } else {
        format!("{bytes} bytes")
    }
}

/// `fm duplicates list`: each set, its copies with the keeper policy's
/// answer, and the sentence the API carries for each copy.
pub fn print_sets(sets: &[DuplicateSet], out: &mut dyn Write) -> io::Result<()> {
    if sets.is_empty() {
        return writeln!(out, "No duplicate sets.");
    }
    writeln!(out, "Duplicate sets: {}", sets.len())?;
    for set in sets {
        let name = set.postscript_name.as_deref().unwrap_or("(no name)");
        writeln!(out)?;
        writeln!(out, "{} {name} ({})", json_name(&set.level), set.id)?;
        let rec = set.recommendation.as_ref();
        for copy in &set.copies {
            let kept = rec.and_then(|r| r.keep.iter().find(|k| k.path == copy.path));
            let removed = rec.and_then(|r| r.remove.iter().find(|k| k.path == copy.path));
            let mark = match (kept, removed) {
                (Some(_), _) => "keep  ",
                (_, Some(_)) => "remove",
                _ => "      ",
            };
            let face = copy.face.map_or_else(|| "-".to_owned(), |f| f.to_string());
            let version = copy.version.as_deref().unwrap_or("no version");
            let system = if copy.system { "  (system)" } else { "" };
            writeln!(out, "  {mark} {face:>6}  {version}  {}{system}", copy.path)?;
            // A removed copy's reason is the keeper's win over it; it is
            // said once, under the keeper, when it is that same sentence.
            let said_above = |s: &str| rec.is_some_and(|r| r.keep.iter().any(|k| k.sentence == s));
            let sentence = kept.map(|k| k.sentence.as_str()).or(removed
                .map(|r| r.reason.sentence.as_str())
                .filter(|s| !said_above(s)));
            if let Some(sentence) = sentence.filter(|s| !s.is_empty()) {
                writeln!(out, "                 {sentence}")?;
            }
        }
    }
    Ok(())
}

/// `fm duplicates folders`: each pair with both sides counted.
pub fn print_folders(pairs: &[FolderPair], out: &mut dyn Write) -> io::Result<()> {
    if pairs.is_empty() {
        return writeln!(out, "No folders hold the same fonts.");
    }
    for (i, pair) in pairs.iter().enumerate() {
        if i > 0 {
            writeln!(out)?;
        }
        writeln!(out, "Frees {} when one side goes", size(pair.frees))?;
        side(&pair.a, out)?;
        side(&pair.b, out)?;
    }
    Ok(())
}

fn side(s: &FolderPairSide, out: &mut dyn Write) -> io::Result<()> {
    let contained = if s.contained {
        ", every file is on the other side"
    } else {
        ""
    };
    writeln!(out, "  {}", s.path)?;
    writeln!(
        out,
        "    {} font files ({}): {} identical, {} same version, {} other version, {} only here{contained}",
        s.font_files,
        size(s.size),
        s.shared.identical,
        s.shared.same_version,
        s.shared.other_version,
        s.only_here,
    )
}

/// `fm duplicates compare`: both copies and the API's sentences.
pub fn print_comparison(c: &CopyComparison, out: &mut dyn Write) -> io::Result<()> {
    for (label, copy) in [("A", &c.a), ("B", &c.b)] {
        let health = match &copy.health {
            Health::Ok => String::new(),
            Health::Broken(why) | Health::Unsupported(why) => format!("  ({why})"),
        };
        writeln!(
            out,
            "{label}  {:>6}  {}{health}",
            copy.face.to_string(),
            copy.path
        )?;
    }
    for sentence in &c.summary {
        writeln!(out, "{sentence}")?;
    }
    Ok(())
}

/// `fm duplicates plan`: each file with what happens and why, then the
/// summary.
pub fn print_plan(plan: &CleanupPlan, out: &mut dyn Write) -> io::Result<()> {
    for f in &plan.files {
        writeln!(out, "{:<6} {}", json_name(&f.action), f.path)?;
        if !f.sentence.is_empty() {
            writeln!(out, "       {}", f.sentence)?;
        }
    }
    if !plan.files.is_empty() {
        writeln!(out)?;
    }
    summary(plan, out)
}

/// `fm duplicates plan --out`: where the plan went, the summary, and how to
/// apply it.
pub fn print_plan_written(plan: &CleanupPlan, path: &Path, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "Wrote the plan to {}", path.display())?;
    summary(plan, out)?;
    writeln!(out, "Apply it with: fm duplicates apply {}", path.display())
}

/// What applying `plan` does: the files to the trash and the space they
/// free, the hidden files, and every file that stays for another reason
/// than being its set's keeper, with the API's sentence.
pub fn summary(plan: &CleanupPlan, out: &mut dyn Write) -> io::Result<()> {
    let s = plan.summary();
    writeln!(out, "Sets: {}", s.sets)?;
    writeln!(
        out,
        "To the trash: {} files, frees {}",
        s.trash,
        size(s.frees)
    )?;
    writeln!(out, "Hidden: {} files", s.hide)?;
    writeln!(out, "Kept: {} files", s.kept)?;
    let skipped: u32 = s.skipped.iter().map(|c| c.files).sum();
    writeln!(out, "Skipped: {skipped} files")?;
    for f in plan.files.iter().filter(|f| {
        f.action == CleanupAction::Stays && !matches!(f.reason, None | Some(StayReason::Keeper))
    }) {
        writeln!(out, "  {}: {}", f.path, f.sentence)?;
    }
    if !plan.conflicts.is_empty() {
        writeln!(
            out,
            "Metadata that differs between copies: {}; the kept copy's value is taken (choose in the app).",
            plan.conflicts.len()
        )?;
    }
    Ok(())
}

/// `fm duplicates sessions`.
pub fn print_sessions(sessions: &[CleanupSession], out: &mut dyn Write) -> io::Result<()> {
    if sessions.is_empty() {
        return writeln!(out, "No cleanup sessions.");
    }
    let mut table = comfy_table::Table::new();
    table
        .load_preset(comfy_table::presets::NOTHING)
        .set_header([
            "ID", "NAME", "WHEN", "SETS", "TRASHED", "HIDDEN", "FREES", "STATE",
        ]);
    for s in sessions {
        table.add_row([
            s.id.to_string(),
            s.name.clone(),
            s.at.clone(),
            s.sets.to_string(),
            s.trashed.to_string(),
            s.hidden.to_string(),
            size(s.frees),
            state(s).into(),
        ]);
    }
    writeln!(out, "{table}")
}

fn state(s: &CleanupSession) -> &'static str {
    if s.running {
        "running"
    } else if s.undone {
        "undone"
    } else if s.cancelled {
        "cancelled"
    } else {
        "done"
    }
}

/// `fm duplicates apply` and `undo`: the session and each file's status.
pub fn print_session(s: &CleanupSession, out: &mut dyn Write) -> io::Result<()> {
    writeln!(
        out,
        "Session {} \"{}\": {} trashed, {} hidden, frees {} ({})",
        s.id,
        s.name,
        s.trashed,
        s.hidden,
        size(s.frees),
        state(s)
    )?;
    for f in &s.files {
        let how = match f.removed_as {
            RemoveAs::Hide => " (hide)",
            RemoveAs::Trash => "",
        };
        writeln!(out, "  {:<9} {}{how}", json_name(&f.status), f.path)?;
        if let Some(message) = &f.message {
            writeln!(out, "            {message}")?;
        }
    }
    writeln!(out, "Undo with: fm duplicates undo {}", s.id)
}

/// `fm duplicates apply` answered no.
pub fn print_declined(out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "Nothing was changed.")
}
