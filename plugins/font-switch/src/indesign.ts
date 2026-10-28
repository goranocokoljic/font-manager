// InDesign (UXP `indesign` module): the selected text, or the text of the
// selected text frames, gets the `Font` object of `app.fonts`.

import { type HostCollection, elements } from '@fm/plugin-shared';
import {
  type FontNames,
  type FontSwitchAdapter,
  findHostFont,
  notActive,
  requestName,
  selectText,
} from './adapter';

/** A font of `app.fonts`, the properties the adapter reads. */
export interface InDesignFont {
  postscriptName?: unknown;
  fontFamily?: unknown;
  fontStyleName?: unknown;
}

/** Text that takes a font. */
export interface InDesignText {
  appliedFont?: unknown;
}

/** A selected object: text, a text frame, or anything else. */
export interface InDesignSelected extends InDesignText {
  constructorName?: string;
  texts?: HostCollection<InDesignText>;
}

/** The part of the `indesign` module's `app` the adapter uses. */
export interface InDesignFontSwitchApp {
  /** Empty when nothing is selected or no document is open. */
  selection?: HostCollection<InDesignSelected> | null;
  fonts: HostCollection<InDesignFont>;
}

/** Selected text; an insertion point is no text. */
const TEXT = new Set([
  'Character',
  'Word',
  'TextStyleRange',
  'Line',
  'Paragraph',
  'TextColumn',
  'Text',
  'Story',
]);
/** Frames whose text the font goes to. */
const FRAMES = new Set(['TextFrame', 'TextPath']);

const NAMES: FontNames<InDesignFont> = {
  postscriptName: (f) => f.postscriptName,
  family: (f) => f.fontFamily,
  style: (f) => f.fontStyleName,
};

function textOf(selection: InDesignFontSwitchApp['selection']): InDesignText[] {
  return elements(selection).flatMap((item) => {
    const kind = item.constructorName ?? '';
    if (TEXT.has(kind)) return [item];
    if (FRAMES.has(kind)) return elements(item.texts);
    return [];
  });
}

export function inDesignAdapter(app: InDesignFontSwitchApp): FontSwitchAdapter {
  return {
    host: 'indesign',
    async apply(font) {
      const targets = textOf(app.selection);
      if (targets.length === 0) return selectText();
      const found = findHostFont(elements(app.fonts), NAMES, font);
      if (!found) return notActive(font);
      for (const text of targets) text.appliedFont = found;
      return { applied: true, font: requestName(font) };
    },
  };
}
