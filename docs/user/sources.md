# Sources

A source is a folder that Font Fu reads fonts from. The library is the list of
all fonts in all sources.

## Add a folder

Select **Add folder** under **Sources** in the sidebar, or open the settings and go to
**Sources**. Choose a folder. Font Fu reads it and all the folders inside it.

The files stay where they are. Font Fu does not copy, move or change them.

To share fonts with a team through Dropbox, iCloud Drive, OneDrive, Google Drive or a
network share, read [Shared libraries](shared-libraries.md).

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

While Font Fu reads a folder, a progress bar shows how many files it has read.

## Automatic rescans

In the settings, under **Advanced**, you can tell Font Fu to rescan all folders
every few minutes. Set the interval to 0 to turn it off.

## Fonts that cannot be read

When a file is damaged or is not a font that Font Fu can read, the font shows a
health mark in the list. Font Fu does not delete or change the file.

## Problems

The bottom row of the sidebar shows how many problems your library has.
Select it to open the **Problems** view. The view lists:

| Group | What it means |
|-------|---------------|
| **Duplicates** | The same font is in two or more files. The list names the copy to keep. |
| **Broken files** | The file is damaged or has no Unicode character map. |
| **Legacy formats** | Font Fu can read the font but cannot activate it, for example a Type 1 font. |
| **Sidecar conflicts** | Another computer changed the same rating, note or license to a different value. See [Shared libraries](shared-libraries.md#conflicts). |
| **Activation conflicts** | An active font has the same PostScript name as an installed font or another active font. |
| **Folders and index** | A source folder is missing, or the library index was damaged and replaced. |

Select a group at the top of the view, or in the sidebar, to see only that group.
Font Fu does not change any file when it lists a problem.

### Resolve duplicates

Under **Resolve duplicates**, each duplicate font shows all its copies. The copy
marked **Recommended** is the newest version, or the copy in your library folder.

1. Select **Keep this one** next to the copy you want to keep.
2. Font Fu shows the copies that will move to the trash. Nothing has moved yet.
3. Select **Move to trash** to continue, or **Cancel** to stop.

The other copies go to the trash of your system: the Recycle Bin on Windows, the
Trash on macOS and Linux. Font Fu never deletes a file.

To undo, select **Undo** in the message at the bottom of the window. The message
stays for 10 seconds. After that, use **Undo** in the **History** list of the
Problems view. Undo puts every copy back where it was.

Some copies do not move:

- A copy that is active. Deactivate it first.
- A copy on a drive that has no trash, for example a network drive or a USB drive
  on Windows.

Undo does not replace a file that is now at the old path. Move that file away,
then select **Undo** again. If you emptied the trash, the copy cannot come back.

## Download fonts from an online catalog

You can get a free family from Google Fonts or Fontshare. Turn the catalog on
first, under **Online catalogs** in the sidebar.

### Preview a catalog in your language

The catalogs have the same preview row as your library. To see the families in
another language, choose it in the **Language** menu of the preview row. Each
family then shows the pangram of that language. Google Fonts and Fontshare use
the same choice. Your library keeps its own choice.

A letter that a family does not have shows as a box, as in your library. The
row also tells you how many glyphs are missing.

### Install a family

**Install** downloads a family and activates it in one step. Your design apps can
use it at once.

1. Open the catalog and find the family.
2. Select **Install** next to the family.
3. The first time, Font Fu asks where installed fonts go. It proposes the
   folder **Font Fu Fonts** in your Documents folder.
   - Select **Use this folder**. Font Fu makes the folder.
   - Or select **Choose…** and select another folder.

Font Fu remembers the folder. It is the one folder that Font Fu puts
fonts into without asking each time. It is never a system font folder. Later,
**Install** starts at once.

A line above the list tells you what happens: downloading, adding to the
library, then the result, for example "Installed Alegreya: 6 styles active".
Font Fu adds the folder to your library if it is not there yet. It
activates the styles in the mode that you set under **Settings** → **Activation**.

If the family is already in your library, Font Fu downloads nothing. It
activates the styles that are there, and the line tells you so.

To change the folder, open **Settings** → **Online catalogs** and select
**Change…** next to **Folder for installed fonts**.

### Download the fonts a document needs

Font Fu can install a family for you when a design app opens a document
that needs a font your library does not have. Turn it on in **Settings** →
**Online catalogs**: select **Download fonts a document needs from the catalogs
you turned on**. The switch works only when the folder for installed fonts is
chosen and at least one catalog is on.

Font Fu downloads only from the catalogs you turned on. It looks for the
family only in the font lists it already has; refresh a catalog to get its
newest list. The family goes into the folder for installed fonts, the same as
**Install**. See [Fonts a document needs](activation.md#fonts-a-document-needs).

### Download a family into a folder you choose

**Download…** is the manual way. It puts the files into a folder that you
select each time, and it does not activate them.

1. Open the catalog and find the family.
2. Select **Download…** next to the family.
3. Under **Download into**, select one of your folders. Downloads go only into a
   folder you added, never into the system fonts.
4. If the family has a variable font, select **Variable font** or **One file per
   style**.
5. Select **Download**.

Font Fu puts the files into a new folder with the name of the family, inside
the folder you selected. Then it reads the folder again, and the family shows in
your library. A file that is already there stays as it is. Font Fu never
replaces it. If the download fails, Font Fu writes no file.

Font Fu remembers where each downloaded family came from: the catalog, the
version and the date. It also remembers each file by its contents, so it still
knows the file after you move it to another folder of your library.

### Check for updates

Font Fu never checks for updates on its own. To check, open a catalog and
select **Check for updates**. Font Fu gets the lists of the catalogs that
you downloaded from again, and compares each downloaded family with its catalog.
**Refresh** compares them too.

- A family with a newer version shows **Update available**, in the catalog and
  in your library.
- A family that the catalog no longer has is shown as **No longer in the
  catalog**. Your files stay as they are.

To get the update, download the family again into another folder. Font Fu
never replaces a file that is already there.

Next: [Activation](activation.md).
