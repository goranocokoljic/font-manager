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
- Use the **List**, **Table**, **Grid** and **Waterfall** buttons to change how the
  fonts show. The grid shows a card for each font. The waterfall shows the preview
  text at six sizes. When you change the view, the selected fonts stay selected and
  the list stays at the same font.
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

> Font Manager makes no network calls unless you ask it to, and it has no accounts.
