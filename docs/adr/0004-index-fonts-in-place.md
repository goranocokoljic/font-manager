# 0004 — Index fonts in place; no vault

Status: Accepted (2026-10-14)

## Context

Designers keep fonts in folders they already organize, often synced or shared. Some
font managers copy everything into a private vault, which doubles disk use, breaks
other tools' paths and makes the manager the owner of the user's files.

## Decision

The library is an index over folders the user chose. Fonts stay where they are by
default. Copying into a library folder on import is an explicit alternative the user
picks. There is no vault.

## Consequences

- The app never touches user files uninvited. Moving, renaming or deleting a font file
  is an explicit user action with undo, and deletion goes to the OS trash.
- Session activation registers the file from its current path; permanent activation is
  per-user and needs no admin rights. Nothing is written to system font folders.
- The scanner must handle files that move or vanish (watcher events, re-scan).
