# Activation

Activation makes a font available to other apps, for example a word processor or a
design tool. Font Manager does not copy or install the file: it tells the operating
system where the file is.

## Two kinds of activation

| Kind | Key | How long it lasts |
|------|-----|-------------------|
| For this session | `A` | Until you quit Font Manager. |
| Permanent | `Shift+A` | Also after a restart, until you deactivate the font. |

Select one or more fonts and press the key. Press it again to deactivate them. In the
settings, under **Activation**, you choose which kind the **Activate** button uses.

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
