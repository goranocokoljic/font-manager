// Font Switch (E34) in Sketch. Sketch gives a plugin without a panel nothing
// to drop a font on, so the "Apply Copied Font" command applies what the
// app's "Copy for Font Switch" put on the clipboard (ADR 0017) to the
// selected text layers, through `@fm/font-switch`'s Sketch adapter.

import {
  type HostFont,
  type SketchFontSwitchHost,
  applyCopiedFont,
  sketchAdapter,
} from '@fm/font-switch';

/** A Cocoa `NSArray` as Sketch's script runtime gives it. */
export interface NativeArray<T> {
  count(): number;
  objectAtIndex(index: number): T;
}

/** The part of `NSFontManager.sharedFontManager()` the command reads. */
export interface NativeFontManager {
  availableFontFamilies(): NativeArray<unknown>;
  /** Each member: PostScript name, style name, weight, traits. */
  availableMembersOfFontFamily(family: unknown): NativeArray<NativeArray<unknown>> | null;
}

function each<T>(array: NativeArray<T> | null | undefined): T[] {
  if (!array) return [];
  return Array.from({ length: Number(array.count()) }, (_, i) => array.objectAtIndex(i));
}

/** The fonts the Mac has, as the adapter looks them up. */
export function nativeFonts(manager: NativeFontManager): HostFont[] {
  return each(manager.availableFontFamilies()).flatMap((family) =>
    each(manager.availableMembersOfFontFamily(family)).map((member) => ({
      postscriptName: String(member.objectAtIndex(0)),
      family: String(family),
      style: String(member.objectAtIndex(1)),
    })),
  );
}

/** The "Apply Copied Font" command: the font on the clipboard to the selected text. */
export function applyCopiedFontCommand(
  host: SketchFontSwitchHost,
  clipboardText: () => string | null,
  message: (text: string) => void,
): () => Promise<void> {
  const adapters = { sketch: sketchAdapter(host) };
  return async () => {
    try {
      const result = await applyCopiedFont(clipboardText(), () => 'sketch', adapters);
      message(result.applied ? `${result.font} applied` : result.message);
    } catch (e) {
      // A Sketch API that threw; never into Sketch's event loop.
      message(`Font Manager: ${String(e)}`);
    }
  };
}
