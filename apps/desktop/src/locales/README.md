# Message catalogs

Every word the app shows comes from a catalog here: `en.json` is the source,
every other `<tag>.json` translates it. The UI reads them through `t()` in
`src/i18n/index.ts`; the native Help menu and the tray read the `shell` group
(`src-tauri/src/shell_text.rs`).

## Add a locale

1. Copy `en.json` to `<tag>.json`, where `<tag>` is a BCP 47 language tag in
   lower case (`fr`, `pt-br`). Set `meta.name` to the language's own name
   (`Français`); the Language setting lists it so.
2. Translate every value. Keep the keys, and keep every `{placeholder}` as it
   is. A counted message is an object of plural forms:
   `{ "one": "{count} font", "other": "{count} fonts" }`. Give the forms your
   language's plural rules use (`Intl.PluralRules`: `zero`, `one`, `two`,
   `few`, `many`, `other`); `other` is required, and `zero`, when present,
   is used for 0 in every language.
3. Register the catalog in three places:
   - `CATALOGS` in `src/i18n/index.ts` (import the file);
   - `SOURCES` in `src-tauri/src/shell_text.rs` (the menu and tray labels;
     its counted labels pick `one` for 1 and `other` otherwise — extend
     `count` there if your language needs more forms);
   - a value of `Language` in `crates/fm-api/src/settings.rs` whose wire name
     is the tag (`Fr` is `"fr"`; a tag with a region needs
     `#[serde(rename = "pt-br")]`), then `pnpm gen:types`.
4. Run `pnpm test`. `src/i18n/catalog.test.ts` lists every key your catalog
   is missing or has too many, and every plural or placeholder that does not
   match English. It must report none.
5. Switch the app to your language in Settings → Appearance → Language and
   look at the main window, Settings and the command palette (Ctrl+K / ⌘K).

## Add or change a string

Add it to `en.json` and to every other catalog in the same change; the
catalog test fails otherwise. In a `.vue` template write `{{ t('group.key') }}`
— `src/i18n/templates.test.ts` fails on literal text and on literal
`aria-label`, `title` or `placeholder` values. Show an API error with
`errorMessage(error)`: the catalog has one message per `ErrorCode` (English
shows the core's own sentence through `{message}`). Format numbers and dates
with `formatNumber` and `formatDate`, never by hand.

## Right-to-left languages

A catalog for a language written right to left (Arabic, Hebrew, Persian, Urdu …)
mirrors the chrome by itself: `textDirection` in `src/i18n/index.ts` knows the
language, and the app sets `dir="rtl"` on the root. For that to work:

- Write CSS with logical properties (`margin-inline-start`, `inset-inline-end`,
  `text-align: start`), never `left`/`right`. `src/styles/logical.test.ts`
  fails on a physical one; its allow-list names the few that must not mirror.
- An icon that points along the line (`▸`) gets the class `fm-directional`.
- Read Left/Right arrow keys through `inlineKey()`, so "next" follows the screen.
- Text the user chose (samples, notes) gets `dir="auto"`; names and paths go
  in `<bdi>`. Both keep their own direction in either chrome.

To see the mirrored layout before a catalog exists, set the OS (or browser)
language to the pseudo-locale `ar-XB`: English text in right-to-left chrome.
In browser mode: `pnpm test:e2e e2e/rtl.spec.ts`, or screenshots with
`FM_SCREENS_LOCALE=ar-XB`.
