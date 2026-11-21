# Cleaning up duplicates

A collection that grew over many years often holds the same font more than once:
the same file in two folders, the same font as TTF and as WOFF2, or an old and a new
version of one font. Font Fu finds these copies and helps you keep one of each.

Font Fu never deletes a font file for good. A copy that leaves the library goes to the
trash of your computer, or it is hidden: the file stays where it is, and Font Fu does
not show it. You can undo every cleanup.

## The three levels

Font Fu puts the copies of one font in a **duplicate set**. Each set has one level:

| Level | What it means | In the CLI |
|-------|---------------|------------|
| Identical | The same file, byte for byte, in two or more places. | `identical` |
| Same version | One font with the same version, in files that are not the same. For example the TTF and the WOFF2 of one font. | `same-version` |
| Other version | One font in files with different versions. For example version 1.0 and version 2.1. | `other-version` |

"One font" means one PostScript name. Your computer can activate only one font with a
name at a time, and your documents ask for the name, not for the version.

## Which copy stays

The **keeper policy** chooses the copy to keep. You set it in the app. The default is:
the copy that covers more of your languages, then the newest version, then the format,
then the copy in the library folder, then the path from A to Z.

Some copies always stay:

- the only copy that covers one of your languages,
- a copy in a system font folder,
- a copy that the app activated,
- a file that another set keeps (a font collection file with several fonts).

Font Fu tells you why each copy stays or goes, in one sentence. The app, the CLI and
AI assistants show the same sentence.

## In the app

Open **Duplicates**. You see how many fonts have more than one copy and how much space
a cleanup frees. Work through the sets, or clean up two folders that hold the same
fonts. Then apply the plan. Undo it from the list of sessions.

## On the command line

| Command | What it does |
|---------|--------------|
| `fm duplicates list [--level <level>] [--folder <dir> \| --source <id>]` | List the duplicate sets. Each copy shows its face id, its version and its path. The copy to keep and the copies to remove show the reason in one sentence. Repeat `--level` for more levels. |
| `fm duplicates folders [--min-shared 5]` | List pairs of folders that hold the same fonts, with the space that removing one side frees. |
| `fm duplicates compare <a> <b>` | Show what differs between two copies: languages, glyphs, features, axes and metrics. Give the face ids from `fm duplicates list`. |
| `fm duplicates plan [--out plan.json] [--force]` | Plan a cleanup. Shows each file with what happens to it and why, then a summary. Changes nothing. `--out` saves the plan. |
| `fm duplicates apply <plan.json> [--yes] [--name <name>]` | Apply a saved plan. |
| `fm duplicates sessions` | List the cleanups, newest first, with their ids. |
| `fm duplicates undo <session> [--path <file>]` | Put the files of a cleanup back. `--path` puts back only that file. |

Choose what a plan covers with one of these:

- nothing: the whole library,
- `--folder <dir>`: the sets with a copy in the folder; only the copies in the folder can leave,
- `--source <id>`: the same for one folder of your library,
- `--set <id>`: only this set (repeat it for more),
- `--keep-folder <dir> --remove-folder <dir>`: keep the copies of one folder and remove the
  same files from the other folder. Add `--other-levels` to also remove other copies of
  the same font, through the keeper policy.

Add `--hide` to hide the copies instead of moving them to the trash.

### Apply a plan

```
fm duplicates plan --out plan.json
fm duplicates apply plan.json
```

`fm duplicates apply` shows a summary first: the files that go to the trash, the files
that are hidden, the files that stay for a reason (with the reason), and the space that
is freed. Then it asks `Apply this plan? [y/N]`. Type `y` to apply it. Any other answer
changes nothing.

`--yes` applies the plan without the question. With `--json`, `fm` cannot ask, so add
`--yes`. There is no option that deletes a file for good.

Before each file leaves, Font Fu checks it again. A file that changed since the plan, or
that the app activated in the meantime, stays. The result shows it as `skipped`, and
`fm` then exits with code 4.

### Undo

```
fm duplicates sessions
fm duplicates undo <id>
```

Undo puts the files back from the trash, or shows the hidden copies again. If another
file is at the old path now, that file stays and the undo of that one file fails.

### Output for programs

With `--json`, every command prints the answer of the Font Fu API, unchanged. Read
[The `fm` command line](cli.md) for the exit codes.

## With an AI assistant

An AI assistant can list your duplicate sets, list folder pairs, compare two copies and
plan a cleanup. It cannot apply a cleanup. Its plan tells you where to apply it: in the
app, or with `fm duplicates apply`. Read [AI assistants](mcp.md).
