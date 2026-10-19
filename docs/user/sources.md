# Sources

A source is a folder that Font Manager reads fonts from. The library is the list of
all fonts in all sources.

## Add a folder

Select **Add folder** under **Sources** in the sidebar, or open the settings and go to
**Sources**. Choose a folder. Font Manager reads it and all the folders inside it.

The files stay where they are. Font Manager does not copy, move or change them.

## The System fonts source

The fonts that your operating system installed are always in the list. This source
shows as **System fonts**. You cannot pause or remove it.

## Rescan, pause and remove

Each folder in the sidebar has these buttons:

| Button | What it does |
|--------|--------------|
| **Rescan** | Reads the folder again, to find fonts you added, changed or deleted. |
| **Pause** | Hides the fonts of the folder from the library and stops reading it. **Resume** brings them back. |
| **Remove** | Takes the folder out of the library. The files stay on your disk. |

While Font Manager reads a folder, a progress bar shows how many files it has read.

## Automatic rescans

In the settings, under **Advanced**, you can tell Font Manager to rescan all folders
every few minutes. Set the interval to 0 to turn it off.

## Fonts that cannot be read

When a file is damaged or is not a font that Font Manager can read, the font shows a
health mark in the list. Font Manager does not delete or change the file.

## Problems

**Problems** in the sidebar shows how many problems your library has. The status bar
shows the same number. Select it to open the **Problems** view. The view lists:

| Group | What it means |
|-------|---------------|
| **Duplicates** | The same font is in two or more files. The list names the copy to keep. |
| **Broken files** | The file is damaged or has no Unicode character map. |
| **Legacy formats** | Font Manager can read the font but cannot activate it, for example a Type 1 font. |
| **Sidecar conflicts** | Another computer changed the same tag, rating or note to a different value. |
| **Activation conflicts** | An active font has the same PostScript name as an installed font or another active font. |
| **Folders and index** | A source folder is missing, or the library index was damaged and replaced. |

Select a group at the top of the view, or in the sidebar, to see only that group.
Font Manager does not change any file when it lists a problem.

### Resolve duplicates

Under **Resolve duplicates**, each duplicate font shows all its copies. The copy
marked **Recommended** is the newest version, or the copy in your library folder.

1. Select **Keep this one** next to the copy you want to keep.
2. Font Manager shows the copies that will move to the trash. Nothing has moved yet.
3. Select **Move to trash** to continue, or **Cancel** to stop.

The other copies go to the trash of your system: the Recycle Bin on Windows, the
Trash on macOS and Linux. Font Manager never deletes a file.

To undo, select **Undo** in the message at the bottom of the window. The message
stays for 10 seconds. After that, use **Undo** in the **History** list of the
Problems view. Undo puts every copy back where it was.

Some copies do not move:

- A copy that is active. Deactivate it first.
- A copy on a drive that has no trash, for example a network drive or a USB drive
  on Windows.

Undo does not replace a file that is now at the old path. Move that file away,
then select **Undo** again. If you emptied the trash, the copy cannot come back.

Next: [Activation](activation.md).
