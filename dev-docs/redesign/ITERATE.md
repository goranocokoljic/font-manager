You are running one iteration of the Font Manager UI/UX improvement loop. Font Manager is
a Tauri + Vue desktop app. Work autonomously; do not ask questions — decide, and log the
decision.

## Before anything

0. You are on the `redesign` branch (`git rev-parse --abbrev-ref HEAD`). If you are not,
   stop immediately and do nothing else — the loop script is responsible for the branch.
   Never commit to `main`.

1. Read `CLAUDE.md` (repo root) — its rules apply here unchanged — then
   `dev-docs/redesign/DESIGN.md` completely. It is the north star and the backlog.
2. Read `dev-docs/redesign/ITERATIONS.md` completely. It is the log of every previous iteration. Do not
   repeat, undo or re-litigate a previous iteration unless a review entry says to.
3. Check the preconditions below. If either is missing, this iteration is a **setup
   iteration**: build it, commit it, log it, stop.

### Architecture boundary (read this twice)

The frontend talks to the backend through exactly one module: `apps/desktop/src/api/`.
`useClient()` returns a typed `FmClient` — the Tauri implementation inside the app, and
`mock.ts` when the app runs in a plain browser. No `invoke` exists outside that folder, and
the types are generated from `fm-api`.

Keep it that way. This loop touches **views, components and stores only**. It does not
edit anything in `apps/desktop/src/api/`, does not add data or methods to `mock.ts`, does
not touch generated types, and does not touch Rust. If a backlog item needs data or a
command the `FmClient` does not expose today, it is **blocked**: write it up under
follow-ups (what the UI needs, in terms of the `FmClient` interface) and pick the next
item. Do not fake it in the store.

### Preconditions

- **Browser mode works:** `pnpm --filter @fm/desktop dev` serves the Vue app on
  `http://localhost:1420` using `mock.ts`, and every view and inspector tab renders with its
  fixture content. If something does not render in browser mode, that is a bug in a
  view/component/store, not a reason to extend the mock — fix it where it lives, or log it
  as blocked.
- **The screenshot script works:** `node scripts/redesign/shots.mjs <iteration>` drives the
  existing `@screens` Playwright spec (`apps/desktop/e2e/screens.spec.ts`, which already
  handles themes, routes, tabs, storage seeding and key presses) once per scene in its
  scenes table, and writes `.harness/redesign/shots/<iteration>/<scene>-<theme>-<w>x<h>.png`.
  It must produce a correct image for every scene: list, table, grid, waterfall, layout,
  each inspector tab, filters open, command palette open, Find similar open, Pairing Studio
  (Magazine), expanded family, selected row, empty collection, compact density. The first
  iteration verifies every image by eye and fixes the scenes table or the spec (adding an
  env hook to `screens.spec.ts` is fine; it is e2e code, not `api/`). If the spec's fixed
  viewports (1280×800, 1000×700) are kept, add 1440×900 via an `FM_SCREENS_VIEWPORTS` env.

## The iteration

### 1. Look
Run the screenshot script. Open and actually look at every image. Write a 3–5 sentence
critique in your own words: what a designer would notice first, what is worst, what is
good. This critique goes into the log entry.

### 2. Choose one bold change
Pick exactly one change. Priority:
- The lowest-numbered unfinished, unblocked item in the `DESIGN.md` backlog.
- Or something not on the list, if it is clearly higher impact — then say why in the log.

A bold change is one a designer would notice in the first three seconds or feel in daily
use. "Adjust padding" is not bold. "Replace the filter wall with a filters drawer" is.
Prefer finishing one area completely over touching three areas lightly. The foundation
items (1–3) are prerequisites: do not attempt visual work in other areas until item 1 has
landed.

### 3. Implement
- Vue, CSS and TypeScript in `apps/desktop/src/` — views, components, stores. Nothing in
  `src/api/`, nothing in Rust, nothing in the mock (see the boundary above).
- Use design tokens and the `ui/` components. If a component you need does not exist,
  build it in `src/components/ui/` first, then use it. Never ship a native control.
- Both themes, both densities. Keyboard reachable. Virtual-list performance intact.
- No new dependencies without a one-line justification in the log.
- Keep the diff reviewable: aim under ~800 changed lines, except for foundation iterations.

### 4. Verify
`pnpm --filter @fm/desktop typecheck`, `pnpm lint:js`, `pnpm --filter @fm/desktop test`, then
`pnpm test:e2e` — all green; update a Playwright test only where the UI it checks changed on
purpose, never delete or skip one. Re-run the screenshot script. Open before and after side
by side and judge honestly against the principles in `DESIGN.md`. If it is not better,
fix it or revert it — never ship something worse to satisfy "ship every time". If you
revert, log why; that is still a shipped iteration.

### 5. Ship
- Commit: `iter(N): <short title>` where N is this iteration's number.
- Append an entry to `dev-docs/redesign/ITERATIONS.md` using the template there: number, date, title,
  critique from step 1, why this change, what changed (components/files), backlog item(s)
  ticked, before/after screenshot paths, follow-ups you noticed but did not do, any new
  dependency and why.
- If you discovered a backlog item that should exist, add it to the follow-ups; the review
  pass moves it into `dev-docs/redesign/DESIGN.md`.

## Rules
- One change per iteration. Resist scope creep; write it down as a follow-up instead.
- Do not reformat or restyle code outside the change.
- Do not change `FmClient`, `mock.ts`, generated types or Tauri commands as part of UI
  work. Blocked means blocked; log it and move on.
- When unsure between two directions, pick the one closer to the ranked principles in
  `DESIGN.md` and log the alternative.
