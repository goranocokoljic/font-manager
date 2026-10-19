# Activation

Activation makes a font available to other apps, for example a word processor or a
design tool. Font Manager does not copy or install the file: it tells the operating
system where the file is.

## Two kinds of activation

| Kind | Key | How long it lasts |
|------|-----|-------------------|
| For this session | `A` | Until you quit Font Manager. |
| Permanent | `Shift+A` | Also after a restart, until you deactivate the font. |

Select one or more fonts and press the key. Press it again to deactivate them. If only
some of the selected fonts are active, the key activates the rest. A family activates or
deactivates with all its styles. In the settings, under **Activation**, you choose which
kind the **Activate** button in the inspector uses.

## See what is active

Every row in the list and every card in the grid has a dot:

| Dot | Meaning |
|-----|---------|
| Empty circle | Not active |
| Half filled | Some styles of the family are active |
| Filled | Active |

The status bar shows how many fonts are active.

## Deactivate all

Press `Ctrl+K` and choose **Deactivate all**. All fonts that Font Manager activated stop
being active, for this session and permanent ones.

## Keep only the current set

A project set is a named group of fonts for one job. The sets are under **Project sets**
in the sidebar. Click a set to make it the current set. Click it again to clear it.

Press `Ctrl+K` and choose **Deactivate everything not in the current set**. All fonts that
Font Manager activated stop being active, but the fonts of the current set stay active.
The command is dimmed when no set is current.

## Undo

When you activate or deactivate many fonts at once (several rows, a family,
**Deactivate all**, or **Deactivate everything not in the current set**), a message at the bottom of the window tells you what changed. Click **Undo** in the message to put every font back as it
was, in the same kind of activation. The message goes away after 10 seconds. It stays
while the pointer is on it.

## Rules that keep your system safe

- Font Manager never needs administrator rights.
- It never writes to the system font folders. Activation is for your user account only.
- It only deactivates fonts that it activated itself. Fonts that other apps or your
  system installed stay as they are.
- If the file is moved or deleted, the font stops working. Font Manager shows this in
  the list.

## When two fonts have the same name

Sometimes a font has the same PostScript name as a system font or a font that is
already active. Apps ask for fonts by this name, so they can show only one of the two.
When you activate such a font, Font Manager asks you what to do:

- **Keep existing** — the other font stays. Your font is not activated.
- **Replace for this session** — your font is activated until you quit Font Manager,
  also when you asked for a permanent activation. After that, the other font is back.
- **Cancel** — nothing changes.

In the settings, under **Activation**, you can choose **Replace the other font for this
session without asking**. Then Font Manager does not ask.

Font Manager never deactivates the other font. If the other font is a system font or a
font that another app installed, it stays installed.

### The Conflicts view

Open **Conflicts** in the status bar, or **Show font conflicts** in the command
palette. The view lists every active font that has the same PostScript name as another
font, the other font, and the winner. The winner is the font that Font Manager puts
first: the font you activated with **Replace for this session**, or the font activated
last when Font Manager activated both.

## How it works on each system

| System | Session | Permanent |
|--------|---------|-----------|
| Windows | A per-user font entry, removed when Font Manager quits | A per-user font entry |
| macOS | Registered for this login session | Registered for your user |
| Linux | A link in `~/.local/share/fonts` that Font Manager removes when it quits | A link in `~/.local/share/fonts` |

Apps that are already open can need a few seconds to see a new font. On Linux this
can take up to 30 seconds. Some apps only see new fonts after a restart.

> If Font Manager stops unexpectedly, it removes the session fonts the next time it
> starts.
