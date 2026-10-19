# 0009 — The OS trash per OS

Status: Accepted (2026-10-19)

## Context

ADR 0004 says deletion goes to the OS trash and every move of a user's file has undo.
E8.3 ("Keep this one") is the first feature that removes files. Each OS has its own
trash, and none of the trash crates is in the spec's library table, so the code is
written in `fm-platform` with the crates it already has.

## Decision

`fm_platform::Trash` moves one file to the trash and returns where it went
(`Trashed`); `restore` moves it back. One implementation per OS:

| OS | Trash | How | Where the file goes |
|----|-------|-----|---------------------|
| Windows | Recycle Bin | `IFileOperation` with `FOFX_RECYCLEONDELETE` and a progress sink (`windows` crate) | `<drive>\$Recycle.Bin\<SID>\$R…`, as the sink reports it, with its `$I…` record |
| macOS | Finder trash | `NSFileManager trashItemAtURL:resultingItemURL:error:` (Objective-C runtime FFI) | the URL the call returns |
| Linux | freedesktop.org Trash spec 1.0 | rename + `.trashinfo` (std only) | `$XDG_DATA_HOME/Trash/files`, or `$topdir/.Trash/$uid` / `$topdir/.Trash-$uid` on another volume |

Restore is the same everywhere: rename the file back, then remove the OS record
(`$I…`, `.trashinfo`).

## Consequences

- Never a permanent delete. Windows deletes for good when the bin cannot take a file
  (no bin on the drive, the bin turned off, the file larger than the bin, a `subst`
  drive). Only fixed drives are used, and the progress sink's `PreDeleteItem` aborts
  every delete that is not `TSF_DELETE_RECYCLE_IF_POSSIBLE`. Either way the file is
  refused (`NoTrash`), stays where it is, and no prompt is shown. The sink is a COM
  object written by hand: the `#[implement]` macro needs `windows-core` as a direct
  dependency, which the library table does not list.
- The core writes the undo log in one transaction after the moves; when that write
  fails, the moved files are put back before the error is reported, so no file is
  ever in the trash without an undo.
- Restore never overwrites: a file at the old path again makes it fail (`Occupied`);
  the user moves that file and retries. A trash the user emptied cannot be undone.
- The Linux `DeletionDate` is written in UTC (no time-zone crate is in the library
  table); file managers only show it.
- macOS: Foundation is opened with `dlopen` on the first move, never linked. Every
  binary links `fm-platform`, and with Foundation loaded CoreText registration was
  slower (100-face activation over its 1 s budget on CI).
- Windows: the `$I` record holds the long path, so a path with 8.3 short names is
  looked up by its long form, and the file comes back under its long name.
- macOS: Finder's "Put Back" does not know a file the app moved back; it never needs to.
- Each implementation has the same integration test (`crates/fm-platform/tests/trash.rs`)
  against the real trash on its CI runner.
