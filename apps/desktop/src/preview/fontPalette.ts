// CSS `font-palette` values for color faces (E16.3). The webview draws a COLR
// face in another CPAL palette when an `@font-palette-values` rule names the
// face's `font-family` and the palette as `base-palette`; a row then sets
// `font-palette` to the rule's name. One rule per face and palette, added to
// one shared style sheet the first time a row asks for it.
import type { FaceId } from '../api/types';
import { faceFamily } from './faceLoader';

/** Where the rules go; tests pass a fake. */
export interface PaletteSheet {
  insertRule(rule: string, index?: number): number;
}

let sheet: PaletteSheet | null | undefined;
const added = new Set<string>();

function sharedSheet(): PaletteSheet | null {
  if (sheet !== undefined) return sheet;
  if (typeof document === 'undefined') return (sheet = null);
  const style = document.createElement('style');
  style.id = 'fm-font-palettes';
  document.head.append(style);
  return (sheet = style.sheet);
}

/** The `@font-palette-values` name of palette `index` of `face`. */
export function paletteName(face: FaceId, index: number): string {
  return `--fm-palette-${face}-${index}`;
}

/**
 * The CSS `font-palette` that draws `face` in palette `index`, adding its rule
 * once. Palette 0 is the font's default: `undefined`, nothing to add. A webview
 * that does not know the rule still answers the name; the face then keeps its
 * default palette.
 */
export function fontPalette(face: FaceId, index: number | null | undefined): string | undefined {
  if (!index || index < 0 || !Number.isInteger(index)) return undefined;
  const name = paletteName(face, index);
  if (!added.has(name)) {
    added.add(name);
    try {
      sharedSheet()?.insertRule(
        `@font-palette-values ${name} { font-family: "${faceFamily(face)}"; base-palette: ${index}; }`,
      );
    } catch {
      // Not supported here (jsdom): the face keeps its default palette.
    }
  }
  return name;
}

/** Replaces the shared sheet and forgets the rules added (tests). */
export function setPaletteSheet(next: PaletteSheet | null | undefined): void {
  sheet = next;
  added.clear();
}
