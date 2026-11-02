// Every text the missing-fonts panel shows (E30.4), in one place so a host
// or a translation can swap them.

import { NOT_RUNNING } from './client';

export const PANEL_STRINGS = {
  title: 'Missing fonts',
  listLabel: 'Missing fonts of the active document',
  noDocument: 'No document is open.',
  checking: 'Checking the fonts of the document…',
  noneMissing: 'No fonts missing',
  missing: (count: number): string => (count === 1 ? '1 font missing' : `${count} fonts missing`),
  inLibrary: 'In your library',
  conflict: 'Conflicts with an installed font',
  notInLibrary: 'Not in your library',
  activate: 'Activate',
  activateFont: (font: string): string => `Activate ${font}`,
  activateAll: 'Activate all',
  conflictQuestion: (count: number, font: string): string =>
    count === 1
      ? `${font} conflicts with an installed font. Keep the installed font, or replace it for this session?`
      : `${count} fonts conflict with installed fonts. Keep the installed fonts, or replace them for this session?`,
  keep: 'Keep installed',
  replace: 'Replace',
  notActivated: (font: string, reason: string): string => `${font} was not activated: ${reason}`,
  stillConflicts: (font: string): string =>
    `${font} was not activated: it conflicts with an installed font.`,
  downloaded: (count: number, catalog: string, families: string[]): string =>
    `Downloaded ${count === 1 ? '1 family' : `${count} families`} from ${catalog}: ${families.join(', ')}`,
  catalogNames: { googleFonts: 'Google Fonts', fontshare: 'Fontshare' } as Record<string, string>,
  notRunning: NOT_RUNNING,
  unknownFont: 'Unnamed font',
};

export type PanelStrings = typeof PANEL_STRINGS;
