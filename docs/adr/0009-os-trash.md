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
| Windows | Recycle Bin | `SHFileOperationW`, `FOF_ALLOWUNDO` (`windows` crate) | `<drive>\$Recycle.Bin\<SID>\$R…`; found by its `$I…` record |
| macOS | Finder trash | `NSFileManager trashItemAtURL:resultingItemURL:error:` (Objective-C runtime FFI) | the URL the call returns |
| Linux | freedesktop.org Trash spec 1.0 | rename + `.trashinfo` (std only) | `$XDG_DATA_HOME/Trash/files`, or `$topdir/.Trash/$uid` / `$topdir/.Trash-$uid` on another volume |

Restore is the same everywhere: rename the file back, then remove the OS record
(`$I…`, `.trashinfo`).

## Consequences

- Never a permanent delete. Windows deletes for good on a drive without a Recycle Bin,
  so only fixed drives are used; a file elsewhere is refused (`NoTrash`) and stays.
  A file too large for the bin makes Windows ask the user (`FOF_WANTNUKEWARNING`).
- Restore never overwrites: a file at the old path again makes it fail (`Occupied`);
  the user moves that file and retries. A trash the user emptied cannot be undone.
- The Linux `DeletionDate` is written in UTC (no time-zone crate is in the library
  table); file managers only show it.
- macOS: Finder's "Put Back" does not know a file the app moved back; it never needs to.
- Each implementation has the same integration test (`crates/fm-platform/tests/trash.rs`)
  against the real trash on its CI runner.
