# Getting started

Font Manager shows every font on your computer in one list. You can search them,
preview them and make them available to other apps. It is free and works offline.

## Your fonts stay where they are

Font Manager does not copy, move or change your font files. It reads the folders you
choose and keeps a list of the fonts in them. When you remove a folder from the app,
the files stay on your disk.

## Add your first folder

1. In the sidebar, under **Sources**, select **Add folder**.
2. Choose the folder that holds your fonts.
3. Font Manager reads the folder. A progress bar shows how many files it has read.

The fonts on your system are always in the list too. They show as **System fonts**.
Read [Sources](sources.md) to learn more about folders.

## Find a font

- Type in the search box at the top. Press `/` to go to it from anywhere.
- The list shows a preview of each font. Change the preview text and size above the
  list.
- Use the **List**, **Table**, **Grid**, **Waterfall** and **Layout** buttons to change
  how the fonts show. The grid shows a card for each font. The waterfall shows the
  preview text at six sizes. The layout shows the selected font as a page of text. Pick
  the page in the **Preset** box: Book, Columns, Magazine, Quote, Characters or
  Waterfall. When you change the view, the selected fonts stay selected and the list
  stays at the same font.
- To resize a block of text in the layout, drag it: drag to the right or left to make
  the text larger or smaller, and drag up or down to add or remove space between the
  lines. Hold `Ctrl` and turn the mouse wheel over a block to change its size. On a
  trackpad on Windows or a Mac, pinch the block to change its size. Press `Esc` during a
  drag to go back.
- In the layout, double-click a block of text (or press `Enter` on it) to change the
  text. Click **Save** to keep the change, or **Revert** to drop it. **Duplicate** makes
  a copy of the layout with a new name. **Reset to original** puts back the preset as
  the app ships it. **History** shows the earlier versions; **Restore** makes one current
  again, and the app keeps the version it replaces.
- The badge beside each block shows its size (for example "24 pt"). Click it (or press
  `Enter` on it) to open the properties of that block: size, line height, tracking,
  paragraph spacing, first-line indent, columns (1–3), alignment, and wrap or clip. The
  block changes at once; the other blocks stay as they are. A value out of range goes to
  the nearest limit. Press `Escape` to close the properties.
- The text of a layout has a width that is easy to read: about 65–75 characters on a
  line of the body text. The text is in the center of the window, and a wider window
  does not make it wider. Click **Lock width** to turn this off: the text then follows
  the width of the window. Click it again to keep the width that the text has at that
  moment. The command palette has this command too ("Lock or unlock the layout width").
  Click **Save** to keep the width with the layout.
- Each layout is a text file (JSON) in the `layouts` folder of the app's config folder.
  You can edit a file by hand: the app reads it the next time you open the layout view.
  A file the app cannot read shows as "cannot be read" in the **Preset** box.
- Select a font to see its details in the inspector on the right. Press `Ctrl+I`
  (`Cmd+I` on a Mac) to show or hide the inspector.
- Open the **Features** tab of the inspector to see the OpenType features of the
  font, for example small capitals or oldstyle figures. Turn a feature on or off and
  the preview of that font changes. **Reset** sets the features back to the default.
  Features that the text engine applies by itself show "applied automatically". The
  app keeps your choices for each font until you quit it.
- Open the **Glyphs** tab to see every character of the font, in groups by Unicode
  block (for example Basic Latin or Cyrillic). Choose a block in the list to see only
  that block. Type in the search field to find a character: type the character itself
  (`Ж`), its code point (`U+0416` or `0416`) or its glyph name (`uni0416`). Click a
  character to see its code point, glyph name and glyph id.
- Write notes in the **Info** tab. **Notes** are for the style that you selected.
  **Family notes** are for all styles of the family. Font Manager saves a note when
  you leave its field. To delete a note, clear its field. The **Notes** column of the
  table shows the family notes. Notes stay when Font Manager reads the folder again.
- Record your license in the **License** section of the **Info** tab. Set the license
  type, the number of seats and the purchase date, then click **Save license**. The
  license is for all styles of the family. To attach a receipt, choose where Font
  Manager keeps its copy and click **Attach receipt…**. **Next to the font** puts the
  copy in the `.fontlibrary` folder of the font's folder, so a synced folder carries
  it to your team. Font Manager copies the receipt; your file stays where it is. When
  you remove or replace a receipt, Font Manager moves its copy to the trash.
- To get a list of your licenses for a spreadsheet, open the command palette and
  choose **Export license inventory…**, or click **Export license inventory…** on a
  collection. Choose where to save the CSV file. The file has one row for each
  family: its foundry, styles, formats, your license, the open license and the
  embedding permission of the font, its license URL and its folder. From the palette,
  the file lists the collection that is open, or all fonts.

## Make a smart collection

A smart collection fills itself from rules. Click **New smart** in the
**Collections** group of the sidebar, or choose **New smart collection** in the
command palette. Give the collection a name. Then make the rules:

1. In each rule, choose what the rule tests (for example **Class**, **Weight**,
   **Supports language** or **Tag**). Then choose its value. The values come from
   your library.
2. Choose **all of** to keep fonts that match every rule, or **any of** to keep fonts
   that match one rule.
3. Click **Add group** to put rules in a group with its own **all of** or **any of**.
   For example: Class is Sans serif, and a group of **any of** with Cyrillic and
   Greek.

The editor shows how many families match while you work. Click **Save**. When a rule
cannot be saved (for example, a group with no rules, or a lowest weight above the
highest weight), the editor tells you why and saves nothing. To change the rules
later, click **Edit rules** on the smart collection. All controls work from the
keyboard: `Tab` moves between them, `Enter` adds or removes a rule, `Esc` closes the
editor.

## Use a font in other apps

Select a font and press `A`. The font is now active: other apps can use it until you
quit Font Manager. Press `Shift+A` to keep it active after a restart. Read
[Activation](activation.md) for the details.

## Do everything from the keyboard

Press `Ctrl+K` (`Cmd+K` on a Mac) to open the command palette. Type a few letters of
what you want to do and press `Enter`. All keys are on the
[Keyboard shortcuts](keyboard-shortcuts.md) page.

## Settings

Press `Ctrl+,` (`Cmd+,` on a Mac) to open the settings. There you can choose the theme,
the size of the rows, and how activation works.

## Get updates

Open Settings → About and click **Check for updates**. If a new version is out, Font
Manager shows its number and release notes, and installs it only when you click
**Install and restart**. A major version (for example 1.x to 2.0) asks you a second time.
Installing quits Font Manager first, so session activations end, and then starts the new
version. Turn on **Check automatically when Font Manager starts** to check at every start.
It is off until you turn it on.

Font Manager installs only updates signed with its release key. If you installed it with
Flatpak or a package manager, that tool installs the updates, and Settings → About says
so under Platform notes.

> Font Manager makes no network calls unless you ask it to, and it has no accounts.
