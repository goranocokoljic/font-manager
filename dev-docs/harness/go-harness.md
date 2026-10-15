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
| `scripts/harness/review-counts.mjs` | parses a review file's findings index |
| `dev-docs/harness/parking-lot.md` | out-of-diff review notes for human triage |
| `dev-docs/harness/review-rules.md` | graduated lessons (generated; imported by `CLAUDE.md`) |
| `.harness/` (git-ignored) | logs, state, CI verdicts, reviews, notes, telemetry, `fm-run-summary.md` |

Tests: `node --test scripts/harness/harness.test.mjs`.

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
./go-harness.ps1 -Lane core -Issues 42,43   # only these (still Definition-of-Ready checked)
./go-harness.ps1 -EpicSync                  # post epic summaries for stories a human merged
./go-harness.ps1 -GraduateOnly              # KB graduation review
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
| `-NoLoopGuard`, `-NoGraduate` | off | skip the guard / the graduation review |
| `-Panel`, `-NoPanel` | auto | force the pinned panel on (JetBrains terminal) or off |

## How a story runs

```
pick (DoR) → claim → build → CI → [ci-fix → CI]* → review → CI → [ci-fix → CI]* → finalize → merge gate
```

- **Pick** (`pick.mjs`). Definition of Ready: `type:story`; none of `human`,
  `criteria:tbd`, `harness:in-progress`, `harness:blocked`; every `blocked_by`
  issue closed (read from the API each time); open and unassigned (or assigned
  to the bot). The pick gets `harness:ready`, then `harness:in-progress` and the
  bot as assignee. Lane order: phase, epic, story, size. A story whose predicted
  areas (crates and frontend paths named in its body, plus its track's home)
  overlap the other lane's open story PR waits. A lane resumes its claimed
  story whose PR is still a draft, and stops picking while
  `lane_max_unmerged` ready PRs wait for a human.
- **build** — read the issue, epic, ADR, PureContext `get_task_context`, KB
  pitfalls; branch `E3.2-slug` off `origin/main`; implement core → api →
  adapter; tests per criterion; preflight (`pnpm lint`, `cargo nextest run
  --workspace`, `pnpm test`, `pnpm gen:types` + diff check); `verify_change`;
  draft PR `E3.2 <title>` with `Closes #n`. UI stories capture screenshots at
  1,280×800 and 1,000×700.
- **CI** — the harness waits (no tokens). All three OS must be `success`. Red →
  `ci-fix` with the job log excerpt and the platform hint. 3 red rounds →
  `harness:blocked`. Stories with numeric criteria (a number with ms, s, fps,
  MB) get the `bench` label on the CI wait before finalize; `bench-report.json`
  `pass: false` counts as red. Before E9.1 there is no bench job: the agent
  measures locally and the criterion is marked "verified locally, pending E9.1".
- **review** — `/fm-review`, see below. A `must` that is still open after 2 fix
  rounds → `harness:blocked`.
- **finalize** — `merge_readiness`; each criterion ticked with an evidence
  comment (test + CI job URL / bench number / screenshot); PR body table;
  summary comment; `gh pr ready`.
- **merge gate** — `merge: human`: never merges; the PR waits for you.
  `merge: auto-sm`: merges S and M; L, `track: platform` and any PR touching
  `crates/fm-api` wait for a human. There is no `auto`.

Later stages resume the build conversation (`claude --resume <session>`), so the
fixer keeps the implementation intent. `.harness/notes/issue-<n>.md` is the
memory if a cold start is needed, and the decision record reviewers read.

## Stop conditions → `harness:blocked` + a comment with why and what was tried

CI red 3 times; a `must` finding open after 2 fix rounds; a change to an existing
`fm-api` type outside E1/E25; a dependency not in the spec's library table or
rejected by cargo-deny; contradictory or unverifiable criteria; a conflict with
an ADR; the story over budget (S = 1×, M = 2.5×, L = 5× `budget_base_usd`).
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
| lane says WAIT | a ready PR waits for your merge (`lane_max_unmerged`) |
| a story stays `harness:in-progress` after you merged | it closes with the PR (`Closes #n`); `-EpicSync` posts its epic line |
| leftovers in a worktree | the harness stashes them before a new story: `git -C ../font-manager-core stash list` |
