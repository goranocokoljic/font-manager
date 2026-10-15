# Font Manager — GitHub backlog seed

`issues.json` holds the whole backlog from the Task Breakdown (36 epics, 171 stories);
`create_issues.py` pushes it to a GitHub repository with real sub-issues and real
"blocked by" dependencies.

## Prerequisites

- Python 3.9+
- GitHub CLI 2.40+ (`gh --version`), signed in: `gh auth login`
- The token needs **Issues: write**; add **Projects: write** for the board:
  `gh auth refresh -s project`
- An empty repository, e.g. `gh repo create font-manager --public --clone`

## Run

```bash
python create_issues.py --repo OWNER/font-manager --dry-run   # prints the plan, touches nothing
python create_issues.py --repo OWNER/font-manager             # labels, milestones, issues, sub-issues, blockers
python create_issues.py --repo OWNER/font-manager --project   # … plus the Projects board with every issue added
```

Expect about 10–15 minutes for the full run: ~207 issue creations and ~290 relationship (171 sub-issue + 120 blocked-by)
calls, spaced to stay under GitHub's secondary rate limit.

## What it does, and why it is safe to re-run

| Step | Mechanism | Idempotency |
| --- | --- | --- |
| Labels | `gh label create --force` | update-or-create |
| Milestones | `POST /milestones` | matched by title |
| Epic and story issues | `POST /issues` | matched by the `E3.8 ` prefix in the title |
| Epic → story | `POST /issues/{epic}/sub_issues` with the story's **database id** | skipped if already a sub-issue |
| Depends on → blocked by | `POST /issues/{n}/dependencies/blocked_by` with the blocker's **database id** | skipped if already present |
| Board | `gh project create` / `item-add` | matched by title; items by URL |

Sub-issues and dependencies use the issue's numeric `id` (database id), not its `number` —
that is the detail that turns "linked in the epic" into a true parent/child.

After a run, `issues.map.json` maps every ID to its issue number, id and URL; the harness
can read it, or parse the `<!-- fm-meta … -->` block each issue body ends with:

```
<!-- fm-meta
id: E3.8
epic: E3
phase: 1
size: M
track: core
human: false
depends: [E3.7, E3.4]
-->
```

## Labels the harness uses

- `harness:ready` — seeded on stories with no dependencies and criteria written (E3.6 today).
  The harness itself should set it when every blocker is closed.
- `harness:in-progress`, `harness:blocked` — set by the harness.
- `human` — spikes, ADRs, decisions, interactive setup. The harness never picks these.
- `criteria:tbd` — phases 2–4; acceptance criteria get written at the end of the previous phase.

## Board columns

GitHub creates the Status field with Todo / In Progress / Done. Rename and extend it in the
project settings to **Backlog / Ready / In progress / In review / Done**; the script cannot
edit single-select options through the CLI yet.

## Editing the backlog later

Change `issues.json` and re-run. New stories are created; existing ones are left alone
(the script does not rewrite bodies, so edits made on GitHub are never overwritten).
To change an existing issue's text, edit it on GitHub.
