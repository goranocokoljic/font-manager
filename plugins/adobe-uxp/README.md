# Font Manager for InDesign and Photoshop (UXP)

A UXP plugin for InDesign 18.5+ and Photoshop 24.1+. It has no font logic:
it tells the running Font Manager app which fonts a document uses, and the
app finds and activates them (E29.2).

- **Open.** When a document opens (InDesign `afterOpen`, Photoshop `open`),
  the plugin sends its fonts to `fonts_needed`: InDesign's `document.fonts`
  (PostScript name, family, style), Photoshop's text layer fonts. Fonts the
  host still reports missing later (InDesign `afterActivate`, Photoshop
  `select`) are sent again.
- **Close.** When a document closes, the plugin calls `document_closed`. When
  the app offers fonts only that document used, it asks "Deactivate N fonts
  used only by <document>?"; Deactivate calls `deactivate` with those faces,
  Keep them leaves them active.
- **Missing fonts panel (E30.4).** The panel shows "N fonts missing" for the
  active document and lists each font the host reports missing as "In your
  library" (with an Activate button), "Conflicts with an installed font" or
  "Not in your library" (`fonts_needed` with `activate: false`, then
  `conflicts`). Activate all, or a row's Activate, calls `fonts_needed` to
  activate them; a conflict first asks Keep installed or Replace (Replace
  sends `onConflict: "replace"`). It refreshes when the active document
  changes or closes and says "No document is open." when there is none. The
  view is `MissingFontsPanel` in `plugins/shared/`; its texts are in
  `plugins/shared/src/panel-strings.ts`.
- **App not running.** When the app does not answer or refuses the token,
  the plugin shows "Font Manager is not running" once and does nothing else.

It reaches the app through the local API (ADR 0015): `endpoint.json` in the
app's data folder gives the address (always `http://127.0.0.1:<port>`) and
the token. The manifest allows the network to `http://127.0.0.1` only, and
full file access to read that file. The client and the document logic are in
`plugins/shared/`; the types are the generated `apps/desktop/src/api/types.ts`.

## Build and load

```
pnpm --filter @fm/adobe-uxp build     # dist/: manifest.json, index.html, index.js
```

Load `plugins/adobe-uxp/dist/manifest.json` with the Adobe UXP Developer
Tool (Add Plugin, then Load) into InDesign or Photoshop. Font Manager must be
running with its local API on (Settings → Local API).

## Tests

`pnpm test` runs them (Vitest): the host adapters against mocked `indesign`
and `photoshop` modules and a fake endpoint, and `manifest.json` against
`manifest.schema.json`.
