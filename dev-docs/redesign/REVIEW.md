You are running a **review pass** of the Font Manager UI/UX improvement loop — the critic,
not the generator. Font Manager is a Tauri + Vue desktop app. Work autonomously; do not ask
questions — decide, and log the decision.

0. Confirm you are on the `redesign` branch; if not, stop and do nothing else.
1. Read `CLAUDE.md`, then `dev-docs/redesign/DESIGN.md` and `dev-docs/redesign/ITERATIONS.md`
   completely.
2. Run `node scripts/redesign/shots.mjs <review-number>` (one viewport, both themes, ~40
   images) and open all of them — the review is the one pass that looks at everything. For
   the five iterations under review, open only the `list` shot from each one's folder to see
   the trajectory; do not re-open their full sets.
3. For each of the last five iterations, grade 1–5 against the ranked principles in
   `DESIGN.md` and the "Must not break" list. Be harsh: a 5 is something you would put in
   a portfolio. Note specifically:
   - anything that regressed (visual, keyboard, performance, a broken view),
   - anything that drifted from the design-system rules (literal colours, native controls,
     a second accent colour, a new font, inconsistent radii or spacing),
   - duplicated components or dead CSS left behind,
   - two iterations that pulled in opposite directions.
4. Fix what you found. Reverting an iteration is allowed when it made things worse;
   partial reverts are preferred over full ones. Consolidate duplicated components. Delete
   dead styles. Do not add new features in a review pass.
5. Run `pnpm --filter @fm/desktop typecheck`, `pnpm lint:js`, `pnpm --filter @fm/desktop test`
   and the full Playwright suite once, `--reporter=dot` into `.harness/redesign/e2e-review-<N>.log`,
   reading only the failures. Re-run the screenshot script and confirm every view still renders in
   both themes and both densities.
6. Update `dev-docs/redesign/DESIGN.md`: tick nothing there (ticks live in `ITERATIONS.md`), but re-order the
   backlog if impact has changed, add follow-ups from the log that deserve to be backlog
   items, and amend a design-system rule if iterations proved it wrong (say so in the log).
7. Commit: `review(N): <summary>`. Append a review entry (at most 25 lines) to
   `dev-docs/redesign/ITERATIONS.md` with the five grades, what you fixed or reverted, and
   what the next five iterations should focus on. Then **condense**: collapse the five
   reviewed iteration entries to their title line plus a one-sentence summary each, since
   future iterations read only review entries and the last two iteration entries.
