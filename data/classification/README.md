# data/classification

Data for the classification heuristics (serif, sans, script, …), with a JSON schema and
a validating test.

## Glyph traits (`glyph-traits.json`)

The distinctive letter shapes that "match by letters" (epic E49) detects and the letter
picker offers: the leg of the R, the storeys of the g and the a, the tail of the Q, and so
on. `glyph-traits.schema.json` describes the file; `crates/fm-core/src/glyph_traits.rs`
embeds it, checks it, and its tests validate it.

Each trait has:

- `id` — the glyph's name and the trait, joined by a dot: `R.leg`, `one.flag`. For an
  ASCII letter the name is the letter; for a figure its English name.
- `script` (ISO 15924), `glyph` and `codepoint` (`U+0052`) — the character it is measured
  on. Only Latin (`Latn`) in the first cut; Cyrillic (Д, Л, б) and Greek traits use the
  same shape with their own `script` and an ASCII glyph name (`Decy.descenders`).
- `kind` — `enum`: the detector picks one value; `scalar`: the detector measures the
  numbers listed in `measures`, and the first value whose `when` thresholds all hold is
  the chip (`roundness ≥ 0.95 → round`). The last value of a scalar trait has an empty
  `when` and catches the rest.
- `name`, and for every value a `label` — friendly text in `en` and `de` (more languages
  may follow) — and a one-line `description`.
- `enabled` (optional, default `true`) — whether the picker and the search offer the
  trait. A trait whose detector misses the accuracy bar is `false`: still detected and
  stored, so turning it on later needs no new pass.
- for every value an `example`: an SVG path drawn as a stroke, without fill, in a 56 × 56
  box (absolute commands only). The picker shows these, so it never depends on the fonts
  the library holds.

The scalar thresholds are a first cut; the detectors are calibrated against the hand
labels in `fixtures/glyph-traits-labels.json`, which label every trait for at least 50
families of the fixture corpus, each value at least five times. The detectors live in
`crates/fm-core/src/glyphs/traits.rs` (E49.4). A trait ships enabled only when its
detector is right on at least 90 % of the labelled fonts it answers with confidence ≥ 0.7
and answers at least half of them that confidently; the test
`detector_accuracy_meets_the_bar_or_the_trait_is_disabled` prints the confusion per trait
and fails when `enabled` and the bar disagree. Run
`cargo nextest run -p fm-core glyph_traits glyphs::traits` after a change.
