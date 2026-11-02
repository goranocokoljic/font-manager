# Document fixtures (E32.1)

Small design documents for the `fm-core` `document_fonts` tests. Every file is
written by [`scripts/build-document-fixtures.mjs`](../../scripts/build-document-fixtures.mjs)
from the published structure of its format, with Node's built-in `zlib` only.
No application made them and no third-party file is copied. Re-create them
with:

```
node scripts/build-document-fixtures.mjs
```

The tests in `crates/fm-core/src/document_fonts/tests.rs` check the exact font
list of each file.

| File | Format and structure | Fonts it names |
|------|----------------------|----------------|
| `sample.docx` | Zip (stored and deflated entries). `word/fontTable.xml` (`w:font`), `w:rFonts` in `word/document.xml` and `word/styles.xml`, a theme that no run names | Arial, Calibri, Cambria, Georgia, Inter, Noto Sans JP (families) |
| `sample.pptx` | Zip. A slide → layout → master → `theme1.xml` relationship chain; the slide and master use `a:latin`/`a:ea`/`a:cs` with `+mj-*`/`+mn-*` placeholders; `theme2.xml` is used by no master | Montserrat, Noto Sans KR, Open Sans, Roboto (families) |
| `sample.sketch` | Zip with `document.json`, `meta.json` and two `pages/*.json`; text layers carry `MSAttributedStringFontAttribute.attributes.name` in runs and text styles, one nested two groups deep | Inter-Bold, Inter-Regular, SFProText-Semibold (PostScript) |
| `sample.fig` | Zip with `canvas.fig`: `fig-kiwi`, version 48, a raw-deflated kiwi schema (`NodeType`, `FontName`, `Vector`, `NodeChange`, `Message`) and a raw-deflated `Message` of five node changes (one duplicate, one with an empty PostScript name) | Inter Bold / Inter-Bold, Inter Regular / Inter-Regular, Roboto Mono Regular |
| `zstd.fig` | A bare `canvas.fig` whose data chunk starts with the zstd frame magic `28 B5 2F FD` | — (Unsupported) |
| `sample.indd` | The layout of the Adobe XMP Toolkit `InDesign_Handler`: two 4 KB master pages (GUID `0606EDF5-D81D-46E5-BD31-EFE7FE74B71D`, `DOCUMENT`, sequence numbers 7 and 8, 3 database pages), one filler database page, then two contiguous objects between header and trailer GUIDs: the XMP packet (`xmpTPg:Fonts` with one `rdf:li` in element form and two in attribute form, one with `&amp;`, plus `xmpTPg:PlateNames`) and a filler object | MinionPro-Regular, MyriadPro-Bold, Brand&Co-Sans (with family and face) |
| `sample.ai` | PDF 1.6: catalog, an XMP metadata stream, a Type 1 font `/ABCDEF+MinionPro-Regular` (subset tag), a TrueType font `/Source#20Sans#20Pro` (name escape) and a Flate `/ObjStm` holding `/BaseFont /Lato-Bold`; ends with `%%EOF` | Minion Pro Regular, Source Sans Pro, Lato-Bold |
| `encrypted.ai` | `sample.ai` with `/Encrypt` in the trailer | — (Unsupported) |
| `encrypted.docx` | An OLE compound file header, which is what a password-protected Office file is | — (Unsupported) |
| `sample.psd` | Photoshop v1, 1×1 RGB 8-bit. Three layer records: two with a `TySh` block (version 1, transform, text version 50, descriptor 16, `EngineData` `tdta` with `/EngineDict /StyleRun /RunArray`, `/StyleSheetSet`, and the `/FontSet` of UTF-16BE `/Name` strings, `\`-escaped, in `/ResourceDict` and `/DocumentResources`), one plain. Both font sets also hold `MyriadPro-Regular`, which no run uses. *Title* has runs in ArialMT, Inter-Bold and ArialMT, and a default style sheet in MyriadPro-Regular. *Body* has a run in Inter-Bold and a run without its own font, which takes the default style sheet that `/TheNormalStyleSheet 1` names (Georgia; the first sheet is MyriadPro-Regular); then an empty global layer mask and three global blocks (`Patt`, `CAI `, `cinf`) whose lengths are 0, 77 and 10 bytes with the data padded to 4, as Photoshop 2025 writes them; raw image data | ArialMT, Georgia, Inter-Bold: the fonts the runs use (`AdobeInvisFont` and the unused `MyriadPro-Regular` are left out) |
