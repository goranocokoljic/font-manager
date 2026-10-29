# Font Manager for Illustrator (CEP)

A CEP extension for Illustrator 27.0 (2023) and later. It has no font logic:
it tells the running Font Manager app which fonts a document uses, and the
app finds and activates them (E29.2). It does what the UXP plugin
(`plugins/adobe-uxp/`) does for InDesign and Photoshop.

- **Open.** When a document opens, the extension collects the fonts of every
  text frame through ExtendScript (`jsx/host.jsx`: each text run's
  `textFont`, its PostScript name, family and style) and sends them to
  `fonts_needed`, once per document. Activating the document again sends
  nothing. A font Illustrator does not have cannot be read from its runs:
  their `textFont` is an object whose every property throws UKFG ("unknown
  font"; Illustrator 29.7, E30.0 spike). Those come from the saved
  document's XMP metadata (`xmpTPg:Fonts`: name, family, face), which
  Illustrator writes on every save. An unsaved document with a missing font
  therefore reports nothing for it.
- **Close.** When a document closes, it calls `document_closed`. When the app
  offers fonts only that document used, it asks "Deactivate N fonts used only
  by <document>?"; Deactivate calls `deactivate` with those faces, Keep them
  leaves them active.
- **App not running.** It says "Font Manager is not running" once and does
  nothing else.

CEP gives Illustrator no open or close event. The extension compares the
open documents with the ones it knows after each `documentAfterActivate` and
`documentAfterDeactivate` and every 2 s: a new one is opened, a gone one is
closed. A document is known by its file path, or by its name while it is
unsaved, so Save As is a close and an open.

It is a hidden extension (no panel): it starts with Illustrator, and its
message and question are ExtendScript dialogs.

**Font Switch (E34).** With no panel there is nothing to drop a font on, so
Illustrator gets no Font Switch from this extension yet. Its adapter is
`illustratorAdapter` in `plugins/font-switch/` (`@fm/font-switch`), which a
panel imports (never copies) with this extension's `evalScript`; the
ExtendScript call helpers both use are `@fm/plugin-shared`'s.

## Why CEP, not UXP

Illustrator has no UXP plugin API that third parties can ship: there is no
UXP `illustrator` module with document events and text fonts, and the UXP
Developer Tool does not load plugins into Illustrator. CEP is the extension
system Illustrator supports, and its Node.js runtime can open the app's pipe
or socket.

**Switch to UXP** when a released Illustrator (not a beta) has a UXP API
with all of: document open and close events, the text fonts of a document
(PostScript name, family, style), and a dialog. Then add `ILST` to
`plugins/adobe-uxp/`'s manifest with a host adapter next to `indesign.ts`,
and retire this extension once its minimum version is the oldest Illustrator
we support.

## How it reaches the app

Through the app's IPC endpoint (ADR 0016) when the app listens on it: the
named pipe `\\.\pipe\org.fontmanager.app-<user>` on Windows, the `fm.sock`
socket on macOS. No token is needed. The manifest turns Node on
(`--enable-nodejs`, `--mixed-context`) for this. When the pipe or socket
cannot be opened, it tries the local API (ADR 0015) through `endpoint.json`;
that fails by design, because a `file://` page's `fetch` sends an `Origin`
header and the endpoint refuses those (403). So the pipe or socket is the
transport; without it the extension says "Font Manager is not running".
The client and the document logic are in `plugins/shared/`; the types are
the generated `apps/desktop/src/api/types.ts`. ADR 0018 records what the
real apps do.

## Build and load

```
pnpm --filter @fm/adobe-cep-illustrator build    # dist/: CSXS/manifest.xml, index.html, index.js, jsx/host.jsx
```

Copy `dist/` as a folder named `org.fontmanager.illustrator` into the user's
CEP extensions folder:

- Windows: `%APPDATA%\Adobe\CEP\extensions\`
- macOS: `~/Library/Application Support/Adobe/CEP/extensions/`

An unsigned extension loads only with debug mode on (`PlayerDebugMode` set to
`1` for CSXS 11: `HKEY_CURRENT_USER\Software\Adobe\CSXS.11` on Windows,
`defaults write com.adobe.CSXS.11 PlayerDebugMode 1` on macOS; Illustrator
2025 also reads `CSXS.12`, so set both). Restart Illustrator. Verified in
Illustrator 2025 (29.7.1) on Windows (E30.0).

## Tests

`pnpm test` runs them (Vitest): the extension against a mocked
`window.__adobe_cep__` whose `evalScript` runs the real `jsx/host.jsx`
against a fake Illustrator `app`, a fake endpoint, and `CSXS/manifest.xml`
parsed and checked for the host, the minimum version and Node.
