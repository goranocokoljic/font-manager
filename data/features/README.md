# data/features

The feature catalog, as JSON, community-editable: every registered
[OpenType feature tag](https://learn.microsoft.com/en-us/typography/opentype/spec/featurelist)
with a friendly name, a group and a sample text. `fm-core` embeds it and uses it for
`FeatureInfo.name`, `group` and `sample`.

## Format

```json
{
  "$schema": "./features.schema.json",
  "features": [
    { "tag": "smcp", "name": "Small capitals", "group": "capitals", "sample": "Small Capitals" }
  ]
}
```

- `tag` — the four-character tag; one entry per tag, and the file is sorted by tag.
- `name` — the friendly name, in sentence case. A font's own name for `ssXX` or `cvXX`
  replaces it.
- `group` — `ligatures`, `capitals`, `figures`, `fractions`, `positioning`, `stylistic`,
  `language` or `automatic`. Use `automatic` for a feature the shaper applies by itself
  (the `AUTOMATIC` list in `crates/fm-core/src/features.rs`). `locl` is the one exception:
  the shaper applies it, but its group is `language`.
- `sample` — a short text that shows what the feature does. Use the script the feature is
  for: Devanagari for `abvs`, Arabic for `init`. The `ssXX` and `cvXX` samples are generic,
  because each font gives these features a different use.

## Contributing

A change is a pull request to `features.json`. `cargo nextest run -p fm-core features`
checks it: the schema rules, the order of the tags, and that no registered tag is missing.
