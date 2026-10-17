# data/charsets

Language → required characters, as JSON, community-editable. Font Manager uses these
presets for the "supports language" filter and the coverage report. Every file is
validated against the JSON schema next to it by a test (epic E3).

## Files

| File              | Languages                                                       |
| ----------------- | --------------------------------------------------------------- |
| `latin.json`      | Latin-script European languages, Turkish, Vietnamese            |
| `cyrillic.json`   | Russian, Ukrainian, Belarusian, Bulgarian, Serbian, Macedonian  |
| `greek.json`      | Greek (monotonic)                                               |
| `arabic.json`     | Arabic, Persian                                                 |
| `hebrew.json`     | Hebrew                                                          |
| `thai.json`       | Thai                                                            |
| `devanagari.json` | Hindi, Marathi, Nepali                                          |
| `cjk.json`        | Chinese (Simplified, Traditional), Japanese, Korean — basics    |
| `charset.schema.json` | the JSON schema every other file follows                    |

## Format

```json
{
  "$schema": "./charset.schema.json",
  "languages": [
    {
      "code": "sr-Cyrl",
      "name": "Serbian (Cyrillic)",
      "script": "Cyrl",
      "otLanguage": "SRB",
      "base": "А Б В …",
      "auxiliary": "…",
      "source": "hyperglot"
    }
  ]
}
```

- `code` — BCP 47 tag (`pl`, `sr-Cyrl`); unique across all files.
- `script` — ISO 15924 code. `otLanguage` — the OpenType language tag, used to find
  `locl` forms.
- `base` — characters a font must have to support the language, separated by one space.
  Each item is one Unicode character, precomposed where Unicode has a precomposed form
  (`ệ`, not `e` + two marks). No item twice.
- `auxiliary` — characters for loanwords, names or older spelling. Shown, never
  required. No overlap with `base`.
- `source` — where the base set comes from: `hyperglot`, `google-fonts` or `project`.

## Contributing

A change is a pull request to one data file. `cargo nextest run -p fm-core charsets`
checks it: the schema rules, no duplicates, every character inside its script's Unicode
blocks, and the required languages present. A new file must also be added to `FILES` in
`crates/fm-core/src/charsets.rs` (the data is built into the app); the test fails until it is.

## Sources and credits

- **[Hyperglot](https://github.com/rosettatype/hyperglot)** by Rosetta Type — the
  Latin, Cyrillic, Greek, Arabic, Hebrew, Thai and Devanagari `base` and `auxiliary`
  sets are written after Hyperglot's base and auxiliary orthography characters.
  Marked `"source": "hyperglot"`.
- **[Google Fonts language data](https://github.com/googlefonts/lang)**
  (`gflanguages`, Apache 2.0) — the reference for language codes, names, scripts
  and exemplar characters. A set taken from it is marked `"source": "google-fonts"`.
- **Project-curated (`"source": "project"`)** — the CJK sets are *basics*, not full
  repertoires: Japanese is all hiragana and katakana plus 58 common kanji; Chinese is the
  100 most frequent characters (simplified, and their traditional forms) plus full-width
  punctuation; Korean is 89 of the most frequent Hangul syllables. A font that passes them
  is a candidate for the language, not a guarantee of full coverage.

Thank you to the Hyperglot and Google Fonts contributors. Characters are facts about
writing systems; the selection here is our own and is MIT OR Apache-2.0 like the rest of
the repository.
