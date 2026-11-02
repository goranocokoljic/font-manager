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

## Share a set with a teammate

To give a set to a teammate, point to the set under **Project sets** and click
**Export set…**. Choose where to save the set file. Its name ends in `.fmset.json`.
The file names each font by the content of its file. It holds no fonts, so your teammate
must have the same font files.

To use a set file, click **Import set…** next to **Project sets** and choose the file.
Font Manager finds each font by its content, so a renamed or moved copy still matches.
The new set holds the fonts that your library has. A note lists the fonts that your
library does not have, by family and style.

## Undo

When you activate or deactivate many fonts at once (several rows, a family,
**Deactivate all**, or **Deactivate everything not in the current set**), a message at the bottom of the window tells you what changed. Click **Undo** in the message to put every font back as it
was, in the same kind of activation. The message goes away after 10 seconds. It stays
while the pointer is on it.

## Drag a font to a design app

Drag a font or a family from the list, the grid or the table onto selected
text in a design app. The drag gives the app the font's PostScript name and
its file. When the pointer leaves the window, an inactive font is activated
for this session, so the app can use it. The font stays active after the
drop; the app does not deactivate it. If the font cannot be activated, a
message tells you why.

With the keyboard: select the font, open the **Developer** tab of the
inspector and choose **Copy for Font Switch**. This copies the same data to
the clipboard.

## Fonts a document needs

The Font Manager plugins for design apps (Photoshop, Illustrator, InDesign and
Sketch) tell Font Manager which fonts a document uses when you open it. Font
Manager finds them in your library and activates them for this session. When you
close the document, the plugin asks if it can deactivate the fonts that only
this document used. The question names these fonts, for example:

> Deactivate 3 fonts used only by brand.ai? Alegreya-Bold, Alegreya-Italic, Actor-Regular

A font shows with its PostScript name, or with its family and style when it has
no PostScript name. When more than 8 fonts are in the list, the question names
the first 8 and then says "and N more". Select **Deactivate** to deactivate
them, or **Keep** (**Keep them** in the Adobe apps) to keep them active.

A font that your library does not have is listed as missing.

The fonts of a Photoshop document are the fonts that its text uses. Photoshop
also stores its default fonts, for example Myriad Pro, in every text layer.
Font Manager does not list a default font that no text uses.

### Download a missing font from a catalog

Font Manager can also download a missing font from an online catalog. This is
off until you turn it on:

1. Open **Settings** → **Online catalogs**.
2. Turn on a catalog, for example Google Fonts, and choose the **Folder for
   installed fonts**.
3. Select **Download fonts a document needs from the catalogs you turned on**.

When a document needs a font that your library does not have, Font Manager looks
for its family in the font lists of the catalogs you turned on. It looks only in
the lists it already has, so the search itself does not use the internet. When it
finds the family, it downloads it into the folder for installed fonts, adds it to
your library and activates the font for the document. The plugin tells you, for
example "Downloaded 1 family from Google Fonts: Alegreya".

Font Manager downloads only from catalogs you turned on, and only when a design
app opens a document. A download that fails, or that takes longer than one
minute, leaves the font missing; the other fonts are not affected.

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

## Keep fonts active after you log in

Session fonts stay active while Font Manager runs. To have Font Manager start in the
tray when you log in, open **Settings → Activation** and turn on **Start at login**.
Font Manager adds itself to the apps that your system starts at login:

| System | The login entry |
|--------|-----------------|
| Windows | A value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| macOS | A login item, `~/Library/LaunchAgents/org.fontmanager.app.autostart.plist` |
| Linux | `~/.config/autostart/org.fontmanager.app.desktop` (or under `$XDG_CONFIG_HOME`) |

Turn the switch off to remove the entry. Font Manager removes only an entry that it
wrote. If you wrote a file with the same name yourself, Font Manager does not change
or remove it.

> Inside a Flatpak, Font Manager cannot add itself to the apps that start at login,
> so **Start at login** is not shown. **Settings → About → Platform notes** says so.
