# data/themes

The named themes of the app, one JSON file per theme. The `themeId` setting holds the
`id` of one of them; `listThemes()` returns them, Paper first. The `theme` setting
(system, light or dark) still chooses which token set of the theme applies.

## Format

```json
{
  "$schema": "./theme.schema.json",
  "id": "paper",
  "name": "Paper",
  "light": {
    "accent": "#c56eb2",
    "on-accent": "#141413",
    "bg": "#ffffff",
    "surface": "#f6f6f7",
    "surface-2": "#ffffff",
    "border": "#00000017"
  },
  "dark": { "...": "the same six tokens" }
}
```

- `id` — the file name without `.json`; lowercase letters, digits and `-`.
- `name` — the name the theme picker shows.
- `light`, `dark` — one color per `--fm-*` token of the same name in
  `apps/desktop/src/styles/theme.css`: `#rrggbb`, lowercase. `border` is a hairline and may
  have alpha (`#rrggbbaa`).

Paper is the app's default. Its colors are the ones in `theme.css`.

## Rules

- `on-accent` on `accent` is at least 4.5:1, because words go on the accent.
- `accent` on `bg`, `surface` and `surface-2` is at least 3:1, because it is the focus ring.
- The other colors of `theme.css` (text, control outlines, collection colors) stay legible on
  the three surfaces and on the accent tint of a selected row.

## Contributing

A new theme is a new file here, plus a line in `FILES` in `crates/fm-core/src/themes.rs`, in
`MOCK_THEME_FILES` in `crates/fm-api/src/mock.rs` and in `MOCK_THEMES` in
`apps/desktop/src/api/mock.ts`.

`cargo nextest run -p fm-core themes` checks the files against the schema and the first two
rules. `pnpm test contrast` checks all the rules against `theme.css`.
