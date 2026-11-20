# 0027 — Duplicates

Status: Accepted (2026-11-20, E56.1; amended 2026-11-20, E56.2, E56.3, E56.4)

## Context

Duplicate detection (E8.1) knows two cases: the same file at two paths
(`exact`), and the same PostScript name with the same version in files that
differ (`near`). A collection that grew over twenty years mostly holds a third
case: two or more versions of one font. E8.1 does not find them, because a
near group needs the same version. E56 builds a Duplicates module for cleaning
up such a collection, and every story of it starts from one listing of
duplicate sets. This record fixes what a duplicate set is.

The decisions below were made by Goran on 20 Nov 2026.

## Decision

**Three levels of sameness.** A duplicate set has one level:

- `identical` — files with the same hash at different paths;
- `sameVersion` — one PostScript name and one version in files that differ;
  the set also says whether its copies differ in format (`formatsDiffer`);
- `otherVersion` — one PostScript name in files whose versions differ.

**Two versions of one font are a duplicate set.** They hold the same font for
the user: one PostScript name means one font to the OS, so only one can be
active at a time, and documents name the font, not the version. Leaving the
older version in the library is a choice the user should make once, with both
copies in view, not a state the library hides. So a name set holds every file
with that PostScript name, as long as at least two files differ, and its level
is `sameVersion` when every copy has the same version and `otherVersion`
otherwise.

**A file is listed once per font it duplicates.** An identical set whose fonts
all sit in name sets that hold all its files is not listed again; those name
sets show the copies as `identicalWith` each other (a set of three versions of
which two are the same file shows those two as identical inside it). A
collection file (`.ttc`) is one copy in each set it takes part in, for the
face that set is about, and names the other sets (`otherSets`), so that a plan
never trashes it for one set while another set keeps it.

**Versions are compared as the keeper rule compares them**: trimmed, digit runs
as numbers, other text without case (`Version 1.000` is `version 1.0`). A copy
without a version is its own value: two copies without one are the same
version, and a copy without one is never called older or newer than another.
The set ranks no copy; which copy to keep is the keeper policy (E56.4).

**What counts.** Only the files the library shows: not gone, source not
paused, read (hashed). Files in the system font folders are listed, because
they are copies the user has, and are marked `system`; they can never be
chosen for removal (rule 4): "Keep this one" skips such a copy.

**A listing with a scope and pages.** `list_duplicate_sets` takes the whole
library, one source, or one folder with its subfolders. A set is listed when
one of its copies is in the scope, with all its copies, each marked `inScope`:
the user cleans up a folder, but decides with every copy in view. It pages, so
the UI never holds every set. It is computed on each call and never at start,
so cold start does not change. The E8.1 `list_duplicates` groups and their
types (`DuplicateGroup`, `DuplicateCopy`, `KeepCopyRequest`) do not change.

**Budget.** `scripts/bench.sh` measures one page of `list_duplicate_sets` over
the reference library with its first folder copied beside it
(`duplicate_sets`). On 20 Nov 2026 on Windows (the reference library's
`fontshare` folder copied: 17,069 files, 1,208 sets) the p95 was 229–244 ms,
nearly all of it reading the files and faces from the index. The budget is
**500 ms** on every OS: twice the measured number, for slower CI machines, and
under the half second in which a user opening the module still reads it as
immediate.

**What differs between two copies (E56.2).** Each copy of a listed set
carries the facts a decision needs (`facts`): the file's time, the glyph
count, the axes and features, the activation and the documents that asked
for it, the user's favorite, rating, notes, tags, collections and sets, and
its language coverage from the index (presets covered in full, presets with
`locl`). They are read for the copies of the returned page only, so the
listing budget stays the one above. `compare_copies` takes two faces and
reads both files on demand: languages gained and lost, glyphs added and
removed (a glyph is known by the character it maps, else by its glyph name,
because glyph ids change between versions), features and axes added and
removed, and the vertical metrics, units per em and advance widths compared
per em; a change in the metrics or the widths says text may reflow. When
every table holds the same data (the outlines compared glyph by glyph when a
WOFF2 stores `glyf` differently; `head` without its checksum and transform
flag; `DSIG` and `loca` left out) only the container differs, and the answer
says so. A file that cannot be read is `Health::Broken` for its side and the
comparison names that side; it is never an error.

**Comparison budget.** `scripts/bench.sh` times `compare_copies` on the two
copies of a set in the bench index that hold the most bytes, other versions
first (`copy_comparison`). On 20 Nov 2026 on Windows that was Google Sans
Italic, the variable font against its static instance (7.1 MB together): the
p95 was 39 ms, the first, cold call 648 ms. The budget is **150 ms** on every
OS: almost four times the measured number, because the time grows with the
files and CI disks are slower, and under the fifth of a second in which
opening two copies side by side still reads as immediate. The first call is
not budgeted, as with every other measurement.

**The languages you work in (E56.3).** The settings hold `myLanguages`:
ids of `list_languages` presets, in the user's order, none by default. They
are stored in `config.json` and set through `update_settings`, which refuses
an id without a preset and keeps a repeated id once. An id the file names but
this version has no preset for (a newer version's file, a removed preset, a
hand edit) is dropped on read: that read answers it in `droppedLanguages` and
saves the list without it, so it is reported once; `droppedLanguages` is never
stored. Every rule and guard of this epic that names "my languages" goes
through `fm_core::my_languages::apply`: with the list empty it does nothing,
and its answer carries `MyLanguagesStatus::NotSet`, so the UI can offer to
set them.

**The keeper policy (E56.4).** Which copy of a set to keep is answered by
the keeper policy, stored in `config.json` as `Settings::keeper_policy` and
set through `update_settings`: an ordered list of rules. A copy that wins a
rule wins the set; a tie goes to the next rule; the last rule is always the
path A to Z (`KeeperPolicy::normalized` moves it there), so the answer never
depends on chance. The rules: covers more of my languages; has `locl` forms
for more of my languages; newest version (as the versions above are
compared; no version ranks after any version); format, in an order the user
sets (default OpenType CFF, TrueType, WOFF2, WOFF, Type 1; a format not in
the order ranks after every one in it); variable over static; most glyphs;
in a folder the user prefers (an ordered list of absolute folders); in the
library folder; carries the user's metadata (favorite, rating, tags, notes,
collections, sets: more kinds wins); active or asked for by a document;
path. A rule or format a newer version wrote is dropped on read, so the file
still reads. **The default policy** is: covers my languages, newest
version, format, the library folder, path. With no language set it is the
E8.1 keeper rule with the format added: `DuplicateGroup::choose_keeper`
ranks the format by the file's extension, because `DuplicateCopy` has no
format and its type does not change.

**The language guard is not a rule.** A rule can be moved down or taken out;
the guard cannot, because losing the only copy of a language the user works
in is the one mistake a cleanup must not make, whatever the user ranked
first. After the rules chose the keeper, the guard keeps, one at a time, the
copy that covers the most of my languages (in full) that no kept copy covers
yet, the rules' order breaking a tie, until every language some copy of the
set covers is kept. So a copy that is the only one covering one of my
languages is never recommended for removal. A set in which no copy covers
everything the others cover is answered "keep both" (`KeepBoth`), each kept
copy with the languages it alone has. With no language set the guard does
nothing (`my_languages::apply`) and says so.

**What the policy never does.** It never applies to a copy in a system font
folder: such a copy is not ranked and is always kept (rule 4); a set of
system copies only is `AllSystem`. It never recommends removing the last
copy of a font: the keeper is always kept, and a file that alone holds
another font (a collection file) is kept too.

**Every recommendation carries its reason as data**: the rule that decided,
the values it compared, the other copy, and one sentence ("Kept: covers
Serbian (Cyrillic), the other copy does not."), so the UI, the CLI and MCP
say the same thing. A set's `recommendation` holds the keeper, every kept
copy with why, and every copy to remove with the rule it lost by.
`preview_keeper_policy` tries a policy over a scope without saving it: how
many sets get a keeper, how many are "keep both", and how many
recommendations change against the saved policy.

**The existing requests follow the policy from this version on** (Goran,
20 Nov 2026). `list_duplicates`, `list_problems` and `fm doctor` take their
keeper from the saved policy and the guard, so the Problems view, the CLI
and the module never recommend different copies of one set. No type
changes: the keeper is still first and marked `keep`; a copy the guard
protects, a system copy and a last copy are marked `keep` too and are no
Problems entry. The policy reads the copies' facts from the index only
when a language is set or a rule weighs a fact, so the default costs one
read of the files. The mocks have no index: their sets carry no
recommendation and `list_duplicates` uses `choose_keeper`.

**Out of scope.** Fonts that only look alike (another name, another foundry,
the same design): that is similarity (E23, ADR 0023), not duplication. Files
that are not read, and fonts without a PostScript name, are found only as
identical files.

## Consequences

- One rule in `fm-core` (`duplicate_sets`), with copies in `fm_api::mock` and
  the TypeScript mock (fm-api cannot use fm-core); the case file
  `fixtures/duplicates/set-cases.json` runs against all three, and
  `fixtures/duplicates/mock-library.json` proves both mocks list the same sets
  for the same library.
- A set has a stable id (`identical:<hash>`, `name:<PostScript name>`) the
  later stories (plans, "these are not duplicates") can refer to.
- Every page reads the whole index again. If a later story needs pages faster
  than the budget, it caches the sets until the next library change.
