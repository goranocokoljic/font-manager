# 0025 — The text fields' own menu and a native clipboard read

Status: Accepted (2026-11-17, E54.2)

## Context

E54.1 cancels every `contextmenu` event, so the webview never shows its own
menu (Back, Reload, Inspect…). That also took away the text fields' Cut, Copy,
Paste and Select all. E54.2 gives them back as the app's own menu, and each
item must do what its keyboard shortcut does on the three systems.

Cut, Copy and Select all are the webview's own editing commands: a page runs
`document.execCommand('copy' | 'cut')` during a user action (a click, a key),
and selects a field's text itself. Paste is different: a page may not read the
clipboard on its own.

- WebView2 asks for the `clipboard-read` permission, and refuses it unless
  the window was built with `enable_clipboard_access`.
- WebKitGTK refuses `navigator.clipboard.readText()` and
  `execCommand('paste')` unless the same opt-in set
  `javascript-can-access-clipboard`.
- WKWebView has no such opt-in in wry: `readText()` shows WebKit's own
  "Paste" bubble, which the user must click, whenever the text came from
  another app. That is not what Cmd+V does.

The main window comes from `tauri.conf.json`, which has no clipboard option.

## Decision

**The menu.** One `TextFieldMenu` in `App.vue` listens for `contextmenu` on
the window. On a text `<input>` (text, search, url, tel, email, password,
number) or a `<textarea>` that is not disabled, it opens a `UiPopover` with
exactly Cut, Copy, Paste and Select all, at the pointer (under the field for
the Menu key and Shift+F10). Cut and Copy are disabled without a selection;
Cut and Paste in a read-only field. `UiPopover` gained an `at` point for this.

**The focus stays in the field.** The other menus move the focus into the
menu. This one does not: the arrows, Home, End, Enter and Esc reach it through
a window `keydown` listener in the capture phase while it is open, and the
field names the highlighted item with `aria-activedescendant`. Moving the
focus would end what the field was doing (a layout block's editor and the
notes save and close on blur; a font picker closes) and would lose the
selection Copy needs. Any other key closes the menu and does its own job, so
Ctrl+C and the other shortcuts are untouched.

**Paste reads the clipboard natively.** `fm-platform::clipboard::read_text()`
answers the clipboard's text (empty when it holds none), and the Tauri
command `read_clipboard_text` runs it on the UI thread. The UI inserts the text
with `execCommand('insertText')`, which is one undo step and fires `input` as
typing does; `setRangeText` and an `input` event where that command is
missing. No crate is added: each OS uses bindings the app already has.

| | Windows | macOS | Linux |
|---|---|---|---|
| Read | `OpenClipboard` → `GetClipboardData(CF_UNICODETEXT)` (`windows`) | general pasteboard `stringForType(NSPasteboardTypeString)` (`objc2-app-kit`) | `gtk::Clipboard` `CLIPBOARD` selection `wait_for_text` (`gtk`; X11 and Wayland) |
| Thread | any; the UI thread | the UI thread | the GTK (UI) thread |

Cut and Copy stay on the webview's commands (with `navigator.clipboard`'s
`writeText` as a fallback): writing the clipboard needs no permission on any
of the three webviews during a user action, so no native write is needed.

**Parity.** The menu and every item work on all three systems (rule 5). The
browser mode (Playwright on the MockApi) reads the browser's clipboard.

**Tests.** `crates/fm-platform/tests/clipboard.rs`: one process puts text on
the system clipboard as another app's Copy does (`clipboard::probe`), a child
process reads it with `read_text`, on every CI runner (Linux under
`xvfb-run`). The menu's rules have Vitest tests; the menu has a Playwright
spec (`e2e/text-field-menu.spec.ts`).

## Consequences

- A new text field gets the menu with no work. A field that wants no menu is
  not a text field (a button, a slider).
- The clipboard read is the app's only one; it runs only on the user's Paste.
- Building the main window in Rust with `enable_clipboard_access` would have
  covered Windows and Linux only; macOS would still show the "Paste" bubble.
