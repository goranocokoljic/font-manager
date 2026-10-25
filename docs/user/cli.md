# The `fm` command line

`fm` reads the same library as the app. It opens the index in the app
folders, so the app does not have to run.

## Commands

| Command | What it does |
|---------|--------------|
| `fm version` | Show the app version and the API version |
| `fm resolve <font>` | Show which font a name or id stands for |
| `fm scan [folder]` | Add the folder to the library and scan it. Without a folder, scan every folder of the library. Progress goes to stderr. A folder that fails to scan does not stop the others; `fm` then exits with the code of the first failure. |
| `fm list` | List the families |
| `fm list --family <font>` | List the faces of a family |
| `fm search [words] [--supports sr-Cyrl,sr-Latn] [--feature smcp] [--variable]` | Find families. With `--supports`, show the characters each family misses. |
| `fm info <font>` | Show a font: names, styles, axes, features, scripts and license |
| `fm coverage <font> --text "..."` | Check that a font has every character of the text. Exits with code 5 when a character is missing. `--supports <languages>` checks languages instead. |
| `fm similar <font> [--limit 10]` | List the fonts that look most like this one |
| `fm activate <font> [--permanent]` | Activate a font. A family name activates every style. Shows one line per font with what happened. Exits with code 4 when a font was not activated (a conflict with an installed font, a format that cannot be activated, an error). The app must run. |
| `fm activate --collection <name>` / `--set <name>` | Activate every font of a collection or a project set. Give its name or its id. |
| `fm deactivate <font>` (or `--collection`, `--set`) | Deactivate fonts. `fm` deactivates only fonts that the app activated. It shows other fonts as skipped and exits with code 4. The app must run. |
| `fm sets export <set> [--out file.json]` | Write a project set as JSON. Without `--out`, the JSON goes to stdout. |
| `fm sets import <file.json>` | Make a new project set from an exported set. Shows the fonts found in your library and the fonts that are missing. |
| `fm --help` | Show every command and the exit codes |

## Activation needs the app

`fm activate` and `fm deactivate` work only when Font Manager runs. When the
app is closed, they do nothing and exit with code 4. A font that `fm`
activated itself would stop when `fm` stops.

## Name a font

Where a command asks for `<font>`, give one of these:

1. A face id, for example `42`.
2. A PostScript name, for example `Inter-Bold`.
3. A full name, for example `Inter Bold`.
4. A family, with or without its style, for example `Inter` or `Inter Bold`.

`fm` tries them in this order. Upper and lower case do not matter. When a
name fits more than one font, `fm` lists the fonts with their ids and exits
with code 2. Give the id instead.

## Output for programs

Add `--json` to any command. Then `fm` prints one JSON value and nothing
else: the answer on stdout, or the error on stderr. Without `--json`, `fm`
prints text and tables.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Unexpected error (internal, I/O, cancelled) |
| 2 | Usage error or invalid request, for example a name that fits more than one font |
| 3 | Not found |
| 4 | Refused (permission denied, conflict, busy, unsupported) |
| 5 | A check found problems |

## A separate library for tests

Set `FM_HOME` to an absolute folder. `fm` then keeps its index, settings,
cache and logs in that folder, not in your app folders.
