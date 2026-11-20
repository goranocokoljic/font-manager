//! The keeper policy (E56.4, ADR 0027): which copy of a duplicate set to
//! keep, by an ordered list of rules, with the language guard.
//!
//! - The rules rank the copies outside the system font folders. A copy that
//!   wins a rule wins the set; a tie goes to the next rule; the last rule is
//!   always the path A to Z, so the answer never depends on chance.
//! - The **language guard** is not a rule, and the order cannot turn it off:
//!   after the rules chose the keeper, the copies that cover one of my
//!   languages in full that the kept copies do not are kept too, until every
//!   language some copy covers is kept. A set in which no copy covers all of
//!   them is answered "keep both", each kept copy with the languages it alone
//!   has.
//! - A copy in a system font folder is outside the policy and always kept
//!   (rule 4). A copy whose file alone holds another font (a collection
//!   file) is kept, so the last copy of a font is never recommended for
//!   removal.
//!
//! Every recommendation carries the rule that decided and the values it
//! compared, with one sentence the UI, the CLI and MCP all show.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use fm_api::FaceId;
use fm_api::common::FontFormat;
use fm_api::library::{
    CopyFacts, DuplicateGroup, DuplicateSet, KeeperOutcome, KeeperPolicy, KeeperPolicyPreview,
    KeeperReason, KeeperRecommendation, KeeperRule, KeptBecause, KeptCopy, RemovedCopy, VersionKey,
    format_from_extension,
};
use fm_api::settings::MyLanguagesStatus;
use fm_platform::paths::{Os, is_within};

use crate::charsets;
use crate::duplicate_sets::SetFile;
use crate::error::FmResult;
use crate::my_languages;

/// One copy as the policy weighs it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Candidate {
    /// Absolute file path.
    pub path: String,
    /// Version string (name ID 5) of the face the set is about.
    pub version: Option<String>,
    /// The format the file's extension names, as
    /// [`DuplicateGroup::choose_keeper`] reads it, so the default policy
    /// with no language set gives its keeper; `None` for another extension.
    pub format: Option<FontFormat>,
    /// The source is the library folder.
    pub in_library_folder: bool,
    /// The file is in a system font folder: outside the policy, always kept.
    pub system: bool,
    /// What the index knows of the face; `None` reads as no languages, no
    /// glyph count, static, no metadata, not active.
    pub facts: Option<CopyFacts>,
    /// The fonts (PostScript names) this file holds, each with every file
    /// of the library that holds it, this one included.
    pub holds: Vec<HeldFont>,
}

/// One font a file holds, with the files that hold it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeldFont {
    /// PostScript name.
    pub name: String,
    /// Every file that holds it, by path.
    pub holders: Vec<String>,
}

/// The names of the language presets, for the sentences.
#[derive(Debug, Clone, Default)]
pub struct LanguageNames(HashMap<String, String>);

impl LanguageNames {
    /// The names of every preset; nothing is read when `my_languages` is
    /// empty, as no sentence then names a language.
    pub fn load(my_languages: &[String]) -> FmResult<Self> {
        if my_languages.is_empty() {
            return Ok(Self::default());
        }
        Ok(Self(
            charsets::load()?
                .into_iter()
                .map(|c| (c.code, c.name))
                .collect(),
        ))
    }

    fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.0.get(id).map_or(id, String::as_str)
    }

    /// `ids` by name: "A", "A and B", "A, B and C".
    fn list(&self, ids: &[String]) -> String {
        words(ids.iter().map(|id| self.name(id).to_owned()).collect())
    }
}

/// `items` joined as a sentence lists them.
fn words(items: Vec<String>) -> String {
    match items.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// Whether the policy reads the facts of the copies (the index's
/// `copy_facts`): a language is set, or a rule weighs a fact.
pub fn needs_facts(policy: &KeeperPolicy, my_languages: &[String]) -> bool {
    !my_languages.is_empty()
        || policy.rules.iter().any(|r| {
            matches!(
                r,
                KeeperRule::Variable
                    | KeeperRule::MostGlyphs
                    | KeeperRule::UserMetadata
                    | KeeperRule::ActiveOrRequested
            )
        })
}

/// What one call weighs with: the normalized policy and my languages.
struct Ctx<'a> {
    policy: KeeperPolicy,
    languages: Vec<String>,
    names: &'a LanguageNames,
}

impl Ctx<'_> {
    /// My languages `c` covers in full, in my order.
    fn full(&self, c: &Candidate) -> Vec<String> {
        self.mine(c.facts.as_ref().map(|f| f.languages_full.as_slice()))
    }

    /// My languages `c` has `locl` forms for, in my order.
    fn locl(&self, c: &Candidate) -> Vec<String> {
        self.mine(c.facts.as_ref().map(|f| f.languages_locl.as_slice()))
    }

    fn mine(&self, ids: Option<&[String]>) -> Vec<String> {
        let ids = ids.unwrap_or_default();
        self.languages
            .iter()
            .filter(|l| ids.contains(l))
            .cloned()
            .collect()
    }

    /// Where `c` stands in the preferred folders: 0 is best; outside them
    /// all, their count.
    fn folder_rank(&self, c: &Candidate) -> usize {
        let folders = &self.policy.preferred_folders;
        folders
            .iter()
            .position(|f| is_within(Os::current(), Path::new(&c.path), Path::new(f)))
            .unwrap_or(folders.len())
    }

    /// `a` against `b` by one rule: `Less` when `a` wins it.
    fn compare(&self, rule: KeeperRule, a: &Candidate, b: &Candidate) -> Ordering {
        match rule {
            KeeperRule::MyLanguages => self.full(b).len().cmp(&self.full(a).len()),
            KeeperRule::MyLanguagesLocl => self.locl(b).len().cmp(&self.locl(a).len()),
            KeeperRule::NewestVersion => {
                let key = |c: &Candidate| {
                    let v = VersionKey::of(c.version.as_deref());
                    (v.is_none(), std::cmp::Reverse(v))
                };
                key(a).cmp(&key(b))
            }
            KeeperRule::Format => self
                .policy
                .format_rank(a.format)
                .cmp(&self.policy.format_rank(b.format)),
            KeeperRule::Variable => variable(b).cmp(&variable(a)),
            KeeperRule::MostGlyphs => {
                let key = |c: &Candidate| {
                    let n = glyphs(c);
                    (n.is_none(), std::cmp::Reverse(n))
                };
                key(a).cmp(&key(b))
            }
            KeeperRule::PreferredFolder => self.folder_rank(a).cmp(&self.folder_rank(b)),
            KeeperRule::LibraryFolder => b.in_library_folder.cmp(&a.in_library_folder),
            KeeperRule::UserMetadata => metadata(b).len().cmp(&metadata(a).len()),
            KeeperRule::ActiveOrRequested => active(b).is_some().cmp(&active(a).is_some()),
            KeeperRule::Path => a.path.cmp(&b.path),
        }
    }

    /// `a` against `b` by every rule in order.
    fn rank(&self, a: &Candidate, b: &Candidate) -> Ordering {
        self.policy
            .rules
            .iter()
            .map(|&r| self.compare(r, a, b))
            .find(|o| o.is_ne())
            .unwrap_or(Ordering::Equal)
    }

    /// Why `kept` stands before `other`: the first rule that tells them
    /// apart, its values and its sentence.
    fn reason(&self, kept: &Candidate, other: &Candidate) -> KeeperReason {
        let rule = self
            .policy
            .rules
            .iter()
            .copied()
            .find(|&r| self.compare(r, kept, other).is_lt())
            .unwrap_or(KeeperRule::Path);
        let (k, o, sentence) = match rule {
            KeeperRule::MyLanguages | KeeperRule::MyLanguagesLocl => {
                let of = |c| {
                    if rule == KeeperRule::MyLanguages {
                        self.full(c)
                    } else {
                        self.locl(c)
                    }
                };
                let (k, o) = (of(kept), of(other));
                let only: Vec<String> = k.iter().filter(|l| !o.contains(l)).cloned().collect();
                let what = if rule == KeeperRule::MyLanguages {
                    "covers"
                } else {
                    "has local forms (locl) for"
                };
                let sentence = format!(
                    "Kept: {what} {}, the other copy does not.",
                    self.names.list(&only)
                );
                (k, o, sentence)
            }
            KeeperRule::NewestVersion => {
                let v = |c: &Candidate| {
                    c.version
                        .as_deref()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_owned)
                };
                let (k, o) = (v(kept), v(other));
                let theirs = o
                    .as_deref()
                    .map_or("no version".to_owned(), |o| format!("version {o}"));
                let sentence = format!(
                    "Kept: version {}, the other copy has {theirs}.",
                    k.as_deref().unwrap_or_default()
                );
                (k.into_iter().collect(), o.into_iter().collect(), sentence)
            }
            KeeperRule::Format => {
                let (k, o) = (format_name(kept.format), format_name(other.format));
                let sentence = format!("Kept: {k} comes before {o} in your order of formats.");
                (vec![k.to_owned()], vec![o.to_owned()], sentence)
            }
            KeeperRule::Variable => (
                vec!["variable".to_owned()],
                vec!["static".to_owned()],
                "Kept: a variable font, the other copy is static.".to_owned(),
            ),
            KeeperRule::MostGlyphs => {
                let k = glyphs(kept).unwrap_or_default();
                let sentence = match glyphs(other) {
                    Some(o) => format!("Kept: {k} glyphs, the other copy has {o}."),
                    None => format!("Kept: {k} glyphs, the other copy's are not counted yet."),
                };
                (
                    vec![k.to_string()],
                    glyphs(other).map(|n| n.to_string()).into_iter().collect(),
                    sentence,
                )
            }
            KeeperRule::PreferredFolder => {
                let folder = |c| {
                    self.policy
                        .preferred_folders
                        .get(self.folder_rank(c))
                        .cloned()
                };
                let k = folder(kept).unwrap_or_default();
                let sentence =
                    format!("Kept: in {k}, which comes first in your preferred folders.");
                (vec![k], folder(other).into_iter().collect(), sentence)
            }
            KeeperRule::LibraryFolder => (
                vec!["libraryFolder".to_owned()],
                Vec::new(),
                "Kept: in the library folder, the other copy is not.".to_owned(),
            ),
            KeeperRule::UserMetadata => {
                let (k, o) = (metadata(kept), metadata(other));
                let theirs = if o.is_empty() {
                    "none of it".to_owned()
                } else {
                    format!(
                        "only {}",
                        words(o.iter().map(|s| (*s).to_owned()).collect())
                    )
                };
                let sentence = format!(
                    "Kept: carries your {}, the other copy {theirs}.",
                    words(k.iter().map(|s| (*s).to_owned()).collect())
                );
                let own = |v: Vec<&str>| v.into_iter().map(str::to_owned).collect();
                (own(k), own(o), sentence)
            }
            KeeperRule::ActiveOrRequested => (
                active(kept).into_iter().map(str::to_owned).collect(),
                Vec::new(),
                "Kept: active or asked for by a document, the other copy is not.".to_owned(),
            ),
            KeeperRule::Path => (
                vec![kept.path.clone()],
                vec![other.path.clone()],
                "Kept: its path comes first from A to Z; nothing else tells the copies apart."
                    .to_owned(),
            ),
        };
        KeeperReason {
            rule,
            kept: k,
            other: o,
            other_path: other.path.clone(),
            sentence,
        }
    }
}

fn variable(c: &Candidate) -> bool {
    c.facts.as_ref().is_some_and(|f| f.variable)
}

fn glyphs(c: &Candidate) -> Option<u32> {
    c.facts.as_ref().and_then(|f| f.glyph_count)
}

/// The kinds of the user's metadata `c` carries.
fn metadata(c: &Candidate) -> Vec<&'static str> {
    let Some(f) = &c.facts else {
        return Vec::new();
    };
    [
        (f.favorite, "favorite"),
        (f.rating.is_some(), "rating"),
        (!f.tags.is_empty(), "tags"),
        (f.has_notes, "notes"),
        (!f.collections.is_empty(), "collections"),
        (!f.sets.is_empty(), "sets"),
    ]
    .into_iter()
    .filter_map(|(has, kind)| has.then_some(kind))
    .collect()
}

/// `active` when the app activated `c`, `requested` when only a document
/// asked for it, `None` otherwise.
fn active(c: &Candidate) -> Option<&'static str> {
    let f = c.facts.as_ref()?;
    if f.activation.is_some() {
        Some("active")
    } else if !f.activation_documents.is_empty() {
        Some("requested")
    } else {
        None
    }
}

/// A format as a sentence names it.
fn format_name(format: Option<FontFormat>) -> &'static str {
    match format {
        Some(FontFormat::Otf) => "OpenType CFF",
        Some(FontFormat::Ttf) => "TrueType",
        Some(FontFormat::Ttc) => "a TrueType collection",
        Some(FontFormat::Otc) => "an OpenType collection",
        Some(FontFormat::Woff) => "WOFF",
        Some(FontFormat::Woff2) => "WOFF2",
        Some(FontFormat::Type1) => "Type 1",
        Some(FontFormat::Dfont) => "a dfont",
        Some(FontFormat::Bitmap) => "a bitmap font",
        None => "a file that is no font",
    }
}

/// What `policy` and the language guard recommend for the copies of one
/// set. Every copy ends in exactly one of `keep` and `remove`.
pub fn recommend(
    copies: &[Candidate],
    policy: &KeeperPolicy,
    my_languages: &[String],
    names: &LanguageNames,
) -> KeeperRecommendation {
    let applied = my_languages::apply(my_languages, <[String]>::to_vec);
    let ctx = Ctx {
        policy: policy.normalized(),
        languages: applied.outcome.unwrap_or_default(),
        names,
    };
    let mut ranked: Vec<usize> = (0..copies.len()).filter(|&i| !copies[i].system).collect();
    ranked.sort_by(|&a, &b| ctx.rank(&copies[a], &copies[b]));
    let system_kept = || {
        copies
            .iter()
            .filter(|c| c.system)
            .map(|c| KeptCopy {
                path: c.path.clone(),
                because: KeptBecause::System,
                languages_only_here: Vec::new(),
                fonts_only_here: Vec::new(),
                sentence: "Kept: in a system font folder; the policy never removes it.".to_owned(),
            })
            .collect::<Vec<_>>()
    };
    let Some(&winner) = ranked.first() else {
        return KeeperRecommendation {
            outcome: KeeperOutcome::AllSystem,
            keeper: None,
            reason: None,
            keep: system_kept(),
            remove: Vec::new(),
            my_languages: applied.status,
        };
    };

    // The kept copies, by index, with why: the keeper, the system copies,
    // then the language guard, then the last copies of a font.
    let mut kept: Vec<(usize, KeptBecause)> = vec![(winner, KeptBecause::Policy)];
    kept.extend(
        (0..copies.len())
            .filter(|&i| copies[i].system)
            .map(|i| (i, KeptBecause::System)),
    );
    let cover: Vec<BTreeSet<String>> = copies
        .iter()
        .map(|c| ctx.full(c).into_iter().collect())
        .collect();
    let all: BTreeSet<String> = cover.iter().flatten().cloned().collect();
    let mut covered: BTreeSet<String> = kept.iter().flat_map(|&(i, _)| cover[i].clone()).collect();
    let mut guarded = false;
    loop {
        let missing: BTreeSet<&String> = all.difference(&covered).collect();
        if missing.is_empty() {
            break;
        }
        // The copy that covers the most of what is missing; on a tie the
        // one the rules rank first.
        let best = ranked
            .iter()
            .copied()
            .filter(|i| !kept.iter().any(|&(k, _)| k == *i))
            .map(|i| (i, cover[i].iter().filter(|l| missing.contains(l)).count()))
            .filter(|&(_, n)| n > 0)
            .fold(None::<(usize, usize)>, |best, (i, n)| match best {
                Some((_, m)) if m >= n => best,
                _ => Some((i, n)),
            });
        let Some((i, _)) = best else { break };
        covered.extend(cover[i].iter().cloned());
        kept.push((i, KeptBecause::Languages));
        guarded = true;
    }
    // The last copy of a font: while the copies to remove hold a font no
    // file outside them holds, keep the first of them the rules rank.
    let mut lost_fonts: HashMap<usize, Vec<String>> = HashMap::new();
    loop {
        let removed: Vec<usize> = ranked
            .iter()
            .copied()
            .filter(|i| !kept.iter().any(|&(k, _)| k == *i))
            .collect();
        let gone: HashSet<&str> = removed.iter().map(|&i| copies[i].path.as_str()).collect();
        let found = removed.iter().find_map(|&i| {
            let lost: Vec<String> = copies[i]
                .holds
                .iter()
                .filter(|h| h.holders.iter().all(|p| gone.contains(p.as_str())))
                .map(|h| h.name.clone())
                .collect();
            (!lost.is_empty()).then_some((i, lost))
        });
        let Some((i, lost)) = found else { break };
        lost_fonts.insert(i, lost);
        kept.push((i, KeptBecause::LastCopy));
    }
    let one_covers_all = !all.is_empty() && cover.iter().any(|c| c.is_superset(&all));
    let outcome = if guarded && !one_covers_all {
        KeeperOutcome::KeepBoth
    } else {
        KeeperOutcome::Keeper
    };

    let reason = ranked
        .get(1)
        .map(|&second| ctx.reason(&copies[winner], &copies[second]));
    let keep: Vec<KeptCopy> = kept
        .iter()
        .map(|&(i, because)| {
            let c = &copies[i];
            let others: HashSet<&String> = kept
                .iter()
                .filter(|&&(k, _)| k != i)
                .flat_map(|&(k, _)| cover[k].iter())
                .collect();
            let only: Vec<String> = ctx
                .languages
                .iter()
                .filter(|l| cover[i].contains(*l) && !others.contains(l))
                .cloned()
                .collect();
            let sentence = match because {
                KeptBecause::Policy => reason.as_ref().map_or_else(
                    || "Kept: the only copy outside the system font folders.".to_owned(),
                    |r| r.sentence.clone(),
                ),
                KeptBecause::System => {
                    "Kept: in a system font folder; the policy never removes it.".to_owned()
                }
                KeptBecause::LastCopy => format!(
                    "Kept: its file is the last copy of {}.",
                    words(lost_fonts.get(&i).cloned().unwrap_or_default())
                ),
                KeptBecause::Languages if only.is_empty() => {
                    "Kept: it covers languages of yours the copy the rules chose does not."
                        .to_owned()
                }
                KeptBecause::Languages => format!(
                    "Kept: covers {}, the other copies kept do not.",
                    ctx.names.list(&only)
                ),
            };
            KeptCopy {
                path: c.path.clone(),
                because,
                languages_only_here: only,
                fonts_only_here: lost_fonts.get(&i).cloned().unwrap_or_default(),
                sentence,
            }
        })
        .collect();
    let remove = ranked
        .iter()
        .copied()
        .filter(|i| !kept.iter().any(|&(k, _)| k == *i))
        .map(|i| RemovedCopy {
            path: copies[i].path.clone(),
            reason: ctx.reason(&copies[winner], &copies[i]),
        })
        .collect();
    KeeperRecommendation {
        outcome,
        keeper: Some(copies[winner].path.clone()),
        reason,
        keep,
        remove,
        my_languages: applied.status,
    }
}

/// For each file path, the fonts (PostScript names) it holds, each with
/// every file of `files` that holds it.
pub fn holdings(files: &[SetFile]) -> HashMap<&str, Vec<HeldFont>> {
    let names_of = |file: &SetFile| -> BTreeSet<String> {
        file.faces
            .iter()
            .map(|f| f.name.clone())
            .filter(|n| !n.is_empty())
            .collect()
    };
    let mut holders: HashMap<String, Vec<String>> = HashMap::new();
    for file in files {
        for name in names_of(file) {
            holders.entry(name).or_default().push(file.path.clone());
        }
    }
    files
        .iter()
        .map(|file| {
            let held = names_of(file)
                .into_iter()
                .map(|name| HeldFont {
                    holders: holders.get(&name).cloned().unwrap_or_default(),
                    name,
                })
                .collect();
            (file.path.as_str(), held)
        })
        .collect()
}

/// The candidates of a duplicate set, from its copies and their facts.
fn set_candidates(set: &DuplicateSet, held: &HashMap<&str, Vec<HeldFont>>) -> Vec<Candidate> {
    set.copies
        .iter()
        .map(|c| Candidate {
            path: c.path.clone(),
            version: c.version.clone(),
            format: format_from_extension(&c.path),
            in_library_folder: c.in_library_folder,
            system: c.system,
            facts: c.facts.clone(),
            holds: held.get(c.path.as_str()).cloned().unwrap_or_default(),
        })
        .collect()
}

/// Sets each set's [`DuplicateSet::recommendation`] by `policy` and the
/// guard; the copies' `facts` must be filled when [`needs_facts`].
pub fn apply_to_sets(
    sets: &mut [DuplicateSet],
    files: &[SetFile],
    policy: &KeeperPolicy,
    my_languages: &[String],
    names: &LanguageNames,
) {
    let held = holdings(files);
    for set in sets {
        let candidates = set_candidates(set, &held);
        set.recommendation = Some(recommend(&candidates, policy, my_languages, names));
    }
}

/// Orders each group by `policy` and the guard: the keeper first, then the
/// other kept copies, each marked `keep`, then the copies recommended for
/// removal. A group's format and system mark come from `files` by path, the
/// facts from `facts` by face.
pub fn apply_to_groups(
    groups: &mut [DuplicateGroup],
    files: &[SetFile],
    facts: &HashMap<FaceId, CopyFacts>,
    policy: &KeeperPolicy,
    my_languages: &[String],
    names: &LanguageNames,
) {
    let by_path: HashMap<&str, &SetFile> = files.iter().map(|f| (f.path.as_str(), f)).collect();
    let held = holdings(files);
    for group in groups {
        let candidates: Vec<Candidate> = group
            .copies
            .iter()
            .map(|c| {
                let file = by_path.get(c.path.as_str());
                Candidate {
                    path: c.path.clone(),
                    version: c.version.clone(),
                    format: format_from_extension(&c.path),
                    in_library_folder: c.in_library_folder,
                    system: file.is_some_and(|f| f.system),
                    facts: c.face.and_then(|f| facts.get(&f).cloned()),
                    holds: held.get(c.path.as_str()).cloned().unwrap_or_default(),
                }
            })
            .collect();
        let rec = recommend(&candidates, policy, my_languages, names);
        let order: Vec<&str> = rec
            .keep
            .iter()
            .map(|k| k.path.as_str())
            .chain(rec.remove.iter().map(|r| r.path.as_str()))
            .collect();
        let at = |path: &str| order.iter().position(|p| *p == path).unwrap_or(order.len());
        group.copies.sort_by_key(|c| at(&c.path));
        for copy in &mut group.copies {
            copy.keep = rec.keep.iter().any(|k| k.path == copy.path);
        }
    }
}

/// The paths a recommendation keeps, with its keeper, to tell whether two
/// recommendations differ.
fn decision(rec: &KeeperRecommendation) -> (Option<&str>, BTreeSet<&str>) {
    (
        rec.keeper.as_deref(),
        rec.keep.iter().map(|k| k.path.as_str()).collect(),
    )
}

/// What `tried` would do over `sets` against `saved`; the copies' `facts`
/// must be filled when [`needs_facts`] for either policy.
pub fn preview(
    sets: &[DuplicateSet],
    files: &[SetFile],
    tried: &KeeperPolicy,
    saved: &KeeperPolicy,
    my_languages: &[String],
    names: &LanguageNames,
) -> KeeperPolicyPreview {
    let held = holdings(files);
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let (mut with_keeper, mut keep_both, mut changed) = (0, 0, 0);
    for set in sets {
        let candidates = set_candidates(set, &held);
        let new = recommend(&candidates, tried, my_languages, names);
        let old = recommend(&candidates, saved, my_languages, names);
        match new.outcome {
            KeeperOutcome::Keeper => with_keeper += 1,
            KeeperOutcome::KeepBoth => keep_both += 1,
            KeeperOutcome::AllSystem => {}
        }
        if decision(&new) != decision(&old) {
            changed += 1;
        }
    }
    KeeperPolicyPreview {
        sets: count(sets.len()),
        with_keeper,
        keep_both,
        changed,
        my_languages: MyLanguagesStatus::of(my_languages),
    }
}

#[cfg(test)]
mod tests;
