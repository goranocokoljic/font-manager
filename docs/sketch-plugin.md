# The Sketch plugin

The Font Fu plugin for Sketch activates the fonts a Sketch document needs when you
open it. It works with Sketch 70 and later on macOS, and it needs the Font Fu app.

## What it does

- **When you open a document**, the plugin tells Font Fu which fonts the document
  uses. Font Fu finds them in your library and activates them.
- **When some fonts are not in your library**, the plugin shows one message:
  "N fonts missing", with their names.
- **When you close a document**, Font Fu can offer to deactivate the fonts that
  only this document used. Click **Deactivate** to deactivate them, or **Keep** to keep
  them active.
- **When Font Fu is not running**, the plugin says so once and does nothing else.

The plugin talks only to the Font Fu app on your own computer (`127.0.0.1`). It
does not connect to the internet, and it has no update feed of its own: a new version
comes with each Font Fu release.

## Install the plugin

1. Open the Font Fu release on GitHub and download
   `font-fu_<version>_sketch-plugin.zip`.
2. Unzip it. You get `font-manager.sketchplugin`.
3. Double-click `font-manager.sketchplugin`. Sketch installs it.
4. Start Font Fu, then open a document in Sketch.

To see the plugin, open Sketch, then **Plugins › Manage Plugins…**. It is listed as
**Font Fu**.

To check the download, compare its SHA256 with the `SHA256SUMS` file of the release
(see [Building Font Fu from source](building.md), "Verify a download").

## Update or remove the plugin

- **Update:** install the zip of the new release in the same way. Sketch replaces the
  old version.
- **Remove:** in Sketch, open **Plugins › Manage Plugins…**, select **Font Fu**,
  and uninstall it. The fonts that Font Fu activated stay active until you
  deactivate them in Font Fu.

## Build it yourself

```
pnpm install
pnpm --filter @fm/sketch-plugin build
```

The build writes `plugins/sketch/dist/font-manager.sketchplugin` and
`plugins/sketch/dist/font-manager.sketchplugin.zip`. Two builds of the same commit give
a byte-identical zip.
