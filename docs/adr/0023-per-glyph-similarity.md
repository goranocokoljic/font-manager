# 0023 — Per-glyph similarity: a histogram of edge orientations, stored per face and glyph

Status: Accepted (2026-11-10, E49.8)

## Context

Epic E49 lets a designer find fonts by their letters. Its third part (E49.8, E49.9) is
"an R like this one": pick a reference glyph and sort the library by how close each
face's version of that glyph is. The whole-face `findSimilar` (E23.2) compares a
fingerprint of a sample string; it cannot say that two faces have the same `R` and a
different `g`. The story asks for a shape descriptor of one glyph, rendered at 64 px at
the default instance, normalised to the em and centred, and leaves the choice of
descriptor to this record. It must:

- agree with the named traits where both apply: the nearest neighbours of a straight-leg
  `R` in the labelled corpus (`fixtures/glyph-traits-labels.json`) are straight-leg `R`s
  in at least 85 % of cases;
- answer a query over a 10k library from stored descriptors in under 150 ms;
- be computed for the trait glyphs inside the background trait pass (E49.4) without
  breaking its 60 s budget, and for any other glyph on demand, then cached;
- give the same numbers on every OS (rule 5) and never panic on malformed data.

## Decision

**The descriptor is a histogram of oriented gradients (HOG)** of the glyph's coverage
image, in `fm_core::glyphs::similarity`:

1. The glyph is drawn unhinted at its default instance with the rasterizer the
   fingerprints already use (swash/zeno), at 64 px to the em, its outline box centred in
   a 64 × 64 square. Baseline position and advance do not count.
2. At every pixel the coverage gradient (central differences) votes, by its strength,
   for 8 unsigned edge orientations in a 6 × 6 grid of cells. Votes are soft in both
   directions: each orientation gets the squared positive cosine to the edge (the votes
   of one pixel always sum to the same, and no `atan2` is needed: only `+ − × ÷ √`, so
   the result is the same on every OS), and each pixel is shared bilinearly between the
   four nearest cell centres.
3. The 288 values are scaled to unit length, clipped at 0.2 (so one long edge does not
   drown the rest), scaled to unit length again and stored as 288 bytes, the largest 255.

**The distance** is the Euclidean distance between the two unit vectors, divided by √2,
so it lies in `0..=1` and 0 is the same drawing. It is computed from integer dot
products of the stored bytes.

**Why this descriptor, and not the two the story names.**

- *Zernike moments* are rotation-invariant by construction. That is a defect here: the
  difference between a straight and a curved leg, or a flat and an angled terminal, is
  largely the orientation of one stroke, which the moment magnitudes discard. They are
  also global: a change in one corner of the glyph spreads over every coefficient.
- *Shape context* is a set of histograms, one per sampled edge point, compared by an
  assignment between point sets. That is far too slow to rank a 10k library in 150 ms,
  and a single pooled histogram loses the "where" that makes it useful.
- *A plain coverage grid* (the ink of 16 × 16 cells, like the E23.1 fingerprint) was
  measured: 58 of 61 straight-leg `R`s have a straight-leg nearest neighbour (95.1 %). It
  passes, but it compares ink, so weight dominates: a bold and a light `R` of the same
  design are far apart.
- *HOG* compares where the edges are and which way they run. Both edges of a stroke vote
  the same orientation whatever its weight, so a bold and a light straight leg read
  alike, and a straight and a curved leg do not. Measured on the labelled corpus: 60 of
  61 straight-leg `R`s have a straight-leg nearest neighbour (98.4 %; the corpus is 81 %
  straight-leg, so chance is about 81 %). It is one fixed-length vector, so a query is
  one pass over the stored rows.

**Storage.** Table `glyph_descriptor` (schema V24): one row per (code point, face) with
the descriptor's version, the file hash it is of and the bytes, `NULL` for a face that
does not draw the character. Keyed by code point first, so a query reads one range. A row
is current when its version is `similarity::VERSION` and its hash is the file's hash now;
anything else is described again. 288 bytes × 16 glyphs is about 4.6 kB per face, 46 MB
for 10k faces.

**Who describes.**

- The trait pass describes the 16 trait glyphs (the epic's 15 and the `O` the roundness
  detector reads) from the same file read as the traits, and writes them in the same
  transaction. `similarity::STAMP` is one more key of `glyph_trait_stamp`, so the pass's
  resume, cancel and "what is out of date" logic covers the descriptors: a descriptor
  version bump describes every face again and measures no trait; a detector bump leaves
  the descriptors alone.
- `findSimilarGlyph` first lists the faces with no current row for its character,
  describes them off the index lock on the disk threads, writes them, then ranks from the
  stored rows only. For a trait glyph after the pass, the list is empty.

**The API** (additive, rule 6): `SearchService::find_similar_glyph(SimilarGlyphRequest {
face, codePoint, limit })` returns `SimilarGlyphFace { face, distance }`, closest first,
ties by face id. The candidates are the searchable faces with a descriptor of the
character, less the reference face; the other faces of its family stay (a wall sorted by
a reference glyph keeps them). A reference that is unknown or does not draw the character
is `NotFound`; a code point that is no character is `InvalidRequest`. `findSimilar` is
unchanged.

## Consequences

- The bench reports `similar_glyph`: the p95 of 20 results for the `R` and the `g` of 10
  faces, after the trait pass, against a 150 ms budget. The trait pass budget (60 s for
  10k faces) now includes the descriptors.
- The first query of a glyph that is not a trait glyph reads every face's file once; on a
  10k library that takes seconds, not milliseconds. Every later query reads the cache.
  A file that cannot be read is tried again on the next query.
- Any change to the raster, the grid, the orientations or the normalisation bumps
  `similarity::VERSION`; the next pass describes every face again.
- The descriptor is of the default instance of a variable face, as the traits are.
- The CLI, MCP and the UI (E49.9) are adapters over `findSimilarGlyph`; they add no logic.
