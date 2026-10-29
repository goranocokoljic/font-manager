# 0018 — Adobe hosts: UXP for InDesign and Photoshop, CEP for Illustrator

Status: Accepted (2026-10-29, E30.0)

## Context

E29 and E30 make the design apps tell Font Manager which fonts a document
needs, so the app activates them (E29.2) and offers to deactivate them when
the document closes. The plugins were written against Adobe's documentation
and tested against mocks. E30.0 is the spike that runs them in the real
apps and records what the documentation did not say. Verified on Windows 11
with Photoshop 2025 (26.4.1) and Illustrator 2025 (29.7.1); InDesign was not
at hand, so its half of the UXP plugin stays verified by the mocks only
(see Consequences).

## Decision

**One plugin runtime per app.**

| App | Runtime | Why |
|-----|---------|-----|
| InDesign 18.5+ | UXP (`plugins/adobe-uxp/`) | UXP `indesign` module has the document events and `document.fonts`. |
| Photoshop 24.1+ | UXP (`plugins/adobe-uxp/`) | UXP `photoshop` module has the `open`/`select`/`close` notifications and the text layers' fonts. |
| Illustrator 27+ | CEP (`plugins/adobe-cep-illustrator/`) | Illustrator has UXP inside (Adobe's own `com.adobe.ccx.start` names host `AI`), but no UXP API a third party can ship against. CEP is what it loads. |

**Events per app, as they really fire.**

| App | Document opened | Fonts still missing later | Document closed |
|-----|-----------------|---------------------------|-----------------|
| InDesign (UXP) | `afterOpen` (twice: document, then its layout window; the second is ignored) | `afterActivate` | `beforeClose` |
| Photoshop (UXP) | `action.addNotificationListener` `open` | `select` | `close` |
| Illustrator (CEP) | none: the extension compares the open documents after `documentAfterActivate`/`documentAfterDeactivate` and every 2 s | the same comparison | the same comparison |

Photoshop's `open` fires with the document's text layers readable, so the
fonts go to `fonts_needed` at once and the app activates them before the
user looks for them: no plugin message is shown then (by design, E29.2); the
panel lists only the fonts the app could not match.

**What a missing font looks like to a script.** InDesign names it
(`document.fonts`, a `status` other than installed). Photoshop names it (`characterStyle.font`
is the PostScript name whether installed or not; "missing" is "not in
`app.fonts`"). Illustrator hides it: a text run's `textFont` is an object
whose every property throws UKFG (1430996551, "unknown font"). The only place
Illustrator still names such a font is the saved document's XMP metadata
(`xmpTPg:Fonts`: `stFnt:fontName`, `fontFamily`, `fontFace`), which it
writes on every save. The CEP extension reads the runs it can and appends the
XMP list (`fmXmpFonts` in `jsx/host.jsx`); an unsaved Illustrator document
with a missing font reports nothing for that font, and this is accepted.

**Transports.**

| Runtime | Transport | Facts |
|---------|-----------|-------|
| UXP | local API (ADR 0015) over `fetch`, to `http://localhost:<port>` | A UXP `fetch` carries no `Origin`, so the endpoint takes it. The manifest names `http://localhost` and nothing else; the plugin rewrites the `127.0.0.1` address from `endpoint.json` to `localhost` (`uxpUrl`), which the endpoint accepts as its Host. |
| CEP | IPC pipe or socket (ADR 0016) first; local API second | A CEP page is `file://`, so its `fetch` carries an `Origin` header and the endpoint refuses it (403) by design; the pipe needs no token and is the transport that works. The HTTP fallback stays for a host without Node, where it will not work either; it is kept only so the client code is one. |

**Why `localhost`, from Photoshop's own runtime** (the permissions module
in `dynamic-torqnative.dll`, Photoshop 26.4). Before any matching, every
`network.domains` entry goes through `parse-domain` with `localhost` as the
one custom top-level domain, and an entry it returns `null` for is dropped
from the list. `127.0.0.1` has no top-level domain, so every
`http://127.0.0.1…` entry, with or without a port, is dropped: "Permission
denied to the url … Manifest entry not found" even when the entry is
there. `localhost` survives. The matcher then compares scheme (when the
entry has one), host (with `*.` and `.*` wildcards) and port, and **an
entry without a port matches any port**. So `http://localhost` alone
allows `http://localhost:<port>` for whatever port the OS gave the app,
and the port stays as ADR 0015 chose it. "All" network access was
refused: it declares a plugin that talks to the whole internet, which
Adobe's review flags and rule 7 forbids in spirit. A fixed port list was
tried first and reverted: it bought nothing once the rule was known.

**Manifests.** Photoshop 2025 refuses a third-party manifest whose `host` is
an array: "Expected the host attribute to be an object for the 3P Plugin".
Adobe's documentation allows the array, and Adobe's own plugins use it; third
parties get one host per manifest. The source `plugins/adobe-uxp/manifest.json`
keeps both hosts (its schema and tests check it), and the build emits one
folder per host, `dist/photoshop/` and `dist/indesign/`, each with `host` as
one object and its own id, `org.fontmanager.photoshop` and
`org.fontmanager.indesign` (`manifests.ts`). Both share `index.html` and
`index.js`; the UXP `uxp.host.name` picks the adapter at run time as before.

**Loading a plugin for development.** Adobe's UXP Developer Tool and both
`uxp-devtools-cli` packages connect to Photoshop 26.4 but it rejects every
`plugin load` ("Plugin rejected … due to invalid object", Adobe's own sample
included); the `.ccx` route needs the Creative Cloud desktop app (UPIA). The
way that works is the folder Photoshop scans when developer mode is on:

1. As an administrator, write `{"developer": true}` to
   `C:\Program Files\Common Files\Adobe\UXP\Developer\settings.json`
   (macOS: `/Library/Application Support/Adobe/UXP/Developer/settings.json`).
2. Copy `dist/photoshop/` to
   `%APPDATA%\Adobe\UXP\Plugins\External\org.fontmanager.photoshop_0.1.0\`
   (macOS: `~/Library/Application Support/Adobe/UXP/Plugins/External/`),
   folder name `<id>_<version>`.
3. Start Photoshop. Plugins → Font Manager. The log that names the scanned
   folders and the manifest errors is
   `%APPDATA%\Adobe\Adobe Photoshop 2025\Logs\UXPLogs_*.log`.

The Illustrator extension loads from `%APPDATA%\Adobe\CEP\extensions\` with
`PlayerDebugMode` 1 (CSXS 11 and 12 for Illustrator 2025), as its README
says; a `.debug` file next to its manifest opens Chrome DevTools on the page.

**UI.** UXP draws a `<dialog>` without colors, so the plugin's `index.html`
sets them (dark on dark otherwise). The panel's look is a design task,
parked, not part of this spike.

## Consequences

- E30.x stories build on real behaviour: the events table above is the
  contract the host adapters implement, and `fmXmpFonts` is the only source
  of a missing Illustrator font.
- A UXP release ships two plugins (Photoshop, InDesign) from one source; a
  marketplace listing per app.
- The endpoint must keep accepting `localhost:<port>` as its Host (ADR
  0015 does); the UXP plugin has no other way in.
- InDesign is verified only by the mocks until someone with InDesign runs
  `dist/indesign/` the same way; that check is part of the InDesign plugin
  story (#358), with the sideload folder above.
- Illustrator stays on CEP until a released Illustrator has a third-party
  UXP API with document events, text fonts and a dialog (the CEP README
  says what "ready" means); the UKFG/XMP workaround goes away with it.
- The Adobe devtools CLI is not a dependency and no script depends on it.
