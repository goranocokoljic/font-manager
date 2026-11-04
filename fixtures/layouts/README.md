# Layout goldens

Chromium renders of the six layout presets on an A4 page at 1×, set in
`fixtures/fonts/google-static/dmsans/DMSans_36pt-Regular.ttf` (OFL). The core's page
renderer must stay within 2 % of each one (E45.5, `crates/fm-core/src/preview/page/tests.rs`).

`presets.json` is what the images were made from; a test fails when it no longer matches the
core's presets. To make them again:

```
cargo run -p fm-core --example layout_page -- presets > fixtures/layouts/presets.json
node apps/desktop/scripts/layout-goldens.mjs
```

The script copies the block and page CSS of `LayoutBlock.vue` and `FontLayout.vue`. Change it
when those change.
