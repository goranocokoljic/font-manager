# data/pairings

Curated font pairings and the small pairing model trained on them (E33.2), as JSON,
community-editable. `suggest_pairings` blends the model's score into the rules' ranking;
the model ships inside the binary, so nothing is downloaded.

## Files

- `pairings.json` — pairs of family names that go well together, each with a short
  source note. Follows `pairings.schema.json`.
- `model.json` — the logistic model trained from `pairings.json` and `fixtures/fonts/`.
  Generated: do not edit by hand. Follows `model.schema.json`.

## Format

```json
{
  "$schema": "./pairings.schema.json",
  "pairings": [
    { "families": ["Libre Baskerville", "Libre Franklin"], "source": "Impallari Type's Libre families" }
  ]
}
```

- `families` — two different family names, as the fonts state them. No pair twice, in
  either order.
- `source` — at most 120 characters: the designers' own companion families, or
  "editor's pick:" and why.

## Contributing

A change is a pull request to `pairings.json`. Every family must be in `fixtures/fonts/`,
because the model is trained there. Then retrain and commit both files:

```
cargo run -p fm-core --example train_pairings
cargo nextest run -p fm-core pairing
```

The tests check the schema rules, that every family is in the fixtures, that the
committed model is not stale, and that the model ranks held-out partners higher than the
rules alone.
