# data/unicode-blocks

The Unicode blocks, as JSON. The glyph map in the inspector groups a face's characters
by these blocks and offers them in its block picker. `blocks.json` follows
`blocks.schema.json`; a test in `crates/fm-core/src/blocks.rs` validates it.

## Source

`blocks.json` is a copy of `Blocks.txt` from the Unicode Character Database. The
`unicodeVersion` and `source` fields say which one. Today: Unicode 17.0.0,
<https://www.unicode.org/Public/17.0.0/ucd/Blocks.txt>.

## Updating to a new Unicode version

1. Download `Blocks.txt` of the new version.
2. Rewrite `blocks`: one `{ "first", "last", "name" }` per line of the file, hex in
   upper case, in file order.
3. Set `unicodeVersion` and `source` to the new version and URL.
4. Run `cargo nextest run -p fm-core blocks`.

Character names come from no file here: the glyph map shows the font's glyph names.
