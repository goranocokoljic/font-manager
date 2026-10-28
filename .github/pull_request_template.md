<!--
Title: "<story id> <story title>", for example "E3.2 FTS index". Without a story: a short title of what changed.
Read CONTRIBUTING.md before you open the pull request.
-->

Closes #<!-- issue number -->

Story: <!-- story id, for example E3.2, or "none" -->

## What changed

-

## How it was verified

<!--
The commands you ran, test names, bench numbers, screenshots for UI changes.
Map each acceptance criterion of the issue to its evidence.
-->

| # | Criterion | Evidence |
| - | --------- | -------- |
| 1 |           |          |

- [ ] `pnpm lint && cargo nextest run --workspace && pnpm test` pass
- [ ] `pnpm gen:types` leaves `apps/desktop/src/api/types.ts` unchanged
- [ ] I followed the rules in [CONTRIBUTING.md](../CONTRIBUTING.md#rules-that-do-not-bend)
