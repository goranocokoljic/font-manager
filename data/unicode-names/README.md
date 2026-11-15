# data/unicode-names

The Unicode character names, as JSON. The glyph detail in the Characters section shows
the name of the character under the glyph ("LATIN SMALL LETTER G"). `names.json` follows
`names.schema.json`; a test in `crates/fm-core/src/unicode_names.rs` validates it.

## Source

`names.json` is generated from `UnicodeData.txt` of the Unicode Character Database. The
`unicodeVersion` and `source` fields say which one. Today: Unicode 17.0.0,
<https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt>.

- `names`: the Name property of every code point the file lists by itself.
- `ranges`: the ranges the file lists as `<…, First>` / `<…, Last>` whose names are
  derived from the code point: CJK and Tangut ideographs (a prefix and the hex code
  point) and Hangul syllables (the rule of the Unicode Standard, section 3.12).
- Controls (`<control>`), surrogates and private use have no name and are left out.

## Updating to a new Unicode version

1. Download `UnicodeData.txt` of the new version.
2. Run `node scripts/unicode-names.mjs <path to it> <version>`.
3. Run `cargo nextest run -p fm-core unicode_names`.
