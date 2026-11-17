# fixtures/similar

`expected.json` is the answer sheet for **Similar**: pairs a person agreed on by eye. For each
reference face it lists the faces that should rank **near** it and the faces that should rank
**far** from it. The score in `crates/fm-core/src/similar.rs` is tuned against it (E54.13): for
every reference, every near face must rank above every far face.

`expected.schema.json` describes the file. `the_agreed_pairs_are_well_formed` in
`crates/fm-core/src/similar/tests.rs` checks the file and the schema.

## How the pairs were chosen

- All faces come from the fixture corpus (`fixtures/fonts`), so the tests can read them.
- Every face was set in the same line ("Hamburgefonstiv Rag 123") at one size, in a browser,
  and judged by eye from that sheet. A variable font is judged at its default instance.
- A pair is in the file only when the answer is plain. "Near" means: a designer who looks for a
  replacement would accept it in the list. "Far" means: its place high in the list would look
  like a fault. A face about which two people could disagree is in neither list.
- "Near" is strict: highly similar, not only related. An italic is not near an upright face, a
  condensed width is not near the normal width, and one letter of another build (a different
  g) is enough to leave a face out. Two faces of one kind (two handwriting faces, two heavy
  heading faces) are not near for that reason alone.
- A sibling family is near when it also looks alike (Lexend Tera and Lexend Peta, Asap and
  Asap Sharp at the same width and slant).
- The file covers serif, sans, slab, script, display and mono, and the two faults seen in use:
  a sibling family ranked below an unrelated sans, and a handwriting face ranked near a display
  serif.
- Fonts whose Latin letters are a fallback (most of the Noto script fonts of the corpus) and
  the broken, TTC and Type 1 fixtures are not used.

The first version (November 2026) was drafted from that sheet and then checked by the owner,
who took out the references and the near faces that were related but not alike.

## How to add a pair

1. Pick the reference and the other faces from `fixtures/fonts/manifest.tsv`. Use the path as
   the manifest gives it.
2. Look at them side by side in the app (Compare) or in a browser. Add a face only when the
   answer is plain.
3. Add the reference to `references` with its `class`, a `note` that says what the eye goes by,
   and at least one `near` and one `far` face. To add a face to a reference that exists, put it
   in that reference's `near` or `far` list.
4. Run `cargo nextest run -p fm-core similar`. The ranking test
   (`every_near_face_ranks_above_every_far_face`, E54.13) fails when the score
   disagrees with the new pair: then the pair or the score is wrong, and a person decides.
