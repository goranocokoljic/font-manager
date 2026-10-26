# Shared libraries

A team can share one font library through a synced folder: Dropbox, iCloud Drive,
OneDrive, Google Drive for desktop or a network share (SMB). Each person adds the same
folder to Font Manager. The sync tool copies the fonts. Font Manager reads them in place,
as with every other folder (see [Sources](sources.md)).

Font Manager has no server and no accounts. The sync tool does all the copying. Font
Manager makes no network calls of its own for a shared folder.

## Before you start

- Every teammate adds the **same** folder: the top folder of the shared fonts. Do not
  add a parent folder on one computer and a folder inside it on another. Font Manager
  finds your tags, ratings and notes by the path of each font inside the folder that
  you added, so the folder must be the same on every computer.
- Keep the fonts **available offline**. A file that is only in the cloud is not on your
  disk. Font Manager cannot read it until the sync tool downloads it, and an app cannot
  use an active font that is not on the disk. Each section below tells you how to keep
  the files on the disk.
- When the sync tool changes a file while Font Manager runs, Font Manager usually sees
  the change at once. It also rescans all folders every hour, and when you select
  **Rescan**. To change the interval, open the settings and go to **Advanced**.

## Dropbox

Dropbox has a client for Windows, macOS and Linux.

| System | How to add the folder |
|--------|-----------------------|
| Windows | Install Dropbox. Select **Add folder** and choose the shared folder in your Dropbox folder, usually `C:\Users\<name>\Dropbox`. |
| macOS | Install Dropbox. Select **Add folder** and choose the shared folder under **Dropbox** in the Finder sidebar, usually `~/Library/CloudStorage/Dropbox`. |
| Linux | Install Dropbox from dropbox.com. Select **Add folder** and choose the shared folder in `~/Dropbox`. |

To keep the fonts offline:

- On Windows and macOS, right-click the shared folder and choose **Make available
  offline**. A folder that is **Available online-only** keeps only placeholders on your
  disk.
- On Linux, the Dropbox client keeps every synced file on the disk. Make sure that
  **Selective Sync** includes the shared folder.

## iCloud Drive

iCloud Drive is part of macOS. On Windows it comes with the iCloud for Windows app.
**Apple has no iCloud Drive client for Linux.** A teammate on Linux cannot use an
iCloud Drive folder; use one of the other services for a team with Linux computers.

| System | How to add the folder |
|--------|-----------------------|
| Windows | Install iCloud for Windows and turn on **iCloud Drive**. Select **Add folder** and choose the shared folder in `C:\Users\<name>\iCloudDrive`. |
| macOS | Turn on **iCloud Drive** in System Settings → your name → iCloud. Select **Add folder** and choose the shared folder under **iCloud Drive** in the Finder sidebar. |
| Linux | Not possible. Apple has no client. |

To keep the fonts offline:

- On macOS, **Optimize Mac Storage** removes files from the disk when the disk gets
  full. Turn it off in System Settings → your name → iCloud → iCloud Drive. Or
  right-click the shared folder and choose **Keep Downloaded**.
- On Windows, right-click the shared folder and choose **Always keep on this device**.

## OneDrive

OneDrive is part of Windows. On macOS it is an app from the App Store or from
microsoft.com. **Microsoft has no OneDrive client for Linux.** Clients from other people
are available. Font Manager can read a folder that such a client keeps on the disk, but
Microsoft does not support them.

| System | How to add the folder |
|--------|-----------------------|
| Windows | Sign in to OneDrive. For a folder that a teammate shares with you, select **Add shortcut to My files** on the OneDrive website. Select **Add folder** and choose the folder in `C:\Users\<name>\OneDrive`. |
| macOS | Install OneDrive and sign in. Add the shared folder as on Windows. Select **Add folder** and choose it under **OneDrive** in the Finder sidebar, usually `~/Library/CloudStorage/OneDrive-<name>`. |
| Linux | Not possible with a client from Microsoft. |

To keep the fonts offline: **Files On-Demand** keeps only placeholders of files that
you have not opened. Right-click the shared folder and choose **Always keep on this
device**. The folder then shows a green circle with a white check mark.

## Google Drive for desktop

Google Drive for desktop is an app for Windows and macOS. **Google has no Google Drive
client for Linux.** A teammate on Linux cannot use a Google Drive folder through Google.

| System | How to add the folder |
|--------|-----------------------|
| Windows | Install Google Drive for desktop and sign in. For a folder that a teammate shares with you, add a shortcut to it in **My Drive**. Select **Add folder** and choose the folder in the Google Drive drive (usually `G:\My Drive`). |
| macOS | Install Google Drive for desktop and sign in. Add the shared folder as on Windows. Select **Add folder** and choose it under **Google Drive** in the Finder sidebar, usually `~/Library/CloudStorage/GoogleDrive-<email>/My Drive`. |
| Linux | Not possible. Google has no client. |

Google Drive for desktop has two modes. Choose one in its settings, under **Google
Drive**:

| Mode | What it does | What to do |
|------|--------------|------------|
| **Stream files** | Files stay in the cloud. The app downloads a file when you open it. | Right-click the shared folder and choose **Offline access** → **Available offline**. |
| **Mirror files** | Every file is in the cloud and on your disk. | Nothing. The fonts are always offline. |

## Network shares (SMB)

A network share is a folder on a server or a NAS (a network storage box) in your office.
Windows, macOS and Linux can all connect to it. A share has no offline copy: when the
computer is not connected, the fonts are not there.

| System | How to add the folder |
|--------|-----------------------|
| Windows | In File Explorer, select **This PC** → **Map network drive**. Type the address, for example `\\server\fonts`, and select **Reconnect at sign-in**. Then select **Add folder** and choose the drive. |
| macOS | In the Finder, choose **Go** → **Connect to Server**. Type the address, for example `smb://server/fonts`. The share shows under `/Volumes`. Then select **Add folder** and choose it. To connect at each login, add the share to **Login Items** in System Settings → General. |
| Linux | In the file manager, open **Other Locations** and type the address, for example `smb://server/fonts`. GNOME puts the share under `/run/user/<id>/gvfs`. For a fixed path, ask your administrator to mount it with `cifs` in `/etc/fstab`, for example at `/mnt/fonts`. Then select **Add folder** and choose it. |

When the share is not connected:

- The folder shows under **Folders and index** in **Problems**. Connect the share and
  select **Rescan**.
- A font that you activated from the share stops working in other apps.
- **Move to trash** does not work on a network drive on Windows, because it has no trash.
  Font Manager does not delete the file.

A network share sends no change events to Font Manager. Font Manager finds changes at
the next rescan.

## The `.fontlibrary` folder

Your tags, ratings, favorites, notes, collections and licenses are in the library of your
computer. Font Manager also writes them into a folder with the name `.fontlibrary`, in
the top of each folder that you added. This folder has one small JSON file (a text file)
for each family: a sidecar. The sync tool copies the sidecars to your teammates with the
fonts. When Font Manager on another computer reads a changed sidecar, it adds the changes
to that library.

- Font Manager writes only into `.fontlibrary`. It does not change the font files.
- Font Manager writes no sidecars for **System fonts**, and does not change the sidecars
  of a paused folder.
- **Do not edit or delete a sidecar while the folder syncs.** Make changes in Font
  Manager. A sidecar that Font Manager cannot read shows in **Problems**, and Font
  Manager does not write that file again until it can read it.
- A sidecar that a newer version of Font Manager wrote is only read, never written.
  Update Font Manager on every computer of the team.

## Conflicts

Two people can change the same font before their computers sync. Font Manager adds both
changes together:

- Tags and collections: what each person added stays, and what each person removed goes.
  These never conflict.
- Favorite, rating, notes, family notes and license: when only one person changed a
  value, Font Manager takes it. When both people changed it to different values, your
  value stays and the other value is a **conflict**. Font Manager does not lose it.

Some sync tools cannot add two versions of a file together. They keep both, and give
the second file a new name, a **conflicted copy**:

| Sync tool | Name of the conflicted copy |
|-----------|-----------------------------|
| Dropbox | `<name> (Anna's conflicted copy 2026-10-26).json` |
| OneDrive | `<name>-<computer name>.json` |
| Google Drive | `<name> (1).json` |
| iCloud Drive | `<name> 2.json` |

Font Manager reads a conflicted copy as one more version of the sidecar. It writes the
result to the sidecar of the family, and it does not change the copy. When no conflict of
that family shows in **Problems** any more, you can delete the copy.

### Resolve a conflict

1. Select **Problems** in the sidebar, then the **Sidecar conflicts** group.
2. Each conflict shows the family, the field (for example **Rating**), the value on
   **This computer** and the value on the **Other computer**.
3. Select **Keep mine** to keep your value, or **Use theirs** to take the other value.

Font Manager writes your choice to the sidecar. The other computer takes it at its next
sync, and it does not show a new conflict.

## Share a project set

A project set does not go into the sidecars. To share one, export it as a set file:

1. Under **Project sets**, point to the set and select **Export set…**.
2. Save the file in the shared folder, or send it to your teammate. Its name ends in
   `.fmset.json`.
3. Your teammate selects **Import set…** next to **Project sets** and chooses the file.

The set file names each font by the contents of its file (a hash), not by its path. So a
font matches on each computer, also when the shared folder is at a different path or the
file has a new name. The set file holds no fonts. A note lists the fonts that the library
of your teammate does not have. Read [Activation](activation.md#share-a-set-with-a-teammate)
for more.

## Limits

- **No locking.** Two people can change the same font at the same time. Font Manager
  shows a conflict; it cannot stop the second change.
- **No admin controls.** Each person who can write to the folder can change the tags,
  ratings and notes of the team. Font Manager has no roles and no permissions of its own.
  Use the permissions of the sync tool or the server.
- **A read-only share keeps your metadata local.** When you cannot write to the folder,
  Font Manager cannot write sidecars. Your tags, ratings and notes stay in the library
  of your computer, and your teammates do not get them. You still get the changes of the
  people who can write. When you can write to the folder again, Font Manager writes your
  changes at the next change or rescan.
- **One value for the team.** A sidecar does not keep a value for each person. A rating
  or note is the value of the team.

Next: [Activation](activation.md).
