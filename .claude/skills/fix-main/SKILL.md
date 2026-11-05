---
name: fix-main
description: |
  Make `main` green again. Run by the keeper lane of go-harness.ps1 when the
  newest CI run on main is red with a failed step after one re-run. Reads the
  keeper's verdict file (failed jobs, their step, a log excerpt, the commits since
  the last green run), finds the cause, fixes a flaky test for good or reverts the
  commit that broke main, verifies locally and opens a PR. Never merges. Emits
  GOCYCLE_PHASE markers and ends with a GOCYCLE_ sentinel.
  Invoke as: /fix-main --ci <verdict.json>
  Triggers on: fix main, red main, keeper fix.
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

# fix-main — make `main` green again

You run in the keeper's worktree, detached at `origin/main`. Nobody is waiting
for a reply; the keeper reads your last line. **Read `CLAUDE.md` first**: its
rules do not bend here either.

The verdict file (`--ci`) has: `sha` (the red commit), `run_id`, `url`, `kind`
(`test` here: a step failed), `failed[]` with `name` (OS), `step`, `excerpt`
(the log lines around the error), `last_green_sha` and `suspects[]` (the commits
between the last green run and `sha`, oldest first).

## Markers and sentinels (REQUIRED)

Plain text, alone on a line: `GOCYCLE_PHASE: read | branch | implement | preflight | pr`
before each phase. The very last line of the run is exactly one of:

- `GOCYCLE_OK` — a PR with the fix is open and the local checks passed.
- `GOCYCLE_BLOCKED: <one-line reason>` — a human must decide (see below).
- `GOCYCLE_FAIL: <one-line reason>` — tooling broke and three tries did not help.

## read

`GOCYCLE_PHASE: read`

1. Read the verdict file. Note every failed job, its step and the first real
   error in the excerpt (a test name, a compiler error, a lint rule).
2. Fetch the full log when the excerpt is not enough:
   `gh api repos/{owner}/{repo}/actions/jobs/<job id>/logs` (the id is in the
   job `url`'s last path segment, or in `failed[].id`).
3. Decide which case you are in:
   - **Flaky**: the failing test does not fail locally, or it depends on real
     time, timer order, network, file-system timing, or the speed of a shared
     runner. Today's examples in this repo: a test that slept 60 ms of real time
     while a mock fired 2 ms timers; a test that read a scroll position before
     the browser settled; an fps threshold on a loaded runner.
   - **Regression**: one of the `suspects` changed the code the failing step
     covers. `git log -p <sha> -- <paths>` and the test tell you.
   - **Environment**: a missing system package, a changed runner image, a new
     toolchain version. Nothing in the tree is wrong.
4. Reproduce locally when the failure is in a test you can run here (Vitest,
   `node --test`, `cargo nextest run -p <crate> <filter>`, Playwright browser
   mode). Run a suspected flaky test several times:
   `for i in 1 2 3 4 5; do npx vitest run <file> -t "<name>" || break; done`.

## branch

`GOCYCLE_PHASE: branch`

```bash
git fetch origin
git checkout -B fix-main-<sha7> origin/main      # sha7 = the first 7 characters of `sha`
```

If a PR from `fix-main-<sha7>` already exists, check it out and continue there.

## implement

`GOCYCLE_PHASE: implement`

Pick exactly one of these; do not mix them.

- **Flaky test → make it deterministic.** Fake timers (`vi.useFakeTimers` with
  `toFake` limited to what the test needs), `expect.poll` / `await expect(...)`
  instead of a fixed sleep, an explicit settle instead of a guess, a measured
  baseline instead of an absolute threshold. The test must still fail when the
  behaviour it covers breaks. **Never** delete the test, skip it, retry it in a
  loop, widen an assertion so it passes, or raise a bench budget.
- **Regression with an obvious, small fix** (a wrong path separator, a missing
  `await`, an off-by-one the test shows): fix the code and keep the test.
- **Regression that is not obvious, or would take real design:** revert the
  suspect commit: `git revert --no-edit <sha>`. If it was a squash-merged story
  (`(#<pr>)` in its title), say in the PR body which issue needs to be reopened;
  do not reopen it yourself.
- **Environment:** print `GOCYCLE_BLOCKED: environment: <what changed>; nothing in
  the tree is wrong` and stop. The keeper keeps re-running main's jobs.

Hard limits. Stop with `GOCYCLE_BLOCKED` instead of doing any of these:

1. Changing a request/response type in `crates/fm-api` (CLAUDE.md rule 6).
2. Changing `.github/workflows/*`, `scripts/bench/*` targets or budgets, or
   `go-harness.config.json`.
3. Adding a dependency.
4. A fix that touches more than about 5 files, or more than one story's area:
   that is a story, not a keeper fix; name the files and the choice.

Use PureContext as in `/go-cycle`: `resolve_repo` for the worktree, `index_file`
after every write, `prepare_change` before touching shared code.

## preflight

`GOCYCLE_PHASE: preflight`

Run what the change can affect, in this order, and all of it when in doubt:

```bash
pnpm lint                                       # always
pnpm test                                       # frontend or harness change
cargo nextest run --workspace                   # any Rust change
pnpm gen:types && git diff --exit-code apps/desktop/src/api/types.ts   # any fm-api or Rust change
```

A fixed flaky test runs five times in a row and passes every time. A revert
runs the step that failed on main.

## pr

`GOCYCLE_PHASE: pr`

Commit: `fix(main): <what>` with a body that names the cause (the main run URL,
the failing step, why it failed) and why this fix is right. Then:

```bash
git push -u origin HEAD
gh pr create --base main --title "fix(main): <what>" --body-file - <<'EOF'
Keeper fix for the red main run <url> (commit <sha7>).

## Cause
<the failing job and step; flaky / regression from <suspect> / environment; the evidence>

## Fix
<what changed and why it is the right fix, not a workaround>

## Verification
<commands run and their result; "test ran 5x green" for a flaky test>

<for a revert: "Reverts <sha7> (<title>, PR #<n>). Issue #<m> needs to be reopened by a human.">

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
```

Not a draft: the keeper merges it when its CI is green. You never merge.

End: `GOCYCLE_OK`.
