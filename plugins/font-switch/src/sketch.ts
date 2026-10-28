// Sketch: the selected text layers get the font by its PostScript name
// through the native `MSTextLayer`. Sketch's JavaScript API has no font list;
// the plugin's wiring gives one (from `NSFontManager`).

import {
  type FontSwitchAdapter,
  HOST_FONT_NAMES,
  type HostFont,
  findHostFont,
  notActive,
  requestName,
  selectText,
} from './adapter';

/** A selected layer of `sketch/dom`; only `type: 'Text'` ones have a font. */
export interface SketchSelectedLayer {
  type?: string;
  /** The native `MSTextLayer`. */
  sketchObject?: { setFontPostscriptName?(name: string): void } | null;
}

/** What the adapter needs from Sketch. */
export interface SketchFontSwitchHost {
  /** The selected layers of the front document; none without a document. */
  selectedLayers(): readonly SketchSelectedLayer[];
  /** The fonts the Mac has. */
  fonts(): readonly HostFont[];
}

export function sketchAdapter(host: SketchFontSwitchHost): FontSwitchAdapter {
  return {
    host: 'sketch',
    async apply(font) {
      const layers = host
        .selectedLayers()
        .filter((l) => l.type === 'Text' && l.sketchObject?.setFontPostscriptName);
      if (layers.length === 0) return selectText();
      const found = findHostFont(host.fonts(), HOST_FONT_NAMES, font);
      if (!found?.postscriptName) return notActive(font);
      for (const layer of layers) layer.sketchObject?.setFontPostscriptName?.(found.postscriptName);
      return { applied: true, font: requestName(font) };
    },
  };
}
