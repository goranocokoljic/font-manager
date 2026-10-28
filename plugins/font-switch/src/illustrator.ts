// Illustrator (ExtendScript, reached through CEP's `evalScript`): the panel
// cannot touch Illustrator's objects, so `ILLUSTRATOR_JSX` reads the
// selection and the fonts there and applies the font this side found.

import { type ExtendScript, scriptCall } from '@fm/plugin-shared';
import {
  type FontSwitchAdapter,
  HOST_FONT_NAMES,
  type HostFont,
  findHostFont,
  notActive,
  requestName,
  selectText,
} from './adapter';
import { ILLUSTRATOR_JSX } from './illustrator-jsx';

/** `fmFontSwitchFonts`' lines as fonts. */
export function illustratorFontList(text: string): HostFont[] {
  return text
    .split('\n')
    .filter((line) => line !== '')
    .map((line) => {
      const [postscriptName, family, style] = line.split('\t');
      return { postscriptName, family, style };
    });
}

export function illustratorAdapter(script: ExtendScript): FontSwitchAdapter {
  let loaded: Promise<unknown> | null = null;
  const load = () => {
    // Loaded again on the next drop when loading failed.
    loaded ??= script.evalScript(ILLUSTRATOR_JSX).catch((e: unknown) => {
      loaded = null;
      throw e;
    });
    return loaded;
  };
  return {
    host: 'illustrator',
    async apply(font) {
      await load();
      if ((await script.evalScript('fmFontSwitchHasText()')) !== 'yes') return selectText();
      const fonts = illustratorFontList(await script.evalScript('fmFontSwitchFonts()'));
      const found = findHostFont(fonts, HOST_FONT_NAMES, font);
      if (!found?.postscriptName) return notActive(font);
      const done = await script.evalScript(scriptCall('fmFontSwitchApply', found.postscriptName));
      // The selection went away while the fonts were read.
      if (done !== 'yes') return selectText();
      return { applied: true, font: requestName(font) };
    },
  };
}
