# Adapting tr-harness for the Font Manager repository

**Purpose.** This file briefs Claude Code (or any engineer) on how tr-harness must change to
drive development of the Font Manager — a free, open-source, cross-platform font manager in
Rust (Windows, macOS, Linux) with a Tauri 2 + Vue 3 UI. Read it fully before touching the
harness; every section is a requirement unless marked "optional".

Reference documents (ask the owner for access if a link does not open):

- Specification: https://claude.ai/code/artifact/b90dd55a-4c52-4f44-b868-46878caa6f11
- Task breakdown: https://claude.ai/code/artifact/195e5ba6-05c4-4eae-b5a3-092d9361acd5
- Backlog seed: `issues.json` + `create_issues.py` (already run; issues exist on GitHub)
- Target repo conventions: `CLAUDE.md` in the font-manager repository (copy in this folder)

The existing tr-harness loop — GitHub issue → implement → build & test → PR → independent
reviewer agents → fix → verify state → merge, with cost and outcome telemetry per run — stays.
What follows is what changes around it.

---

## 1. The target repository at a glance

| Thing | Value |
| --- | --- |
| Layout | Cargo workspace (`crates/fm-core`, `fm-api`, `fm-platform`, `fm-cli`, `fm-mcp`) + pnpm workspace (`apps/desktop` Tauri 2 + Vue 3 + TS, `plugins/`) |
| Branching | Trunk-based; branch per story named `E3.2-short-slug`; squash merge; PR title `E3.2 <story title>`; PR body references `Closes #<n>` |
| Lint | `pnpm lint` (clippy `-D warnings`, rustfmt, cargo-deny, ESLint, Prettier) |
| Tests | `cargo nextest run --workspace`, `pnpm test` (Vitest), Playwright for the UI |
| Type contract | `pnpm gen:types` regenerates `apps/desktop/src/api/types.ts` from `fm-api`; CI fails if stale |
| CI | `.github/workflows/ci.yml` runs the above on `windows-latest`, `macos-latest`, `ubuntu-latest` |
| Bench | `.github/workflows/bench.yml` — nightly on `main`; on a PR when it carries the `bench` label |
| Rule that never bends | No business logic in the Vue frontend or in any adapter (CLI, MCP, plugin IPC); everything goes through `fm-api` into `fm-core` |

---

## 2. Issue model the harness must read

Every issue was created by `create_issues.py` and follows one shape.

### 2.1 Epics and stories

- **Epic**: label `type:epic`, title `E3 Index and search`. Its stories are attached as
  **real GitHub sub-issues** (not task-list links). List them with
  `GET /repos/{owner}/{repo}/issues/{epic_number}/sub_issues`.
- **Story**: label `type:story`, title `E3.8 Supports filter (…)`. The harness works on
  stories only; it never implements an epic directly. An epic is done when all its
  sub-issues are closed — GitHub shows the progress bar; the harness closes the epic
  when the last story closes (optional automation).

### 2.2 Dependencies

Every `Depends on` relation is a **real "blocked by" dependency**. Read it with
`GET /repos/{owner}/{repo}/issues/{n}/dependencies/blocked_by`
(API version header `X-GitHub-Api-Version: 2026-03-10`).

**Definition of Ready** (the only condition under which the harness may start a story):

1. label `type:story`
2. no label `human`, no label `criteria:tbd`, no label `harness:in-progress`, no label `harness:blocked`
3. every issue in `blocked_by` is **closed**
4. issue is open and unassigned (or assigned to the harness bot)

When all four hold, the harness sets `harness:ready` (idempotent), then claims the story by
swapping it for `harness:in-progress`. `harness:ready` is informational for humans; the
harness recomputes readiness from `blocked_by` every time, never from the label alone.

### 2.3 Machine-readable metadata

Every issue body ends with a block the harness parses instead of reading prose:

```
<!-- fm-meta
id: E3.8
epic: E3
phase: 1
size: M            # S ≤ 1 day · M 2–3 days · L 4–5 days
track: core        # core | ui | platform | ops | data | plugins
human: false
depends: [E3.7, E3.4]
-->
```

Parse it as `key: value` lines; `depends` is a bracketed, comma-separated list of story or
epic IDs. `issues.map.json` (written by the seed script) maps IDs to issue numbers if the
harness prefers a static lookup.

### 2.4 Acceptance criteria

The `## Acceptance criteria` section is a checkbox list. Each line is a verifiable
statement. The harness:

- passes the full list to the implementing agent as the definition of the work;
- passes the same list to every reviewer agent;
- ticks each box in the issue body (edit the body, flip `- [ ]` to `- [x]`) only when the
  verification step confirmed it, with a one-line comment on the issue naming the evidence
  (test name, CI job, bench number, screenshot).

The `## Definition of done` list is the same for every story and is enforced by the
verification step, not by the agent's own judgment.

### 2.5 Labels the harness owns

| Label | Set by | Meaning |
| --- | --- | --- |
| `harness:ready` | harness | all blockers closed; may be picked |
| `harness:in-progress` | harness | claimed; a branch and PR exist or are being created |
| `harness:blocked` | harness | stopped; a human must look (see §6) |
| `bench` | harness | on the PR, requests the bench job (see §4.3) |

Labels the harness only reads: `human`, `criteria:tbd`, `type:*`, `phase:*`, `size:*`, `track:*`.

---

## 3. Lanes: two tracks in parallel

The MVP has two tracks that rarely touch the same files: **core** (Rust) and **ui** (Vue +
Tauri shell). Run them as two harness lanes:

- Lane selection: a story's `track` in `fm-meta` — `core`, `platform`, `data`, `ops` → core
  lane; `ui` → ui lane; `plugins` → core lane (Phase 3+).
- Each lane runs in its **own git worktree** of the font-manager repo, so builds and
  `node_modules`/`target` caches never collide.
- Lane concurrency: one story in progress per lane. A story whose blockers are all closed
  but whose files overlap with an in-progress story in the other lane (same crate or same
  `apps/desktop/src` subtree, detected by the PR diff of the in-progress story) waits.
- `human` stories are never picked by either lane; they are listed in the run summary as
  "waiting on a human" with the IDs of the stories they block.

Ordering inside a lane: lowest phase first, then lowest epic number, then lowest story
number, then smallest size. This reproduces the sprint order from the breakdown without a
separate sprint file.

---

## 4. Verification: the CI matrix is the judge

tr-harness currently verifies by running build and tests locally. For this repo that is
insufficient — the development machine is Windows, but a story is done only when all three
platforms are green.

### 4.1 Local pre-flight (fast feedback, not the verdict)

Before pushing, the implementing agent runs in its worktree:

```
pnpm lint
cargo nextest run --workspace
pnpm test
pnpm gen:types && git diff --exit-code apps/desktop/src/api/types.ts
```

A failure here loops back to the agent without opening a PR.

### 4.2 Remote verdict

After the PR is opened, the harness **waits for the CI matrix** and reads check results
(`GET /repos/{r}/commits/{sha}/check-runs`). The story proceeds to review only when
`ci / windows`, `ci / macos` and `ci / linux` all conclude `success`. A failure on one OS is
fed back to the agent with that job's log excerpt and the hint "this passed locally on
Windows; the failure is platform-specific — look for path separators, line endings, case
sensitivity, OS font APIs, timing".

Retry budget per story for CI failures: 3. After that → `harness:blocked` (§6).

### 4.3 Performance criteria

Stories whose acceptance criteria contain a number with a unit (ms, s, fps, MB) need the
bench job. The harness:

1. adds the `bench` label to the PR, which triggers `.github/workflows/bench.yml`;
2. waits for its check and reads the JSON report it uploads as an artifact
   (`bench-report.json`: `{metric: {value, unit, target, pass}}`);
3. treats any `pass: false` as a failed criterion and loops back with the numbers.

Until E9.1 is merged there is no bench job. Before that, numeric criteria are verified by
the agent running `scripts/bench.sh` (or the equivalent `cargo bench`/timing test the story
adds) locally and pasting the numbers in the PR; the harness marks these criteria "verified
locally, pending E9.1" rather than ticked. **Build E9.1 right after E3.3** so this window
stays short.

### 4.4 UI stories

For `track: ui` stories the verification step also:

- runs Playwright against the Vue app in browser mode on the `MockApi` (fast, headless,
  all platforms) and against the Tauri shell where `tauri-driver` is set up (Linux first);
- captures screenshots of the affected screens at 1,280×800 and 1,000×700 (the inspector
  collapses under 1,100 px) and attaches them to the PR;
- hands those screenshots to the accessibility reviewer (§5).

---

## 5. Reviewers

Keep the existing independent reviewer agents. Give **every** reviewer the same context
pack:

- the story's issue body (criteria + fm-meta);
- the spec section(s) the story cites (spec URL above; the harness can keep a local
  Markdown export of the spec under `docs/spec/` in the target repo and pass the matching
  section);
- the five ADRs under `docs/adr/`;
- `CLAUDE.md` from the target repo;
- the PR diff and the CI/bench results.

Add three specialist reviewers, each a separate agent with a narrow prompt:

| Reviewer | Runs for | Looks for |
| --- | --- | --- |
| **Spec compliance** | every story | Does the change do what the cited spec section says, no more and no less? Any behavior the spec forbids (moving user files uninvited, deregistering fonts the app did not register, logic in the frontend or adapters)? |
| **Unsafe / FFI** | any diff touching `crates/fm-platform`, `unsafe`, `windows::`, `core_text`, `objc2`, `fontconfig`, registry or file-system writes outside the app's own folders | Memory safety, error handling per face (never all-or-nothing), cleanup on quit and on crash, no admin elevation, nothing written to system font folders |
| **Accessibility** | `track: ui` | Labels on interactive elements, focus order, keyboard reachability of the new UI, contrast of new chrome, reduce-motion, screen-reader text for list rows; reviews the screenshots from §4.4 |

Reviewer findings follow the existing fix loop. The unsafe/FFI reviewer's findings are
**blocking** (a story cannot merge with an open finding from it); the other two are
blocking for "must" findings and advisory for "should".

---

## 6. Stop conditions and the human gate

The harness stops a story and sets `harness:blocked` (with a comment that says why and
what it tried) when:

- CI fails 3 times on the same platform;
- a reviewer finding survives 2 fix rounds;
- the agent proposes changing `fm-api` request/response types in a story that is not in
  epic E1 or E25 (contract changes are deliberate, not incidental);
- the agent wants to add a dependency that `cargo-deny` rejects or that is not in the
  spec's library list (ask, don't add);
- cost for the story exceeds its budget (§7);
- the acceptance criteria turn out to be contradictory or unverifiable.

**Merge gate, Phase 1:** the harness never merges. It leaves the PR in "ready, all checks
green, reviews addressed" and posts a summary comment; a human merges. Record this as a
config flag `merge: human`.

**After one sprint of clean merges** the owner may set `merge: auto-sm` — the harness
merges stories of size S and M on green, and leaves L stories and anything with
`track: platform` for a human. Never `merge: auto` for platform or `fm-api` changes.

---

## 7. Budgets and telemetry

Keep tr-harness's per-run cost and outcome telemetry. Add:

- a per-story budget derived from size: S = 1×, M = 2.5×, L = 5× a base amount the owner
  sets in config; the stop condition in §6 uses it;
- per-story fields: platform(s) that failed CI, reviewer rounds, bench deltas, wall-clock
  from claim to green;
- a `fm-run-summary.md` appended per run under `.harness/` in the target repo (git-ignored)
  and posted as a comment on the epic when a story closes, so the epic page tells the story.

---

## 8. Ordering note for the first days

The harness cannot verify anything until the workspace, CI and `fm-api` exist. Stories
E1.1, E1.2 and E1.3 carry the `human` label on purpose: do them interactively in Claude
Code. From E1.4 onward the harness takes over. Run the first few stories with
`merge: human` and watch one full loop end to end on a small story (E1.6 or E2.1) before
letting the lanes run unattended.

---

## 9. Checklist for the adaptation itself

- [ ] Issue reader: parse `fm-meta`, `blocked_by`, `sub_issues`; implement Definition of Ready (§2.2)
- [ ] Lanes: `track` → lane, worktree per lane, overlap check (§3)
- [ ] Pre-flight commands for this repo (§4.1)
- [ ] CI matrix wait + per-platform failure feedback; retry budget (§4.2)
- [ ] Bench label trigger + `bench-report.json` reader; "verified locally, pending E9.1" state (§4.3)
- [ ] Playwright + screenshot step for `track: ui` (§4.4)
- [ ] Reviewer context pack; three new reviewer prompts; blocking rules (§5)
- [ ] Stop conditions and `harness:blocked` comment; `merge:` config flag (§6)
- [ ] Budgets by size; new telemetry fields; epic summary comment (§7)
- [ ] Criteria tick-off with evidence comment (§2.4)
- [ ] Dry-run mode that prints which story each lane would pick and why, without touching the repo
