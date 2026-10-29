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
- **Font Switch (E34).** A font dragged from Font Manager onto the panel is
  applied to the selected text (InDesign) or the selected text layers
  (Photoshop). With no text selected it says "Select some text first"; for a
  font the app does not have, "<name> is not active". The adapters and the
  drop handler are imported from `plugins/font-switch/` (`@fm/font-switch`),
  not copied.

It reaches the app through the local API (ADR 0015): `endpoint.json` in the
app's data folder gives the address (always `http://127.0.0.1:<port>`) and
the token. The plugin calls `http://localhost:<port>` instead
(`src/endpoint-url.ts`): UXP lets a plugin call only the hosts its manifest
names, drops `127.0.0.1` from that list because it has no top-level domain,
and takes `localhost` as the one special name; an entry without a port
matches any port. So the manifest allows `http://localhost` only, and full
file access to read `endpoint.json`. The client and the document logic are
in `plugins/shared/`; the types are the generated
`apps/desktop/src/api/types.ts`. ADR 0018 records what the real apps do.

## Build and load

```
pnpm --filter @fm/adobe-uxp build     # dist/photoshop/ and dist/indesign/: manifest.json, index.html, index.js
```

One folder per host: Photoshop refuses a third-party manifest with more than
one host (`manifests.ts` splits the source `manifest.json`). The ids are
`org.fontmanager.photoshop` and `org.fontmanager.indesign`.

The Adobe UXP Developer Tool and the `uxp-devtools-cli` packages do not load
a plugin into Photoshop 2025 ("Plugin rejected … due to invalid object"), and
a `.ccx` needs the Creative Cloud desktop app. Load it from the folder the
apps scan instead:

1. Turn developer mode on, once, as an administrator:
   `C:\Program Files\Common Files\Adobe\UXP\Developer\settings.json`
   (macOS: `/Library/Application Support/Adobe/UXP/Developer/settings.json`)
   with the content `{"developer": true}`.
2. Copy `dist/photoshop/` to
   `%APPDATA%\Adobe\UXP\Plugins\External\org.fontmanager.photoshop_0.1.0\`
   (macOS: `~/Library/Application Support/Adobe/UXP/Plugins/External/`);
   for InDesign, `dist/indesign/` to `org.fontmanager.indesign_0.1.0`. The
   folder name is `<id>_<version>`.
3. Start the app. The plugin is under Plugins → Font Manager. Font Manager
   must be running with its local API on (Settings → Local API).

When the panel does not appear, the manifest error is in
`%APPDATA%\Adobe\Adobe Photoshop 2025\Logs\UXPLogs_*.log`.

Verified in Photoshop 2025 (26.4.1) on Windows (E30.0). InDesign is checked
by the tests only until someone runs `dist/indesign/` the same way (#358).

## Tests

`pnpm test` runs them (Vitest): the host adapters against mocked `indesign`
and `photoshop` modules and a fake endpoint, and `manifest.json` against
`manifest.schema.json`.
