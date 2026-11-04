# data/licenses

What a font's license lets a user do, in plain words, as JSON, community-editable. `fm-core`
embeds `permissions.json` and uses it for `LicenseInfo.permissions`.

This is a summary to help people, not legal advice. Keep each line true to the license text.

## Format

```json
{
  "$schema": "./permissions.schema.json",
  "openLicenses": { "ofl": { "may": ["…"], "mayNot": ["…"] } },
  "declared": { "desktop": { "may": ["…"], "mayNot": ["…"] } },
  "embedding": { "restricted": { "may": [], "mayNot": ["…"] } }
}
```

- `openLicenses` — one rule for each license the app recognizes in a font's name table:
  `ofl`, `apache`, `ufl`, `mit`.
- `declared` — one rule for each license type a user can declare: `desktop`, `web`, `app`,
  `trial`, `personalOnly`. `unknown` has no rule, because it says nothing.
- `embedding` — one rule for each embedding permission of `OS/2.fsType`: `restricted`,
  `previewAndPrint`, `editable`. `installable` has no rule: a file with no `OS/2` table reads
  the same, so it says nothing.
- `may`, `mayNot` — short lines in sentence case with no final period. A rule has at least
  one line, and no line twice.

## Which rule applies

The code decides this in `crates/fm-core/src/license/permissions.rs`, from the same order as
`license::web_use`. No rule applies when nothing is known, and then the app shows nothing.

The lines must agree with `web_use`. A rule that allows the web has a `may` line with the word
"websites". A rule that does not allow the web has no such `may` line. A test checks this.

## Contributing

Run `cargo nextest run -p fm-core license::permissions` after an edit. The tests load the file
with the rules of `permissions.schema.json`.
