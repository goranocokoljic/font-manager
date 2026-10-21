# data/unicode-blocks

The Unicode blocks, as JSON. The glyph map in the inspector groups a face's characters
by these blocks and offers them in its block picker. The coverage list in the Glyphs tab
counts a face's characters per block against the block's `assigned` code points
("Greek and Coptic: 120/135"). `blocks.json` follows
`blocks.schema.json`; a test in `crates/fm-core/src/blocks.rs` validates it.

## Source

`blocks.json` is a copy of `Blocks.txt` from the Unicode Character Database. The
`unicodeVersion` and `source` fields say which one. Today: Unicode 17.0.0,
<https://www.unicode.org/Public/17.0.0/ucd/Blocks.txt>.

`assigned` comes from `extracted/DerivedGeneralCategory.txt` of the same version: the
code points of the block whose General_Category is not `Cn`. Surrogates and private use
count as assigned, as Unicode defines it.

## Updating to a new Unicode version

1. Download `Blocks.txt` of the new version.
2. Rewrite `blocks`: one `{ "first", "last", "name" }` per line of the file, hex in
   upper case, in file order.
3. Set `unicodeVersion` and `source` to the new version and URL.
4. Download `extracted/DerivedGeneralCategory.txt` of the new version and run
   `node scripts/unicode-blocks-assigned.mjs <path to it>` to write `assigned`.
5. Run `cargo nextest run -p fm-core blocks`.

Character names come from no file here: the glyph map shows the font's glyph names.
