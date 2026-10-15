# 0003 — SQLite with FTS5 for the index

Status: Accepted (2026-10-14)

## Context

Search must answer in under 50 ms over 10,000 fonts, with text search, many filters
(language support, features, variable, classification) and smart collections. The
index must work offline, survive a crash and be rebuildable from the folders.

## Decision

One SQLite database through `rusqlite`, with an FTS5 virtual table for text search.
Tables: `source` → `font_file` → `face`, plus collections, tags, sets, `user_meta` and
`activation`. User metadata is also mirrored to sidecar JSON. The preview cache lives
on disk, keyed by file hash + settings hash.

## Consequences

- One embedded file, no server, fast enough for the targets, and well understood.
- The index is a cache: it can be dropped and rebuilt from the folders. User metadata
  (ratings, tags, notes) is backed up on every change, so a corrupted database recovers
  without data loss.
- Writes that must be consistent run in transactions; FTS5 query text is escaped.
