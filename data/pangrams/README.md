# data/pangrams

Pangrams for the preview's sample text, as JSON, community-editable. The preview's
pangram picker lists every language that has one here.

## Format

```json
{
  "$schema": "./pangram.schema.json",
  "pangrams": [
    {
      "code": "sr-Cyrl",
      "kind": "pangram",
      "text": "Љубазни фењерџија чађавог лица хоће да ми покаже штос",
      "source": "wikipedia"
    }
  ]
}
```

- `code` — BCP 47 tag of a language preset in `data/charsets`; one entry per code.
- `kind` — `pangram`, or `sample` for a sample sentence where the language has no true
  pangram (Chinese, Korean).
- `text` — NFC. Every character except white space and common punctuation must be in the
  preset's `base` or `auxiliary` characters.
- `source` — `wikipedia` ([List of pangrams](https://en.wikipedia.org/wiki/List_of_pangrams))
  or `project`.

## Contributing

A change is a pull request to `pangrams.json`. `cargo nextest run -p fm-core pangrams`
checks it: the schema rules, known codes, no code twice, every letter in its preset, and
a pangram for every script in `data/charsets`.

## Sources

- **Wikipedia** — the pangrams marked `"source": "wikipedia"` are the well-known
  pangrams of each language, as Wikipedia's list of pangrams gives them.
- **Project (`"source": "project"`)** — the Chinese and Korean sample sentences are
  written here only from the characters of the `data/charsets` basics sets, so a font
  that passes the preset can show them.
