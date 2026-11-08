# 0022 — Dragging font files out of the window

Status: Accepted (2026-11-08, E44.4)

## Context

E44.4 lets the user drag a selection's font files out of the window into
another app: Explorer, Finder, Nautilus, a mail or chat app, a design app
that takes font files. ADR 0017 left this out ("adding a native file drag
would need an OS drag source per platform in `fm-platform`") because the row
drag is an HTML drag in the webview, and no webview turns an HTML drag into a
file drop: `text/uri-list` leaves the window as a URL on all three.

Two facts shape the decision:

- **One drag at a time.** Once an HTML drag runs, the webview owns the OS drag
  session (Chromium calls `DoDragDrop`, WebKit begins an `NSDraggingSession` or
  a GTK drag). When that drag leaves the window, nothing can add files to it
  or start a second drag.
- **The row drag is Font Switch.** A design app given a file drop opens the
  font file instead of applying the font to the selected text. Making the row
  drag a file drag would break ADR 0017.

## Decision

**The gesture.** The selection bar has a **Files** grip. The user holds it
and moves the pointer; inside the window nothing is dragged (pointer capture,
no HTML drag). When the pointer reaches the window edge, the app starts a
native file drag of the selection's files. Released inside the window, the
grip does nothing. A click says how the grip works. The row drag is
unchanged (ADR 0017).

**What it carries.** `LibraryService::drag_files(DragFilesRequest { faces,
families })` → `DragFiles { paths, missing }` (fm-core, from the index): each
face's file, where the user keeps it, each path once (a collection file holds
several faces), in selection order. A file no longer on disk is left out and
counted; the UI says how many. Nothing is copied: the target copies (rule 2).

**The platform layer.** `fm-platform::drag::FileDrag` (`support()`,
`start(files)`), one implementation per OS. `start` runs on the UI thread
while the mouse button is down; the Tauri command `start_file_drag` asks
fm-api for the files, then runs the drag with `run_on_main_thread`. No window
handle crosses the boundary: each OS takes the app's own window.

| | Windows | macOS | Linux |
|---|---|---|---|
| Data | the shell's data object for the files (`SHCreateShellItemArrayFromIDLists` → `BHID_DataObject`): `CF_HDROP` and the shell ID list | one `NSDraggingItem` per file, pasteboard writer the file `NSURL` (`public.file-url`), the Finder icon as its image | `text/uri-list` (RFC 2483 file URLs, built from the path's bytes) |
| Drag | `SHDoDragDrop` from the foreground window; modal until the drop | `beginDraggingSessionWithItems:event:source:` from the key window's content view | `gtk_drag_begin_with_coordinates` from the active toplevel; XDND on X11, the data-device protocol on Wayland (no portal needed) |
| Copy, not move | allowed effect `DROPEFFECT_COPY` only, and "Preferred DropEffect" = copy (Explorer would move on the same drive) | source mask `NSDragOperationCopy` only | `GDK_ACTION_COPY` only |
| Returns | after the drop or cancel | once the drag began | once the drag began |

**Parity.** The drag ships on all three platforms (rule 5). Where it cannot
work, `AppInfo.fileDrag` says `available: false` with a reason, and the grip
is disabled (`aria-disabled`) with the reason as its tooltip; there is no
`cfg` outside fm-platform. Today that is Linux without an X11 or Wayland
display, and a host without the app window (browser mode, the CLI).

**Tests.** A real mouse drag cannot be driven on a CI runner. Each OS has a
cross-process test (`crates/fm-platform/tests/file_drag.rs`): the test hands
the OS exactly the data the drag hands it (`drag::probe`): the shell data
object on the OLE clipboard, the drag items' writers on a named pasteboard,
the drag's `text/uri-list` answer on a named X selection; a child process of
the test binary reads it back as a drop target does and must get the
original paths, in order, with the files unchanged. The allowed operation is
unit-tested per OS. The gesture is covered by Vitest and Playwright on the
MockApi (`?mockFileDrag=1`).

## Consequences

- The drag needs the mouse button down when `start` runs. The grip holds the
  pointer, so the button is down when the pointer reaches the edge; a very
  quick release at the edge can end the drag at once (the drop goes nowhere).
- On Windows the UI thread runs the drag's modal loop until the drop; the
  webview keeps painting (the loop pumps messages), and the command answers
  after the drop.
- `missing` counts files gone from disk and, on Unix, names that are not
  UTF-8 (fm-api carries paths as strings); they are left out of the drag.
- The webviews' own conversions in ADR 0017 are unchanged.
