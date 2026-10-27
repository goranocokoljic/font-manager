# Font Manager for Sketch

A Sketch plugin (Sketch 70 and later, macOS). It has no font logic: it tells
the running Font Manager app which fonts a document uses, and the app finds
and activates them (E29.2). It does what the Adobe plugins
(`plugins/adobe-uxp/`, `plugins/adobe-cep-illustrator/`) do, through the
same `@fm/plugin-shared` code.

- **Open.** On Sketch's `OpenDocument` action the plugin collects the font of
  every text layer on every page (`sketch/dom` `find('Text', document)`: the
  layer's PostScript name and font family) and sends them to `fonts_needed`
  in one call, with `Sketch: <document id>` as the document.
- **Missing fonts.** When the app does not have some of them, it shows one
  message: "N fonts missing: <names>".
- **Close.** On `CloseDocument` it calls `document_closed`. When the app
  offers fonts only that document used, it asks "Deactivate N fonts used only
  by <document>?"; Deactivate calls `deactivate` with those faces, Keep
  leaves them active.
- **App not running.** It says "Font Manager is not running" once and does
  nothing else, until the app answers again. Sketch runs every action in a
  fresh script, so that "once" is kept in a Sketch session variable.

Sketch gives plugins no pipe or socket, so the plugin uses the local API
(ADR 0015): it reads `endpoint.json` from the app's data folder on every
call, sends the bearer token, and talks only to `127.0.0.1`.

A text layer with several fonts in it is sent by the font Sketch reports for
the layer.

## Build and install

```
pnpm --filter @fm/sketch build
```

The plugin is `dist/FontManager.sketchplugin`. Double-click it to install it
into Sketch.

## Tests

`pnpm --filter @fm/sketch test` runs the plugin against a fake `sketch`
module (`test/mock-sketch.ts`) and a fake app endpoint
(`@fm/plugin-shared/testing`). They run in `pnpm test` on all three CI
platforms.
