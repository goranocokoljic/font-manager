// Photoshop (UXP `photoshop` module): the selected text layers get the font
// by its PostScript name, inside one modal step as Photoshop requires for a
// change to a document.

import {
  type FontNames,
  type FontSwitchAdapter,
  findHostFont,
  hostName,
  notActive,
  requestName,
  selectText,
} from './adapter';

/** A font of `app.fonts`, the properties the adapter reads. */
export interface PhotoshopFont {
  postScriptName?: unknown;
  family?: unknown;
  style?: unknown;
}

/** A layer of the selection; only `kind: 'text'` ones have a font. */
export interface PhotoshopSelectedLayer {
  kind?: string;
  textItem?: { characterStyle?: { font?: unknown } | null } | null;
}

/** The part of the `photoshop` module the adapter uses. */
export interface PhotoshopFontSwitchModule {
  app: {
    activeDocument?: { activeLayers?: readonly PhotoshopSelectedLayer[] | null } | null;
    fonts: readonly PhotoshopFont[];
  };
  core: {
    executeAsModal(run: () => Promise<void>, options: { commandName: string }): Promise<unknown>;
  };
}

const NAMES: FontNames<PhotoshopFont> = {
  postscriptName: (f) => f.postScriptName,
  family: (f) => f.family,
  style: (f) => f.style,
};

export function photoshopAdapter(photoshop: PhotoshopFontSwitchModule): FontSwitchAdapter {
  const { app, core } = photoshop;
  return {
    host: 'photoshop',
    async apply(font) {
      const layers = (app.activeDocument?.activeLayers ?? []).filter(
        (l) => l.kind === 'text' && l.textItem?.characterStyle,
      );
      if (layers.length === 0) return selectText();
      const found = findHostFont(app.fonts, NAMES, font);
      const name = found && hostName(found.postScriptName);
      if (!name) return notActive(font);
      await core.executeAsModal(
        async () => {
          for (const layer of layers) {
            const style = layer.textItem?.characterStyle;
            if (style) style.font = name;
          }
        },
        { commandName: 'Font Switch' },
      );
      return { applied: true, font: requestName(font) };
    },
  };
}
