# go-harness — user manual

`go-harness.ps1` drives Font Manager stories from a GitHub issue to a green,
reviewed, ready PR. It is tr-harness (toprope, profile) adapted to this repo,
as briefed in `dev-docs/HARNESS-ADAPTATION.md`.

## Parts

| File | Job |
|------|-----|
| `go-harness.ps1` | the loop: picks stories, runs stages, waits for CI, merge gate, panel, telemetry |
| `go-harness.config.json` | merge mode, budget base, CI check names, lanes, review settings |
| `.claude/skills/go-cycle/SKILL.md` | one story, one stage: `build`, `ci-fix`, `review`, `finalize` |
| `.claude/commands/fm-review.md` | the code review (must/should, verified, verify-mode later cycles) |
| `scripts/harness/pick.mjs` | Definition of Ready, lane choice, ordering, overlap, dry run |
| `scripts/harness/ci-wait.mjs` | waits for `ci / windows`, `ci / macos`, `ci / linux` (+ bench) |
| `scripts/harness/issue.mjs` | fm-meta, criteria tick with evidence, `harness:blocked` |
| `scripts/harness/kb.mjs` | review knowledge base (lessons, retrieval, graduation) |
| `scripts/harness/epic-sync.mjs` | summary comment on the epic when a story closes; closes done epics |
| `scripts/harness/loop-check.mjs` | convergence guard after each story |
| `scripts/harness/assets.mjs` | publishes UI screenshots to the `harness-assets` branch |
| `scripts/harness/review-counts.mjs` | parses a review file's findings index (counts + per-lens split) |
| `scripts/harness/dashboard/` | local telemetry dashboard: `pnpm dashboard` |
| `dev-docs/harness/parking-lot.md` | out-of-diff review notes for human triage |
| `dev-docs/harness/review-rules.md` | graduated lessons (generated; imported by `CLAUDE.md`) |
| `.harness/` (git-ignored) | logs, state, CI verdicts, reviews, notes, telemetry, `fm-run-summary.md` |

Tests: `node --test "scripts/harness/**/*.test.mjs"` (part of `pnpm test`).

## Before the first run

1. E1.1, E1.2 and E1.3 are `human` stories. Do them interactively. The harness
   refuses to start while `origin/main` has no `Cargo.toml` and `package.json`,
   and the CI step reports `no-ci` until `.github/workflows/ci.yml` exists.
2. The harness files must be on `origin/main`: each lane works in its own git
   worktree checked out from `main`, and the skill and review command are read
   from that checkout.
3. `gh auth status` is green; `claude mcp list` shows `purecontext-mcp ✔ Connected`.
4. Optional, once: `purecontext-mcp hooks --install --git` keeps the index fresh
   on checkout, merge and rebase in every worktree.

## Commands

```powershell
./go-harness.ps1 -DryRun                    # what each lane would pick, and why not the others. Touches nothing.
./go-harness.ps1 -Lane core -NoWorktree -MaxStories 1   # first loop: one story, in this checkout, watched
./go-harness.ps1 -Lane core                 # terminal 1
./go-harness.ps1 -Lane ui                   # terminal 2
./go-harness.ps1 -Lane core -MaxStories 10   # night run (merge: auto): blocked stories are skipped
./go-harness.ps1 -Lane core -Issues 42,43   # only these (still Definition-of-Ready checked)
./go-harness.ps1 -EpicSync                  # post epic summaries for stories a human merged
./go-harness.ps1 -GraduateOnly              # KB graduation review (interactive; run it when you are back)
```

| Parameter | Default | Meaning |
|-----------|---------|---------|
| `-Lane core\|ui` | — | required for a run |
| `-Issues` | all | restrict the lane to these issue numbers |
| `-MaxStories` | 0 | stop after N stories (0 = until nothing is ready) |
| `-Model` | opus | `sonnet`, `opus`, `fable` |
| `-ReviewCycles` | config (3) | cycle 1 full review, cycles 2+ verify the fix |
| `-MaxResumes` | 2 | resumes of one stage after a crash / missing sentinel |
| `-NoWorktree` | off | run in this checkout instead of the lane worktree |
| `-NoWaitForReset`, `-MaxWaits` | wait, 12 | usage limits: sleep until reset, re-dispatch the same attempt |
| `-MaxConsecutiveFails` | 3 | blocked stories in a row → stop the lane |
| `-StopOnBlock` | off | stop the lane at the first blocked story |
| `-NoIdleWait` | off | end the lane when nothing is ready, even while the other lane still works |
| `-Graduate` | off | ask the interactive KB graduation question when the lane ends |
| `-NoLoopGuard` | off | skip the loop guard |
| `-Panel`, `-NoPanel` | auto | force the pinned panel on (JetBrains terminal) or off |

## How a story runs

```
pick (DoR) → claim → build → CI → [ci-fix | sync → CI]* → review → CI → [ci-fix | sync → CI]* → finalize → merge gate
```

- **Pick** (`pick.mjs`). Definition of Ready: `type:story`; none of `human`,
  `criteria:tbd`, `harness:in-progress`, `harness:blocked`; every `blocked_by`
  issue closed (read from the API each time); open and unassigned (or assigned
  to the bot). The pick gets `harness:ready`, then `harness:in-progress` and the
  bot as assignee. Lane order: phase, epic, story, size. A story whose predicted
  areas (crates and frontend paths named in its body, plus its track's home)
  overlap the other lane's open story PR waits. A lane resumes its claimed
  story whose PR is still a draft. With `merge: human` it stops picking while
  `lane_max_unmerged` ready PRs wait for a human. With `merge: auto` a claimed
  story with a ready PR goes back through CI and the merge gate (it did not merge
  because `main` was red or the lane stopped); a PR labelled
  `harness:human-merge` is left for a person and does not hold the lane.
- **Idle.** Nothing to start, but the other lane still has a story in flight
  (its merge can close a dependency or free an overlapping area): the lane waits
  `idle.poll_min` (10) minutes and asks the picker again, for at most
  `idle.max_hours` (8). Nothing in flight anywhere on two picks in a row (one poll apart) → the lane ends; the second pick covers the other lane being between two stories.
- **build** — read the issue, epic, ADR, PureContext `get_task_context`, KB
  pitfalls; branch `E3.2-slug` off `origin/main`; implement core → api →
  adapter; tests per criterion; preflight (`pnpm lint`, `cargo nextest run
  --workspace`, `pnpm test`, `pnpm gen:types` + diff check); `verify_change`;
  draft PR `E3.2 <title>` with `Closes #n`. UI stories capture screenshots at
  1,280×800 and 1,000×700.
- **CI** — the harness waits (no tokens). All three OS must be `success`. Red →
  `ci-fix` with the job log excerpt and the platform hint. 3 red rounds →
  `harness:blocked`. The PR conflicts with `main` (GitHub runs no CI on it) →
  `sync`. GitHub did not run the jobs at all (billing, spending limit) → the lane
  stops at once and keeps the story's state; the next run resumes it at CI.
  Stories with numeric criteria (a number with ms, s, fps,
  MB) get the `bench` label on the CI wait before finalize; `bench-report.json`
  `pass: false` counts as red. The bench job (`.github/workflows/bench.yml`,
  E9.1) runs `scripts/bench.sh` on all three OS and merges the results into
  `bench-report.json` (`"<os>.<metric>"` entries); a metric not measured yet
  has `pass: null` and a note.
- **sync** — merge `origin/main` into the branch, resolve the conflicts (keep
  both sides; the same thing added twice → keep one; lock files and `types.ts`
  regenerated), full preflight, push, CI again. At most
  `auto_merge.max_syncs` (2) per story, then `harness:blocked`.
- **review** — `/fm-review`, see below. A `must` that is still open after 2 fix
  rounds → `harness:blocked`.
- **finalize** — `merge_readiness`; each criterion ticked with an evidence
  comment (test + CI job URL / bench number / screenshot); PR body table;
  summary comment; `gh pr ready`.
- **merge gate** — `merge: human`: never merges; the PR waits for you.
  `merge: auto`: see "Merge gate" below.

Later stages resume the build conversation (`claude --resume <session>`), so the
fixer keeps the implementation intent. `.harness/notes/issue-<n>.md` is the
memory if a cold start is needed, and the decision record reviewers read.

## Merge gate

`go-harness.config.json` → `merge`:

- `human` — the harness never merges. The ready PR waits for you.
- `auto` — after finalize, `scripts/harness/merge-check.mjs` decides:

| Verdict | When | What the harness does |
|---|---|---|
| merge | CI green on all three OS, review has no open `must`, branch up to date with `main` | squash-merge (pinned to the checked head commit), delete the branch |
| update | the branch is behind `main` | `gh pr update-branch`, then CI again (red → ci-fix as usual), then the gate again; at most `auto_merge.max_branch_updates` (3) times |
| sync | the branch conflicts with `main` | the `sync` stage, then CI, then the gate again |
| human | track in `auto_merge.exclude_tracks` (empty in this repo; the default is `platform`) or the PR removes/rewrites lines of `apps/desktop/src/api/types.ts` outside `contract_epics` | comment on the PR, label `harness:human-merge`, leave it ready; the lane goes on |
| block | a draft, or the merge call failed | `harness:blocked` |
| stop | the newest commit on `main` has a failed required check (newest run per job) | re-run main's failed jobs once (a flaky test), check every `main_red.poll_min` (10) minutes; green → the gate again; still red after `main_red.max_hours` (6) → stop the lane, and the next run merges the PR |

Merging only a branch that is up to date with `main` and green means `main` after
the merge is the exact tree CI just tested, even with two lanes merging. Adding to
`fm-api` (new types, new optional fields) only adds lines to `types.ts`, so it
merges; removing or rewriting a line means an existing type changed.

Night run: `./go-harness.ps1 -Lane core -MaxStories 10` in one terminal, the
same with `-Lane ui` in another. A story that needs a human ends
`harness:blocked` and the lane takes the next one; 3 blocked in a row
(`-MaxConsecutiveFails`) look systemic and stop the lane. Nothing waits for a key:
the KB graduation question is asked only with `-Graduate` (or run
`-GraduateOnly` when you are back). `-StopOnBlock` stops at the first blocked
story instead.

## Stop conditions → `harness:blocked` + a comment with why and what was tried

CI red 3 times; a `must` finding open after 2 fix rounds; a change to an existing
`fm-api` type outside E1/E25; a dependency not in the spec's library table or
rejected by cargo-deny; contradictory or unverifiable criteria; a conflict with
an ADR; the story over budget (S = 1×, M = 2.5×, L = 5× `budget_base_usd`) before an
agent stage (waiting for CI and the merge gate go on).
To retry: fix the cause, remove `harness:blocked`, run the lane again.

## The review — why it is different from tr-harness

The profile project's record: 562 findings in 23 cycles, 74% Low, 23% Medium,
2.5% High; two thirds deferred; review the most expensive phase of every task.
`/fm-review` changes the rules:

| tr-harness | fm-review |
|------------|-----------|
| 5 severities (Critical … Style) | `must` (blocks) and `should` (advisory, max 2 per lens) |
| 5 lenses always | SPEC, COR, TST always; REUSE, FFI, A11Y only when the diff calls for them |
| review could start before CI | review starts after the 3-OS matrix is green; lint/format/type findings are banned |
| any plausible issue counted | every finding needs `Where`, `Failure`, `Evidence`; one adversarial verifier must reproduce each `must` |
| every cycle a fresh full review | cycle 2+ checks only the fix delta and the prior musts |
| Medium fixed "when safe" | `should` is never auto-fixed; it goes to the PR summary for a human |
| out-of-diff could become issues | out-of-diff goes to the parking lot, never to issues |
| reviewers blind to intent | the implementer notes record decisions with their constraint; a fixer may dispute a finding there and the verifier judges it |

FFI findings always block. A11Y and SPEC block on `must`, advise on `should`.
Every lens gets the same context pack: the story body, `CLAUDE.md`, graduated
rules, the ADR list, the local spec export (`docs/spec/` when present), the CI
verdict, the implementer notes, the screenshots and the PureContext `repoId`.

## PureContext in the loop

| Where | Calls |
|-------|-------|
| every stage start | `resolve_repo` (not `list_repos` — it is huge), `index_folder` only when missing or behind |
| build | `get_task_context`, `prepare_change` before shared edits, `check_consistency` before new `pub` items, `check_*_safe` before rename/delete/move, `index_file` after each write, `verify_change` before the PR |
| review lenses | `get_symbol_source`, `find_references`, `get_blast_radius`, `search_similar`, `check_consistency` (REUSE) |
| verifier | `find_references`, `get_symbol_source` to try to disprove each finding |
| finalize | `merge_readiness` |

The panel shows `pc N` (PureContext calls this story; yellow at 0). Telemetry
records `purecontext_calls` per phase, stage and story; the loop guard warns when
two recent stories made none. `.vue` files are not parsed by the index: agents
read them and use grep.

## Telemetry (`.harness/`)

| File | One record per |
|------|----------------|
| `tasks.jsonl` | claude stage run: stage, outcome, billed USD (authoritative), phases with tokens and est. cost, PureContext calls, session id |
| `review-cycles.jsonl` | review cycle: mode, must/should/rejected/resolved, fixed/disputed, review vs fix time |
| `stories.jsonl` | finished story: outcome, budget vs billed, CI attempts and failed OS, review rounds, must fixed, advisory, rejected, bench, claim→green |
| `fm-run-summary.md` | one table row per finished story |
| `state/<lane>.json` | the running story's stage and session (resume after a restart) |
| `logs/` | raw stream-json per stage run |

### The dashboard

```powershell
pnpm dashboard        # http://localhost:8090 — reads .harness/ fresh on every request
```

A local, zero-dependency page (`scripts/harness/dashboard/`, adapted from the
toprope dev-cycle dashboard; `--port`, `--data <dir>`). It refreshes every 30 s
and follows the OS light/dark theme; every chart has a table view. Filters:
period, lane, outcome.

| View | What it answers |
|------|-----------------|
| Running now | which story each lane is on, its stage, spend vs budget (`.harness/state/`) |
| KPIs | stories done/blocked, billed cost, over budget, claim→green, CI first-pass green, confirmed must per story, verifier rejection rate, advisory count, PureContext calls |
| Billed cost per day, by stage | where the money went: build, ci-fix, review, finalize (real billed cost) |
| Confirmed must findings per story | first-review blockers over time with a rolling average; falling means the KB works |
| Findings by lens | per lens: confirmed must / should / rejected by the verifier; a lens with a high rejection share needs a narrower charter |
| Where the cost and the time go | billed cost and median time per stage, plus the (free) CI wait |
| Billed vs budget | each story against its size budget |
| CI failures by platform | which OS breaks first; local preflight is Windows, so macOS/Linux reds are platform bugs |
| Review cycles | full review (cycle 1) vs verify cycles: must, should, rejected, resolved, fixed, disputed |
| Stories | sortable table; expand a row for its stage runs, review cycles (with the per-lens split) and build phases |
| Epics, Review KB, Parking lot | stories per epic, lesson lifecycle counts, out-of-diff notes waiting for triage |

When a story's issue closes, `epic-sync.mjs` posts its summary line on the epic
(lane start and end, or `-EpicSync`) and closes an epic whose stories are all closed.

The verifier's `rejected` count is the false-positive signal: watch it per lens
in `review-cycles.jsonl`. A lens with many rejections needs a narrower charter.

## The review KB

Confirmed `must` findings that generalize become lessons
(`dev-docs/harness/kb/lessons/<id>.json`, one file each so two lanes do not
conflict). Seen in 2 stories → `active`: injected into the implementer for
matching paths. Graduation into `review-rules.md` (loaded into every agent) is
a human decision: `./go-harness.ps1 -GraduateOnly` ranks the candidates and you
pick; then commit the changed files in a PR.

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| "origin/main has no Cargo.toml" | E1.1–E1.3 are not merged yet |
| "origin/main has no go-harness.config.json" | commit and push the harness, or use `-NoWorktree` |
| CI `no-ci` → blocked | `.github/workflows/ci.yml` missing (E1.2) |
| CI `timeout` | raise `ci.timeout_min`; check the runners |
| check never matches | the job names differ: set `ci.required_checks` to the names `gh pr checks` shows |
| `pc 0` in yellow | `claude mcp list`; `resolve_repo` on the worktree path |
| lane says WAIT | `merge: human` and a ready PR waits for your merge (`lane_max_unmerged`) |
| lane says IDLE and waits | the other lane's PR touches the same area or closes a dependency; it asks again every 10 min (`-NoIdleWait` to end instead) |
| lane stopped: "main stayed red" | fix `main`; the next run takes the ready PR back to the merge gate |
| lane stopped: "GitHub did not run CI" | fix billing / the spending limit, then run the lane again: it resumes the story at CI |
| a story stays `harness:in-progress` after you merged | it closes with the PR (`Closes #n`); `-EpicSync` posts its epic line |
| leftovers in a worktree | the harness stashes them before a new story: `git -C ../font-manager-core stash list` |
