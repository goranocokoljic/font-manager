# data/subsets

The named character sets a web font export can keep: `latin`, `latin-ext`, `cyrillic`,
`cyrillic-ext`, `greek`, `greek-ext` and `vietnamese`. A request names one or more of
them (`Subset::Presets`); the export keeps the characters of all of them that the face
has. `presets.json` follows `presets.schema.json`; a test in
`crates/fm-core/src/subset.rs` validates it.

## Source

Each preset is a list of CSS `unicode-range` values, copied from the `@font-face` rules
Google Fonts serves for a family with every split (`source` names the URL). A page that
loads the export's files with these ranges downloads only the files it needs, the same
way it does with Google Fonts.

## Updating

1. Open the `source` URL in a current browser (or `curl` it with a browser user agent).
2. Copy the `unicode-range` of each split into the preset of the same name, one value per
   string, hex in upper case.
3. Run `cargo nextest run -p fm-core subset`.

A new preset needs a lower-case name (`latin-ext`) and at least one range.
