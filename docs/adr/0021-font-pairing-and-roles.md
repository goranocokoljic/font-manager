# 0021 — Font pairing: the rules, the model and role-aware scoring

Status: Accepted (2026-11-04, E42.13)

## Context

Story E42.13 asks for an amendment to "the pairing ADR", but pairing never had an ADR: the
rules (E23.4) and the model (E33.2) are documented in `fm_core::pairing` only. So this
record states what was already true, then adds the amendment.

The Family page (E42.7) and the Layout Studio's shuffle (E45) ask a sharper question than
the Compare sheet does: not "what goes with this face?" but "what goes with this body face
as a headline?". A good headline and a good body partner are different faces, so one
ranking cannot answer both. `fm-api` is a public contract, so the change must be additive.

## Decision

**What was already true.** `suggest_pairings` looks at one face per other family, the
closest in weight, then slant. Rules in `fm_core::pairing` decide what is a pairing and
give a score: contrasting classes, the same superfamily, the same designer, a matching
x-height. A local model (`data/pairings/model.json`, inside the binary, no download) gives
a probability from curated pairings, and takes `MODEL_SHARE` of the score.

**The amendment: two optional request fields.**

- `role?: 'headline' | 'body' | 'caption'` is the role the *suggested* face plays. Absent,
  the answer is the same as before this ADR, to the byte.
- `partnerOf?: FaceId` is the face the suggestions must go with. `face` stays required,
  because making it optional would change an existing type (rule 6). With `partnerOf`
  set, `face` is the face being replaced: its family is not suggested, so a shuffle moves
  on. To ask "given this body face, suggest headlines", send that face in both fields, or
  send `{ face: bodyFace, role: 'headline' }`.

**The rules still decide what is a pairing.** A role only moves the pairings they let
through: `fm_core::pairing::role::fit` scores four facts for the role, and `blend` gives
that score `ROLE_SHARE` (0.5) of the result. A fact one of the faces lacks scores nothing.

| Role | Stroke contrast | x-height | Width | Era |
|------|-----------------|----------|-------|-----|
| headline | 0.45 | 0.10 | 0.25 | 0.20 |
| body | 0.30 | 0.40 | 0.15 | 0.15 |
| caption | 0.20 | 0.45 | 0.30 | 0.05 |

- **Stroke contrast** is the fingerprint's measure of the `o`. A headline earns by having
  more than its partner; body and caption earn by having less. A gap of 0.3 earns the full
  share. Large type can carry thin strokes; small type loses them.
- **x-height** earns by closeness to the partner, nothing at a gap of 20 %. A caption also
  earns in full with a larger one, because small type needs it.
- **Width** (`OS/2.usWidthClass`): a headline earns by differing from its partner; body
  and caption earn by staying near normal.
- **Era** earns by years close together, nothing at 50 years. It weighs least for captions,
  where nobody sees it, and most for headlines, where it shows.

**Era is the copyright year.** Fonts state no design era. The earliest four-digit year in
the name table's copyright (name ID 0) is the only stored fact that comes close, and it
needs no new column or scan. It is a weak signal: a 2011 revival of a 1530 design reads as
2011. That is why era never outweighs the measured facts in any role.

**Reasons stay plain words and name the role**: `lower contrast for body`, `higher contrast
for headline`, `x-height within 4 %`, `larger x-height for caption`, `narrower for
headline`, `normal width for caption`, `same era`. Role reasons come first. `x-height within
N %` takes the place of the rules' `similar x-height`; N is rounded up, so the words stay
true. A reason that would fit nearly every result is noise, so `normal width` needs a
partner that is not of normal width, and `same era` needs years at most 5 apart.

## Consequences

- No new column, no migration and no model change. The model's features and its committed
  `model.json` are untouched: a role moves the blended score, not the training.
- The search cache holds one more small number per face, read from `face.parsed`. A role
  request reads the fingerprints that a plain request already reads for the model, so it
  stays in the 50 ms search budget; the `pairings` bench metric times both.
- The MCP `get_pairings` tool and the plain Compare sheet keep sending no role.
- The mocks have no stroke contrast or era. They let the class stand in, so a role changes
  their order and reasons in the same shape, and the UI stories can test against them.
- The weights are judgment, not training data. If curated pairings gain roles, they can be
  fitted the way the model is; that would supersede the table above.
