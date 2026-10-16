# fixtures/fonts

The 200-font test corpus (OFL, Apache and UFL licences only), picked from the reference
library (`scripts/build-reference-library.sh`) by `scripts/reference-library/fixtures.mjs`.
Tests read fonts only from here.

`manifest.tsv` lists every font: sha256, size, licence, kind, path and origin.
`scripts/reference-library/fixtures.test.mjs` checks that the files match it.

| Folder | What |
|--------|------|
| `google-repo/<family>/` | fonts from the google/fonts repository (static, variable, color) |
| `google-static/<family>/` | static instances Google Fonts serves for variable families |
| `fontshare/<family>/` | Fontshare OFL families: OTF, TTF, WOFF, WOFF2 |
| `derived/<family>/` | a TTC packed from two faces of the family, and a truncated (broken) copy |
| `type1/inconsolata/` | Inconsolata zi4 Type 1 (PFB) from CTAN |

Each folder holds the licence file of its family. The derived files come from a family
without a Reserved Font Name; their table data is unchanged (TTC) or cut off (broken).
