---
name: go-cycle
description: |
  One Font Manager story, one stage at a time, driven by go-harness.ps1:
  build (read → branch → implement → preflight → draft PR), ci-fix (repair a
  red CI matrix), review (/fm-review loop, max 3 cycles), finalize (tick
  criteria with evidence, summary, PR ready). Emits GOCYCLE_PHASE and
  GOCYCLE_METRIC markers and ends every stage with a GOCYCLE_ sentinel.
  Invoke as: /go-cycle {issue-number} --stage build|ci-fix|review|finalize
  Triggers on: go-cycle, work a story, harness stage.
allowed-tools:
  - Bash
  - Read
  - Edit
  - Write
  - Glob
  - Grep
  - Agent
  - Skill
---

# go-cycle — one story, one stage

The harness (`go-harness.ps1`) owns the loop. It claims the story, runs this
skill once per stage in the lane's worktree, and waits for CI **between**
stages (no tokens spent waiting). Usually every stage after `build` resumes
the same conversation (`claude --resume`), so you keep the implementation
intent. If it cannot resume, you start cold: `.harness/notes/issue-{n}.md` is
the memory that survives — read it first, keep it current.

**Read `CLAUDE.md` before anything else in a cold start.** Its "Rules that do
not bend" are the hard constraints of every stage.

## Invocation

| Stage | Flags | Ends with |
|-------|-------|-----------|
| `build` | — | a draft PR with green local preflight |
| `ci-fix` | `--ci <verdict.json> --attempt k/3` | a pushed fix for the failing platform(s) |
| `review` | `--cycles N` (default 3) | zero open `must` findings, KB distilled |
| `finalize` | `--ci <verdict.json>` | criteria ticked with evidence, PR ready |

You never merge. The harness decides that from `go-harness.config.json`.

## Markers (REQUIRED)

Plain assistant text, alone on its line, no backticks or bold:

```
GOCYCLE_PHASE: <read|branch|implement|preflight|pr|ci|review|finalize>[ | detail]
GOCYCLE_METRIC: <kind> | key=value key=value
```

- Print a phase marker **before** the work of that phase. On a resume, print
  the marker of the phase you actually resume into.
- Review cycles: `GOCYCLE_PHASE: review | cycle 2/3`. CI fix: `GOCYCLE_PHASE: ci | fix 1/3`.
- Metric lines (review stage): `review_done` (paste the output of
  `review-counts.mjs --marker`, never count by hand), `fix_start`, `fix_done`,
  `dispositions | review_cycle=N | fixed=F disputed=D`. Finalize:
  `criteria | ticked=T pending=P`.
- Never put a marker on the last line.

## Sentinels (REQUIRED — the very last line of every stage)

- `GOCYCLE_OK` — the stage did what the table above says.
- `GOCYCLE_BLOCKED: <one-line reason>` — a human must decide (see Stop conditions).
  The harness labels the story `harness:blocked` and comments your reason.
- `GOCYCLE_FAIL: <one-line reason>` — something broke that a human must look at
  (tooling, auth, an error you cannot get past after 3 tries).

## Stop conditions — print GOCYCLE_BLOCKED, do not work around them

1. You would change an **existing** `fm-api` request/response type (rename,
   remove, retype, make a field required) and the story's epic is not in
   `contract_epics` of `go-harness.config.json` (E1, E25). Adding new types or
   new `Option<T>` / `#[serde(default)]` fields is normal work, not a stop.
2. You need a dependency (crate or npm package) that the spec's library table
   does not list, or that `cargo-deny` rejects. Ask; do not add it.
3. The acceptance criteria contradict each other, or one cannot be verified.
4. The story would break an ADR under `docs/adr/`.
5. A `must` finding survives 2 fix rounds (review stage, below).

## Every stage starts the same way — PureContext first

PureContext is the indexed symbol graph of this repo. Use it for locating code,
reading symbols, caller lists and impact. Use grep only to prove absence.

1. `resolve_repo({ path: "<absolute worktree root>" })` → `repoId`. (Do not call
   `list_repos`: it lists every index on this machine and is huge.)
2. Not indexed → `index_folder({ path })`. Freshness `behind` →
   `index_folder({ path, onlyChanged: true })`. `re-index in progress` → wait
   briefly or use grep for now.
3. After **every** file you write: `index_file({ repoId, filePaths: [...] })`.
   Never `index_folder` mid-stage.
4. Read `.harness/notes/issue-{n}.md` if it exists.

`.vue` files are not parsed by the index: read them, and use grep for them.

---

## Stage `build`

### read

`GOCYCLE_PHASE: read`

```bash
node scripts/harness/issue.mjs meta {n}        # title, fm-meta, acceptance criteria
gh issue view {n} --comments
```

- The epic: find it with `gh issue list --label type:epic --state all --search "<epic id> in:title" --json number,title`
  and read its intro only.
- Blockers (`fm-meta depends`): they are closed and merged — the code is the
  truth, not their issue text. Read their titles; go to the code for shapes.
- The spec section: `docs/spec/` if it exists (`grep -l "<story id>\|<epic id>"`).
  Otherwise the acceptance criteria and `CLAUDE.md` are the spec.
- The ADR for the area (`docs/adr/`). If the story would contradict it → stop
  condition 4.
- Orient: `get_task_context({ repoId, task: "<id> <title>: <criteria>" })`.
- Pitfalls from past reviews — a hard constraint on your implementation:
  `node scripts/harness/kb.mjs retrieve --paths "<globs you will touch>" --top 8`

Write `.harness/notes/issue-{n}.md`:

```markdown
# {id} #{n} — {title}
repoId: <id>
## Criteria
1. <criterion> — plan: <where it lives> — test: <planned test>
## Predicted files
<from prepare_change / get_task_context>
## Decisions
- <decision> — constraint: <the checkable reason: ADR, CLAUDE.md rule, criterion, library limit>
## Disputes
## CI
## Evidence
## Advisory
```

Reviewers read this file. A decision with a named constraint is not re-litigated
by them. "I preferred it" is not a constraint.

### branch

`GOCYCLE_PHASE: branch`

Branch name: `{id}-{slug}` (e.g. `E3.2-fts-index`; slug 2–4 words, kebab-case).

```bash
git fetch origin
EXISTING=$(git branch -a --list "*{id}-*" | head -1 | sed 's/[* ] //; s|remotes/origin/||' | xargs)
PR=$(gh pr list --state open --json number,headRefName -q '.[] | select(.headRefName | startswith("{id}-")) | .number')
```

- Branch or PR exists → **resume**: check it out, pull, read the notes, find
  the first unfinished phase, continue there. Do not re-implement.
- Else: `git checkout -b {id}-{slug} origin/main`.

### implement

`GOCYCLE_PHASE: implement`

- Order: logic in `fm-core` → expose through `fm-api` → wire the adapter or the
  UI. No logic in Vue, Tauri commands, CLI, MCP or the plugin endpoint.
- **Before editing existing shared code** (anything `pub` in `fm-core`,
  `fm-api`, `fm-platform`, a Pinia store, a shared component):
  `prepare_change({ repoId, intent, query })`. Put
  `predictedChange.changedFilePaths` and `missingCoChange` in the notes; you
  pass them to `verify_change` later. High `risk` → read `get_blast_radius`
  and keep the change additive.
- **Before creating a new `pub` symbol or file:**
  `check_consistency({ repoId, name, kind, intendedFilePath, signature })`.
  `gate: warn` with a duplicate → reuse or extend the existing one.
- Rename, delete, move: `check_rename_safe` / `check_delete_safe` / `check_move_safe` first.
- `index_file` after every write.
- Tests at the layer where the logic lives, in the same commit as the code.
  Every criterion gets a test that **fails if the behavior regresses**. Cover
  the unhappy paths that matter: malformed font, empty folder, duplicate face,
  non-UTF-8 path, permission denied. Use `fixtures/fonts/`; performance tests
  skip without `FM_REFERENCE_LIBRARY`. Platform code: an integration test that
  proves a second process sees the activated font. UI stories: a Playwright check.
- Fill the notes' Criteria → test mapping as you go.

### preflight

`GOCYCLE_PHASE: preflight`

All must pass, in this order:

```bash
pnpm lint
cargo nextest run --workspace
pnpm test
pnpm gen:types && git diff --exit-code apps/desktop/src/api/types.ts
```

On a failure: fix and re-run all four. Three failed rounds on the same error →
`GOCYCLE_FAIL`.

**Numeric criteria** (`node scripts/harness/issue.mjs numeric {n}` exits 0):
if `.github/workflows/bench.yml` exists, the harness runs the bench job on the
PR. If it does not exist yet (before E9.1), run `scripts/bench.sh` (or the
timing test the story adds) now and write the numbers in the notes' Evidence
section, marked "pending E9.1".

**UI stories (`track: ui`):** capture the screens this story changes at
1,280×800 and 1,000×700 into `.harness/screens/issue-{n}/`:
`FM_SCREENS_DIR=.harness/screens/issue-{n} pnpm test:e2e --grep @screens`.
If `apps/desktop/e2e/screens.spec.ts` does not exist yet, add it in this story:
it visits the routes in `FM_SCREENS_ROUTES` (comma list) on the MockApi at both
viewports and saves `<route>-<w>x<h>.png` to `FM_SCREENS_DIR`.

**PureContext completeness check:**
`verify_change({ repoId, diff: "<git diff origin/main...HEAD>", predictedFilePaths, predictedCoChange })`.
`gate: block` (`incomplete`) → handle each `unaddressedCoChange` or write in the
notes why it does not apply. `scope_expanded` → each unplanned file must belong
to the story; revert what does not.

### pr

`GOCYCLE_PHASE: pr`

```bash
git fetch origin && git rebase origin/main      # re-run preflight if the rebase changed anything
git push -u origin HEAD
```

Commit messages: imperative, one line of what, a body of why if not obvious.

If no PR exists:

```bash
gh pr create --draft --base main --title "{id} {title}" --body-file - <<'EOF'
Closes #{n}

## What changed
- <2–4 bullets>

## Verification
| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | <criterion> | <test name> — CI pending |

## Notes
<decisions with their constraint; anything a reviewer must know>

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
```

UI stories: publish the screenshots and add them as a PR comment:
`node scripts/harness/assets.mjs --issue {n} --dir .harness/screens/issue-{n}`.

End: `GOCYCLE_OK`.

---

## Stage `ci-fix`

`GOCYCLE_PHASE: ci | fix k/3`

Read the verdict file. For every failed job you have its OS, URL and a log
excerpt. The local preflight passed on Windows, so **the failure is
platform-specific** — look for: path separators and `\` in test expectations;
CRLF vs LF (`.gitattributes`); case-sensitive file systems; OS font APIs that
behave differently; permissions; timing and ordering in tests; missing system
packages on the runner.

- **Bench red** (`bench.status: red`): the failing metrics are in
  `bench.failed` (`value`, `unit`, `target`). Make the code meet the target;
  never change the target or the bench.
- Fix the cause, not the test. If the failure exposed an untested platform
  behavior, add the test.
- If a fix needs a `#[cfg(target_os)]`, it goes in `fm-platform`, never in `fm-core`.
- Preflight again, commit `fix(ci): <os>: <what>`, push.
- Add a line to the notes' CI section: OS, cause, fix.

End: `GOCYCLE_OK`. (The harness waits for CI again and counts attempts.)

---

## Stage `review`

`/fm-review` runs the lenses in isolated subagents. You fix in this context,
where the intent lives. Let `MAX` = `--cycles` (default 3).

For cycle `k` = 1 … `MAX`:

1. `GOCYCLE_PHASE: review | cycle k/MAX`. Record `SEEN=$(git rev-parse HEAD)`.
2. Run the review:
   - k = 1: `/fm-review {id}: <1-line summary of what the story must do> --issue {n} --mode full --cycle 1`
   - k ≥ 2: `/fm-review {id}: <summary> --issue {n} --mode verify --cycle k --prior .harness/reviews/issue-{n}-review-<k-1>.md --since <SEEN of cycle k-1>`
3. Print the metric line, verbatim:
   `node scripts/harness/review-counts.mjs .harness/reviews/issue-{n}-review-k.md --marker k`
4. `must = 0` → print `GOCYCLE_METRIC: dispositions | review_cycle=k | fixed=0 disputed=0`
   and leave the loop.
5. Else print `GOCYCLE_METRIC: fix_start | review_cycle=k` and handle every
   blocking row (`CONFIRMED`, `OPEN`, `NEW`):
   - **Fix it** — in the code and with a test that would have caught it. Use
     `prepare_change` / `index_file` as in build. A `[TST]` finding is fixed by
     writing the missing test, never by weakening one.
   - **Or dispute it** — only with a checkable constraint (an ADR, a
     `CLAUDE.md` rule, a criterion, a library limit) or a factual error you
     verified in the code. Write it in the notes' Disputes section:
     `<ID>: <constraint> — <evidence path:line>`. The next verify cycle judges it.
6. `should` (ADVISORY) rows: do **not** fix them. Copy them to the notes'
   Advisory section. They go into the PR summary. (Fixing advisory findings is
   how the old loop kept finding new problems.)
7. Preflight (all four commands), commit `fix: review cycle k`, push. Print
   `GOCYCLE_METRIC: fix_done | review_cycle=k` and
   `GOCYCLE_METRIC: dispositions | review_cycle=k | fixed=F disputed=D`.

After cycle `MAX`, if the last review still has `must > 0`: those findings
survived `MAX-1` fix rounds →
`GOCYCLE_BLOCKED: <count> must finding(s) open after <MAX-1> fix rounds: <IDs and one-line headlines>`.

**Before `GOCYCLE_OK`, two bookkeeping commits on the branch:**

1. **Parking lot.** Append every `Out-of-diff observations` line of this
   story's reviews to `dev-docs/harness/parking-lot.md` under today's date:
   `- [{id} #{n}] path:line — <one sentence>`. Never open an issue for one;
   a human triages the parking lot in batches.
2. **KB distill.** For each **generalizable** confirmed `must` (a rule a
   future implementer could follow; skip one-offs):
   ```bash
   node scripts/harness/kb.mjs catalog          # merge into an existing lesson when it matches
   ```
   Write a JSON array to `.harness/notes/kb-{n}.json`, items shaped
   `{ "match_id"?, "title", "category", "rule", "rationale": "{id} (#{n}): <what went wrong>", "severity": "must", "source_issue": {n}, "file_globs": [...] }`
   (category: architecture, error-handling, unsafe-ffi, platform-parity,
   api-contract, user-files, correctness, data-integrity, performance,
   testing, determinism, accessibility, security), then
   `node scripts/harness/kb.mjs apply --in .harness/notes/kb-{n}.json`.
   Never graduate a lesson — that is a human step.

```bash
git add dev-docs/harness/parking-lot.md dev-docs/harness/kb/lessons
git commit -m "chore({id}): park out-of-diff notes, distill review lessons" && git push
```
Skip the commit when neither changed.

End: `GOCYCLE_OK`.

---

## Stage `finalize`

`GOCYCLE_PHASE: finalize`

CI is green (the verdict file says so). Now prove each criterion.

1. **Merge readiness:**
   `merge_readiness({ repoId, diff: "<git diff origin/main...HEAD>", predictedFilePaths, predictedCoChange })`.
   `gate: block` → fix the `unresolved[]` items if they are real (preflight,
   commit, push, then stop here with `GOCYCLE_OK` — the harness sees the new
   head commit, waits for CI and runs finalize again), or write in the notes why
   each does not apply and continue.
2. **Criteria.** For each acceptance criterion, name its evidence: the test
   that proves it, plus the CI run (job URL from the verdict file); a bench
   number; a screenshot. Then:
   - proven → `node scripts/harness/issue.mjs tick {n} --index i --evidence "<test name>; <CI job URL>"`
   - numeric, bench job not yet built → `node scripts/harness/issue.mjs pending {n} --index i --note "<local numbers>"`
   - not provable → `GOCYCLE_BLOCKED: criterion i cannot be verified: <why>`.
   Do not tick the Definition of done list — the merge gate owns it.
   Print `GOCYCLE_METRIC: criteria | ticked=T pending=P`.
3. **PR body:** rewrite the Verification table with the real evidence
   (`gh pr edit {pr} --body-file -`). Keep `Closes #{n}`.
4. **Summary comment** on the PR:
   ```markdown
   ## go-harness summary — {id}
   CI: windows ✓ macos ✓ linux ✓ (<run link>)   Bench: <numbers or "not needed" or "pending E9.1">
   Review: <cycles> cycle(s), <must fixed> must fixed, <disputed> disputed (resolved by verifier)
   Advisory (not fixed, for a human to judge): <should rows: path:line — headline, or "none">
   Parked out-of-diff notes: <count> (dev-docs/harness/parking-lot.md)
   PureContext: merge_readiness <gate>
   ```
5. `gh pr ready {pr}`.

End: `GOCYCLE_OK`.

---

## Error handling

| Situation | Action |
|-----------|--------|
| `gh` not authenticated | `GOCYCLE_FAIL: gh not authenticated` |
| Workspace not there (no `Cargo.toml` or `package.json`) | `GOCYCLE_BLOCKED: workspace missing — E1.1–E1.3 are human stories` |
| PureContext tools not available | continue with grep/Read; write "PureContext unavailable" in the notes (the loop guard watches for this) |
| Rebase conflict on `origin/main` | resolve, re-run preflight; a conflict in another story's area you cannot judge → `GOCYCLE_BLOCKED` |
| Same preflight error after 3 fix rounds | `GOCYCLE_FAIL: <command>: <error>` |
