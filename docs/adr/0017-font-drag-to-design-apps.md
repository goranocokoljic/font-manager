# 0017 — Dragging a font out of the window to a design app

Status: Accepted (2026-10-28, E34.1)

## Context

E34 "Font Switch" lets the user drag a font from the list, grid or table onto
selected text in a design app. The row drag already exists for collections
and tags; it is an HTML drag inside the webview (`application/x-fm-fonts`).
To reach another app, the same drag must carry data that app can read, and
the face must be usable by it, so the drag must work the same on Windows
(WebView2), macOS (WKWebView) and Linux (WebKitGTK) (rule 5).

There is no OS drag API in the frontend and none should be added only for
this: the drag source is the webview, which converts the drag's
`DataTransfer` into the OS drag data when the pointer leaves the window.

## Decision

**What the drag carries.** `fm-core` builds the payload
(`LibraryService::font_switch_payload(FaceId)` → `FontSwitchPayload
{ postscriptName, family, style, fileUrl }`, from the index alone; the file
URL is RFC 8089 with percent-encoded UTF-8, built from the path's bytes
without `cfg`). The frontend reads it when the pointer goes down on a row
(a drag's data must be set synchronously in `dragstart`) and the drag sets:

| `DataTransfer` type | Value |
|---------------------|-------|
| `application/x-fm-fonts` | the dragged rows (unchanged; collections and tags read it) |
| `application/x-fm-font-switch` | the payload as JSON |
| `text/plain` | the PostScript name |
| `text/uri-list` | the font file's `file://` URL |

A family row gives the payload of its first shown face.

**What each webview carries out of the window.**

| Type | Windows (WebView2, Chromium) | macOS (WKWebView) | Linux (WebKitGTK) |
|------|------------------------------|-------------------|-------------------|
| `text/plain` | `CF_UNICODETEXT` | `public.utf8-plain-text` | `text/plain;charset=utf-8`, `UTF8_STRING` |
| `text/uri-list` | `UniformResourceLocatorW` (a URL, not a file drop) | `public.url` | `text/uri-list` |
| `application/x-fm-font-switch` | only inside Chromium's private custom-data format | only inside WebKit's private custom-data type | only inside WebKitGTK's private custom-data target |

No webview exports a custom MIME type under its own name, so a design app
sees the same two types on all three platforms: the PostScript name as
text and the file as a URL. Those are what Font Switch applies (by name, the
way design apps pick fonts). The JSON type stays in the drag so that a drop
back into a webview (the app itself, a plugin panel such as the UXP or
Sketch plugins, which are webviews too) reads the whole payload. Nothing
needs OS code, so `fm-platform` gets none: the gap is the same on every
platform and is documented here instead.

**Activation.** A design app can only apply a face the OS knows. When the
drag leaves the window (`dragleave` on the document with no element entered
and the pointer on or past an edge), an inactive face is session-activated
through the existing `ActivationService::activate` call with the user's
conflict setting. Activation registers from the file's current path
(ADR 0008); nothing is deactivated when the drag ends, so the dropped text
keeps its font. A failed or skipped activation shows a toast.

**Keyboard.** The Developer tab of the inspector has "Copy for Font
Switch", which puts the payload JSON on the clipboard for the selected face.

**Tauri's drop handler.** `dragDropEnabled` only concerns drops *into* the
window (files dropped from the OS); a drag that starts in the webview and
leaves it is the webview's own and is not affected.

## Consequences

- A design app that only accepts files as a file drop (`CF_HDROP`,
  `public.file-url` as a file promise) does not get one from this drag:
  `text/uri-list` arrives as a URL. Adding a native file drag would need an
  OS drag source per platform in `fm-platform`; it is left out until a
  target needs it, and would come with its own ADR.
- The webviews' conversions above are engine behaviour, not a Tauri API.
  A webview update that changes them is caught by trying the drag on the
  three platforms before a release, as the other platform stories do.
- The face stays active for the session after a drop. The user deactivates
  it like any other face; rule 3 is kept because the app registered it.
