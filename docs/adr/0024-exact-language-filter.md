# 0024 — The `supports` filter: exact for a language, a near miss for pasted text

Status: Accepted (2026-11-17, E54.4; amends the E3.8 rule)

## Context

The `supports` search filter (E3.8) asks a face for some characters: the base
characters of one or more language presets (`Supports::Languages`), or the characters
of a pasted text (`Supports::Text`). E3.8 had no ADR of its own; its rule lived in
`fm_core::coverage::NEAR_MISS` (5): a face that lacks at most five of the characters
asked for is still a result, lists what it lacks, and ranks after the complete faces.

For a language this rule is wrong. Serbian (Latin), Croatian and Bosnian differ from
basic Latin by ten code points (Č Ć Đ Š Ž in both cases), so a face that lacks four of
the ten passes. The fixture Neuton-Light lacks Ć Đ ć đ and was a Serbian result.

## Decision

Decided by Goran, 17 Nov 2026:

1. **A language is exact.** With language presets, the filter returns a face only when
   it maps every base character of every chosen preset. One missing letter disqualifies
   the face: it is not a result and it is not offered as a near miss. Whoever needs a
   font for a language needs all of its letters.
2. **Pasted text keeps the near miss, for now.** A face that lacks at most `NEAR_MISS`
   (5) characters of a pasted text ("covers this text") is still a result and lists
   what it lacks. The constant stays and is used by this path only. **This allowance is
   to be reconsidered.**

Everything that counts by language follows rule 1, because it all checks a face through
the one `fm_core::coverage::Need`: the search and its filter counts, smart collections
with a language rule (their counts are computed live, so an existing one shows its
new, smaller count after the update without a rescan), drop counts, the brief and
pairing workflows, `fm search --supports` and the MCP search tool. The Rust `MockApi`
and the TypeScript mock follow the same two rules.

Unchanged: the coverage report (`check_coverage`, which lists every missing character
of a face) and the tofu line on a row (E43.4). No `fm-api` type changes.

## Consequences

- A language search returns fewer families: the near misses are gone. `FamilyHit.missing`
  is still sent for a language search, with no characters.
- `fm search --supports` keeps its MISSING column; for a language it is empty.
- A face whose coverage is not stored still fails the filter, as before.
