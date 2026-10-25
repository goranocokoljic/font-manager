# The `fm` command line

`fm` reads the same library as the app. It opens the index in the app
folders, so the app does not have to run.

## Commands

| Command | What it does |
|---------|--------------|
| `fm version` | Show the app version and the API version |
| `fm resolve <font>` | Show which font a name or id stands for |
| `fm --help` | Show every command and the exit codes |

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
