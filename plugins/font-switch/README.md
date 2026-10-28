# @fm/font-switch

Font Switch (E34) in the design apps: a font dragged from Font Manager (or
copied with "Copy for Font Switch") is applied to the text selection of the
app. One adapter per host, behind one interface:

```ts
interface FontSwitchAdapter {
  readonly host: 'indesign' | 'illustrator' | 'photoshop' | 'sketch';
  apply(font: FontSwitchPayload | string): Promise<FontSwitchResult>;
}
```

`font` is the `FontSwitchPayload` of the drag (E34.1, ADR 0017) or a plain
PostScript name. The adapter looks the font up in the host's own font list:
the PostScript name first, then (for a payload) the family and style, each
compared as it is. Nothing else is matched; the app does the matching when it
activates the face.

| Host              | Adapter                            | Text selection                                     | The font is set through                                                                                                           |
| ----------------- | ---------------------------------- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| InDesign (UXP)    | `inDesignAdapter(app)`             | selected text, or the text of selected text frames | `appliedFont` = the `Font` of `app.fonts`                                                                                         |
| Photoshop (UXP)   | `photoshopAdapter(photoshop)`      | selected text layers                               | `characterStyle.font` = PostScript name, in one `executeAsModal` step                                                             |
| Illustrator (CEP) | `illustratorAdapter(extendScript)` | selected text, or the text of selected text frames | `characterAttributes.textFont` = `app.textFonts.getByName`, in ExtendScript (`ILLUSTRATOR_JSX`, loaded once through `evalScript`) |
| Sketch            | `sketchAdapter(host)`              | selected text layers                               | the native `setFontPostscriptName:`                                                                                               |

What `apply` answers:

- the font applied: `{ applied: true, font }`;
- no selection, or a selection with no text in it (an insertion point, a
  shape, a pixel layer): nothing changes, `"Select some text first"`;
- a font the host does not list: nothing changes, `"<name> is not active"`.

## Drops and the clipboard

- `fontFromDrop(dataTransfer)` reads `application/x-fm-font-switch` (the JSON
  payload) first, else `text/plain` (a PostScript name: one word).
- `handleFontDrop(event, detect, adapters)` applies it with the adapter of the
  host `detect()` names (`hostFromName` maps UXP's `host.name` and the CEP
  host ids); a drop that carries no font is left alone.
- `fontSwitchDropTarget(document, detect, adapters, report)` makes a panel a
  drop target; `report` gets only what went wrong.
- `applyCopiedFont(text, detect, adapters)` does the same for text from the
  clipboard, for a host without a panel.

## Who uses it

The plugins import this package (`"@fm/font-switch": "workspace:*"`); none of
them copies an adapter.

- `plugins/adobe-uxp/` (E30.1): its panel is a drop target, with
  `inDesignAdapter` in InDesign and `photoshopAdapter` in Photoshop.
- `plugins/sketch/` (E31.1): Sketch has no panel to drop on, so the menu
  command "Plugins › Font Manager › Apply Copied Font" applies the font on
  the clipboard (what "Copy for Font Switch" puts there) with `sketchAdapter`.
- `plugins/adobe-cep-illustrator/` (E30.2): the extension is hidden and has
  no panel, so nothing can be dropped on it yet. `illustratorAdapter` is ready
  for the panel the move to UXP brings (see that README); the extension's
  ExtendScript helpers (`scriptCall`) and this adapter share
  `@fm/plugin-shared`'s.

The host collections (`elements`) and the ExtendScript call helpers come from
`@fm/plugin-shared`.

## Checks

`pnpm lint` and `pnpm test` cover the package with the other workspace
packages: `tsc --noEmit` and Vitest against mocked host APIs
(`test/*.test.ts`; the Illustrator test runs the real ExtendScript against a
fake `app` and checks it is ES3).
