# fixtures/catalogs

Recorded catalog answers, trimmed, for tests that must not reach the internet. The
tests serve them from a local server on 127.0.0.1 (ADR 0014).

| File | Recorded from | Trimmed to |
|------|---------------|------------|
| `google-fonts/metadata-fonts.json` | `GET https://fonts.google.com/metadata/fonts` (2026-10-24) | 5 of 1,950 families, as recorded (Bebas Neue, Lato, Noto Sans JP, Playfair Display, Roboto Flex); the axis registry cut to their axes and to `tag`, `displayName`, `min`, `defaultValue`, `max`, `precision` |
| `fontshare/fonts.json` | `GET https://api.fontshare.com/v2/fonts` (2026-10-24) | 4 of 100 families, as recorded (Kihim, General Sans, Gambetta, Literata); each family cut to `id`, `name`, `version`, `script`, `category`, `inserted_at`, `designers[].name`, `slug`, `license_type`, `axes`, and its styles to `default`, `id`, `file`, `weight`, `is_variable`, `is_italic`; `count` and `count_total` set to 4 |
| `google-fonts/download-list-lato.json` | `GET https://fonts.google.com/download/list?family=Lato` (2026-10-24) | the license text cut to its first lines; the `)]}'` guard kept |

Re-record with `curl` and trim the same way; the tests assert the family count and one
family's fields.
