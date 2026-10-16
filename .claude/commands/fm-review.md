---
description: >-
  Code review of a Font Manager story branch (Rust workspace + Tauri 2 / Vue 3).
  Full mode: the parent captures the diff once, picks the lenses the diff needs
  (SPEC, COR, TST always; REUSE, FFI, A11Y when the diff calls for them), runs
  them as isolated parallel subagents, merges duplicates, then runs ONE
  adversarial verifier over the blocking findings so only reproduced defects
  block. Verify mode (review cycles 2+): no new lens fan-out — one subagent
  checks whether the prior blocking findings are fixed and whether the fix
  delta introduced a new blocking defect. Writes one file under .harness/reviews/.
argument-hint: "[anchor: the story id + what it must do] --issue <n> [--mode full|verify] [--cycle <k>] [--prior <review file>] [--since <sha>] [--lenses SPEC,COR,TST,REUSE,FFI,A11Y] [--base main]"
---

# fm-review — Font Manager code review

## Why this command looks the way it does

The review this replaces (tr-harness `/multi-lens-code-review`) produced 562
findings over 23 cycles on its last project: 74% Low, 23% Medium, 2.5% High.
Each lens wrote 4–10 findings whatever the diff size, the fixer deferred two
thirds of what it saw, and review was the most expensive phase of every task.
Every later cycle re-reviewed the whole diff with fresh eyes and found new Lows.

This command fixes that with five rules. Every agent it starts gets them:

1. **Two severities only: `must` and `should`.** No Low, no Style, no nit.
2. **The tool gate already ran.** Review starts only after the CI matrix is
   green on Windows, macOS and Linux. Anything clippy (`-D warnings`), rustfmt,
   cargo-deny, ESLint, Prettier, vue-tsc, nextest, Vitest or Playwright checks is
   not a review finding.
3. **A finding needs a failure.** `Where:` (a diff line), `Failure:` (concrete
   input or state → wrong result), `Evidence:` (the code you read). No failure
   scenario → no finding.
4. **Blocking findings are verified.** One adversarial verifier tries to prove
   each `must` false. Only `CONFIRMED` findings block the merge.
5. **Later cycles only check the fix.** Cycle 2+ runs in verify mode: did the
   fix solve the confirmed findings, and did the fix delta break something?
   No fresh full review, so no fresh pile of findings.

Zero findings is the expected result for a good diff. A lens that returns
`_None._` did its job.

## Operational rules

- The parent **never reads the diff**. It writes the diff to a file and passes
  the path. Subagents read it in full (chunk with `offset`/`limit` past the Read
  cap; never skip a file).
- One `Write` for the review file, at the end. No task-list plumbing.
- Batch independent tool calls: the Phase 1 commands in one message, the lens
  subagents in one message.
- Use the Bash tool (git-bash) for git and gh. Do not translate to PowerShell.
- Subagents: `subagent_type: general-purpose`. Model: read
  `go-harness.config.json` → `review.model`; pass it as the Agent `model` unless
  it is `inherit` (then omit it).

## Phase 1 — Initialize

Parse `$ARGUMENTS`:
- `--issue <n>` (required for the file name; else take it from the PR body's
  `Closes #<n>`), `--mode full|verify` (default `full`), `--cycle <k>` (default:
  next free number), `--prior <file>` and `--since <sha>` (verify mode only),
  `--lenses <csv>` (override the automatic lens choice), `--base <branch>`
  (default `main`). The remaining text is the anchor.

Run in one message:

```bash
ROOT=$(git rev-parse --show-toplevel)
BRANCH=$(git rev-parse --abbrev-ref HEAD)              # E3.2-short-slug
git fetch -q origin "$BASE"
BASE_SHA=$(git merge-base "origin/$BASE" HEAD)
mkdir -p "$ROOT/.harness/reviews" "$ROOT/.harness/review"
gh issue view "$ISSUE" --json title,body,labels -q '.title + "\n\n" + .body' > "$ROOT/.harness/review/issue-$ISSUE.md"
```

If `merge-base` fails, stop and name the base branch. No silent fallback.

**Capture the diff** (generated and data files are not reviewable code):

```bash
DIFF="$ROOT/.harness/review/diff-$ISSUE.patch"
EXCL=(':!Cargo.lock' ':!pnpm-lock.yaml' ':!apps/desktop/src/api/types.ts' ':!fixtures/**'
      ':!**/*.png' ':!**/*.ttf' ':!**/*.otf' ':!dev-docs/harness/**' ':!.harness/**')
FROM="$BASE_SHA"; [ "$MODE" = verify ] && FROM="$SINCE"       # verify mode reviews only the fix delta
{ git diff "$FROM"..HEAD -- . "${EXCL[@]}"; git diff HEAD -- . "${EXCL[@]}"; } > "$DIFF"
git diff --stat "$BASE_SHA"..HEAD -- . "${EXCL[@]}" | tail -1   # "N files changed, A insertions, D deletions"
git diff --name-only "$BASE_SHA"..HEAD -- . "${EXCL[@]}"
```

If the diff is empty, print `Nothing to review.` and stop.

**Context pack.** Write `$ROOT/.harness/review/context-$ISSUE.md` with the
Write tool. It holds paths and short facts, not file bodies:

- Story: id, title, size, track, epic (from the `fm-meta` block) and the path
  `.harness/review/issue-$ISSUE.md` (the full body: acceptance criteria + DoD).
- Rules: `CLAUDE.md` (the "Rules that do not bend" and "Conventions" sections)
  and `dev-docs/harness/review-rules.md` (graduated lessons).
- ADRs: `ls docs/adr/*.md` with each file's first heading. Lenses read only the
  ADR for the area they judge.
- Spec: if `docs/spec/` exists, the file(s) whose text names this story id or
  its epic id (`grep -l`). Otherwise write "spec: not exported locally — judge
  against the acceptance criteria and CLAUDE.md".
- CI: a 3-line summary of `.harness/ci/issue-$ISSUE.json` (status per OS, bench
  status) if the file exists.
- Implementer notes: `.harness/notes/issue-$ISSUE.md` if it exists. It records
  deliberate decisions and the constraint behind each, and any disputes of
  earlier findings.
- Screenshots: `.harness/screens/issue-$ISSUE/*.png` if they exist (A11Y lens).
- PureContext: call `resolve_repo({ path: "<ROOT>" })`; write the `repoId` and
  freshness. If the index is behind, call `index_folder({ path, onlyChanged: true })`
  first. If PureContext is not available, write "PureContext: unavailable — use grep".

**Choose the lenses** (full mode; `--lenses` overrides):

| Lens | Runs when |
|------|-----------|
| SPEC | always |
| COR | always |
| TST | always |
| REUSE | not (size S and < 150 changed lines) — the diff adds or changes `pub` items, new files under `crates/*/src` or `apps/desktop/src` |
| FFI | the diff touches `crates/fm-platform/**`, or `grep -nE 'unsafe|windows::|core_text|core_foundation|objc2|fontconfig|winreg|Registry|std::fs::(write|remove_|rename|copy)|fs::(write|remove_|rename|copy)|File::create' "$DIFF"` hits |
| A11Y | track `ui`, or the diff touches `apps/desktop/src/**/*.vue` |

**Output path:** `.harness/reviews/issue-$ISSUE-review-$CYCLE.md` (cycle = the
next free number when `--cycle` is not given).

## Phase 2 (full mode) — Lens subagents, parallel and isolated

Dispatch one subagent per chosen lens in a single message. No subagent sees
another's findings, so lenses that agree are real triangulation.

Each prompt is self-contained. Include verbatim:

1. "Read the review diff in full from `<absolute $DIFF>`. Read the context pack
   `<absolute context file>` first, then the story body it points to."
2. The anchor (or "no anchor — use the acceptance criteria").
3. **The rules for every lens** (next section), copied in full.
4. The lens charter (below), copied in full.
5. The output structure, with the lens prefix.
6. "Return ONLY your lens section as markdown. Do not write any file. Do not
   edit code."

### Rules for every lens

**What blocks (`must`).** Only these:
- an acceptance criterion is not delivered, or is delivered wrong;
- a "Rule that does not bend" in `CLAUDE.md` is broken by this diff;
- user files or user font registrations can be changed, lost or left behind;
- a panic, `unwrap`/`expect` outside tests, or undefined behavior is reachable
  (malformed font data must give `Health::Broken`, never a panic);
- an error aborts a whole batch where it must be per face or per file;
- a criterion has no test that fails when the behavior regresses, or a test
  asserts nothing that can fail;
- platform parity breaks (works on one OS, silently does nothing on another);
- an existing `fm-api` type changes incompatibly;
- an unsafe/FFI defect (FFI lens).

**What is advisory (`should`).** A real defect with a smaller consequence: a
user or maintainer will hit it, but the story still does what it promised. At
most **2** per lens. `should` findings never block and are never auto-fixed.

**What is NOT a finding.** Say nothing about: formatting, lint, naming, compile
or type errors (CI is green); comments and doc wording; "consider …";
refactors; extracting or inlining helpers; a helper with one caller; one more
assertion on a behavior a test already proves; defensive checks for states the
types rule out; performance with no budget at stake; work the spec gives to a
later story; anything the implementer notes record as a deliberate decision
with a named, checkable constraint (unless you show the constraint is wrong).

**Every finding has three lines.**
- `Where:` `path:line` on a line this diff adds or changes.
- `Failure:` the concrete input or state and the wrong result it produces.
- `Evidence:` what you read that proves it (file:line, a caller, a test).
No `Failure:` → it is not a finding. The verifier rejects a `must` it cannot
reproduce, and rejected findings are counted per lens.

**In-diff only.** Rank only defects this diff adds or makes worse. A
pre-existing defect goes under `Out-of-diff observations`: one line, max 2,
never counted, never blocking. The harness parks them for a human.

**Stay in your charter.** Another lens covers the rest. Two lenses reporting
the same root cause costs a merge step and adds nothing.

**Investigation.** Read what a finding depends on, nothing more. Use
PureContext first (`repoId` is in the context pack):
- read one symbol: `get_symbol_source` (several: `get_symbols`);
- who calls it: `find_references`; impact of a changed item: `get_blast_radius`;
- near-duplicates: `search_similar`, `search_symbols`, `check_consistency`.
For "nothing references X" use `grep` — the index cannot prove absence. For
`.vue` files and config files, read the file. Narrow reads (`grep -n` then
`Read` with `offset`/`limit`) beat whole-file reads.

### Output structure (every lens)

```markdown
## [<PREFIX>] <Lens name>

### Must
#### <PREFIX>-1. <one-sentence headline>
Where: <path:line>
Failure: <input/state → wrong result>
Evidence: <what proves it>
Fix: <one or two sentences>

### Should
<at most 2, same four lines>

### Out-of-diff observations
<max 2, one line each with path:line, or _None._>

### Net
<one or two sentences>
```

Write `_None._` under an empty heading.

### Lens charters

**[SPEC] Spec and criteria.** For each acceptance criterion, find the code that
delivers it and say whether it does — no more and no less. Then check what the
spec and `CLAUDE.md` forbid: logic in a Vue component, a Tauri command, the CLI,
the MCP server or the plugin endpoint (everything goes through `fm-api` into
`fm-core`); moving, renaming or deleting a user file without an explicit user
action with undo, or deletion that is not the OS trash; deactivating a face the
app did not register; admin elevation or writes to system font folders; a
network call without a user action; telemetry with the flag off; a
`#[cfg(target_os)]` outside `fm-platform`; an existing `fm-api` type changed, a
field removed or renamed, a new field that is not `Option<T>` or
`#[serde(default)]` (a contract change outside the epics listed in
`go-harness.config.json` `contract_epics` is a `must`: it needs a human); a new
dependency that the spec's library table does not list. Scope creep that
changes behavior another story owns is a `should`.

**[COR] Correctness and robustness.** Rust: reachable `unwrap`/`expect`/`panic!`/
index-out-of-bounds outside tests; integer overflow or truncating `as` casts on
values read from font tables; errors that abort a scan or an activation batch
instead of failing per face; `FmError` without a user message; SQLite writes
outside a transaction where a partial write corrupts the index; FTS5 query text
not escaped; a lock held across `.await`; blocking I/O on an async runtime
thread; a missing `Event` emit after a library, scan or activation change (the
UI subscribes, it does not poll); paths built with string concatenation, lost
non-UTF-8 paths, case-sensitivity assumptions; file handles or OS resources not
released. TypeScript/Vue: store state that can go out of sync with an event;
`fetch` or Tauri `invoke` outside `src/api/`; the font list rendering all rows.

**[TST] Test adequacy.** Does each acceptance criterion have a test, at the
layer where the logic lives, that fails if the behavior regresses? Name the
criterion and the test (or its absence). Then the unhappy paths that matter
here: malformed or truncated font file, empty folder, duplicate face, non-UTF-8
or very long path, permission denied, a file removed mid-scan. Tests must use
`fixtures/fonts/`, temp dirs and no network; performance tests skip when
`FM_REFERENCE_LIBRARY` is unset; `fm-platform` tests prove a second process
sees the activated font; a UI story adds a Playwright check. `must` = a
criterion with no failing-on-regression test, or a test that asserts nothing
that can fail. `should` = an important unhappy path untested while the happy
path is solid. More assertions, test naming and test structure are not findings.

**[REUSE] Reuse and contract consumers.** The two things a diff-only lens cannot
see. (A) A new function, type, store or component that duplicates one that
already exists: check each new `pub` item with `check_consistency` /
`search_similar` / `search_symbols`, and name the existing one and the reuse
path. A duplicate that already behaves differently is a `must`; one that will
drift is a `should`. (B) A changed `pub` item, trait, `fm-api` type, event
variant or shared component whose existing callers break: list them with
`find_references` / `get_blast_radius` and cite each broken caller
(`path:line`). A broken caller is a `must`. A purely additive change is a clean
pass.

**[FFI] Unsafe, FFI and platform.** Every finding of this lens blocks: use
`must` only. Memory safety of every `unsafe` block (lifetimes of pointers
passed to Win32, Core Text, Core Foundation, objc2 or fontconfig; ownership and
release of CF/COM objects; null and error returns checked); errors handled per
face — one bad face never fails the whole batch; cleanup on quit and on crash
(session-activated fonts are deregistered; a crash leaves state the next start
can repair); no admin elevation; nothing written to system font folders or to
registry keys outside the per-user font key; file-system writes only inside the
app's own folders; the activation state recorded in the database before the OS
call is trusted; the same behavior on all three OS, with any difference
documented in the ADR.

**[A11Y] Accessibility.** Read the screenshots in the context pack (1,280×800
and 1,000×700) and the changed `.vue` files. `must`: an interactive element with
no accessible name; a control unreachable by keyboard, or a focus trap; focus
order that skips or loops; new text or chrome under WCAG AA contrast (4.5:1
text, 3:1 large text and UI parts); the inspector unusable below 1,100 px.
`should`: missing reduce-motion handling for a new animation; list rows without
screen-reader text.

## Phase 3 (full mode) — Merge duplicates, then verify the musts

**Merge.** Two findings are one when their `Where:` lines point at the same
file within ±3 lines, or their headlines name the same root cause. A merged
finding keeps the higher severity and lists every lens ID. Drop any finding
without `Where:` or `Failure:`. Do not re-rank or soften for any other reason.

**Verify.** If there is at least one `must`, dispatch ONE verifier subagent
(same model rule). Its prompt includes: the diff path, the context pack path,
the investigation rules above, and the merged `must` list (ID, Where, Failure,
Evidence, Fix). Its instructions:

> You are the adversarial verifier. Your job is to prove each finding wrong.
> For each one, read the cited code and what it depends on — callers via
> `find_references`, the type via `get_symbol_source`, the tests. Decide:
> - `CONFIRMED` — you can state the failure path in the current code, step by step.
> - `REJECTED` — the code already handles it, the path cannot happen, the defect
>   is not on a line this diff adds or changes, or a CI tool covers it.
> - `DOWNGRADED` — it is real but does not meet the `must` list; it becomes a `should`.
> Return one line per finding: `<ID> | <STATUS> | <one-sentence reason with path:line>`.
> Do not invent new findings. Do not edit code.

`should` findings are not verified. Each becomes `ADVISORY`.

## Phase 2+3 (verify mode) — Check the fix, not the world

Used for review cycles 2 and later. Requires `--prior <review file>` and
`--since <sha>` (the commit the prior review saw). `$DIFF` is the fix delta.

Dispatch ONE subagent. Its prompt includes: the fix-delta diff path, the
context pack path (with the implementer notes — read their "Disputes" section),
the rules for every lens, the COR charter, the FFI charter when the delta
touches platform code, and every row of the prior review's index whose status
is `CONFIRMED`, `NEW` or `OPEN`. Its instructions:

> 1. For each prior blocking finding, decide `RESOLVED` (the fix removes the
>    failure path — say how) or `OPEN` (it does not). If the implementer
>    disputes it in the notes, judge the dispute on the evidence: `RESOLVED`
>    when the named constraint holds, `OPEN` when it does not.
> 2. Report a `NEW` finding only when the fix delta itself adds a `must`-level
>    defect from the COR or FFI charter. No `should` findings in verify mode.
> Return the prior findings first, one line each: `<ID> | RESOLVED|OPEN | reason`,
> then any NEW findings in the four-line format.

## Phase 4 — Write the file once

```markdown
# fm-review — <story id> #<issue> — cycle <k> (<full|verify>)

**Branch:** <branch>  **Base:** <base> @ <BASE_SHA short>  **Since:** <sha or —>
**Lenses:** <csv, or "verify">  **Diff:** <N files, +A −D>
**Anchor:** <anchor>
**Date:** <YYYY-MM-DD>

## Findings index

| # | Sev | Status | Where | Headline | Lens IDs |
|---|-----|--------|-------|----------|----------|
| 1 | must | CONFIRMED | crates/fm-core/src/scan.rs:88 | <headline> | COR-1, TST-2 |
| 2 | should | ADVISORY | apps/desktop/src/stores/library.ts:41 | <headline> | SPEC-1 |
| 3 | must | REJECTED | crates/fm-core/src/index.rs:12 | <headline> | COR-2 |

Counts: must=1 should=1 rejected=1 resolved=0 (raw=4 unique=3)

## Verifier
<one line per verified finding, as the verifier returned it>

## Out-of-diff observations
<all lenses' out-of-diff lines, or _None._>

---

<lens sections in the order SPEC, COR, TST, REUSE, FFI, A11Y — only the ones that ran>
```

Rules: `Sev` is `must` or `should`. `Status` is one of `CONFIRMED`, `REJECTED`,
`ADVISORY` (full mode) or `RESOLVED`, `OPEN`, `NEW` (verify mode). A
`DOWNGRADED` finding is written as `should | ADVISORY`. In verify mode the
index lists every prior blocking finding with its new status, plus NEW rows.
`must` in the Counts line = CONFIRMED + OPEN + NEW. With no findings, write
`_No findings._` and `Counts: must=0 should=0 rejected=0 resolved=0 (raw=0 unique=0)`.
`scripts/harness/review-counts.mjs` parses this table — keep the columns.

If a subagent failed to return a usable section, say so under that lens's
heading. Do not drop it silently.

## Phase 5 — Finish

```bash
rm -f "$DIFF"
node scripts/harness/review-counts.mjs "<output file>"
```

Print the output path and the counts JSON. Nothing else. The file is the record.
