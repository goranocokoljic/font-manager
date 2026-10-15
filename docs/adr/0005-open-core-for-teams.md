# 0005 — Teams through synced folders; open-core team server later

Status: Accepted (2026-10-14)

## Context

Studios share font libraries. A server makes that easy but costs hosting, accounts and
security work, and the free app must stay offline and private: no account, no network
call without a user action.

## Decision

The free, open-source app (MIT OR Apache-2.0) supports teams through synced folders
plus sidecar metadata files that travel with the fonts. A team server — a central
library with admin controls — is a later, separately licensed add-on: an open-core
model.

## Consequences

- No server, accounts or telemetry in the free app; everything works offline.
- Sidecar metadata needs a merge rule for concurrent edits from synced machines.
- The team server, when it comes, talks to the same `fm-api` contract; nothing in the
  free app depends on it.
