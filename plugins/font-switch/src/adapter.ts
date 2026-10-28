// What every host adapter shares: the interface, the two messages, and the
// one lookup Font Switch does (a name in the host's font list; no matching).

import type { FontSwitchPayload } from '@fm/plugin-shared';

/** The design apps Font Switch has an adapter for. */
export type FontSwitchHost = 'indesign' | 'illustrator' | 'photoshop' | 'sketch';

/** What Font Switch applies: the payload of a drag (E34.1) or a PostScript name. */
export type FontRequest = FontSwitchPayload | string;

/** A font as a host lists it. */
export interface HostFont {
  postscriptName?: string;
  family?: string;
  style?: string;
}

/** What applying a font did: the font it applied, or why it did nothing. */
export type FontSwitchResult =
  { applied: true; font: string } | { applied: false; message: string };

/** Applies a font to the current text selection of one host. */
export interface FontSwitchAdapter {
  readonly host: FontSwitchHost;
  apply(font: FontRequest): Promise<FontSwitchResult>;
}

/** The answer when the selection has no text in it. */
export const SELECT_TEXT = 'Select some text first';

/** The answer when the host does not know the font. */
export function notActive(font: FontRequest): FontSwitchResult {
  return { applied: false, message: `${requestName(font)} is not active` };
}

export const selectText = (): FontSwitchResult => ({ applied: false, message: SELECT_TEXT });

/** The name a request is known by: its PostScript name. */
export function requestName(font: FontRequest): string {
  return typeof font === 'string' ? font : font.postscriptName;
}

/** A name the host gave, or nothing for an empty one. */
export function hostName(value: unknown): string | undefined {
  if (value === null || value === undefined) return undefined;
  const s = String(value);
  return s.trim() === '' ? undefined : s;
}

/** How to read a host font's names; each is read only when the lookup needs it. */
export type FontNames<T> = { [K in keyof HostFont]-?: (font: T) => unknown };

/** For fonts that are `HostFont`s already. */
export const HOST_FONT_NAMES: FontNames<HostFont> = {
  postscriptName: (f) => f.postscriptName,
  family: (f) => f.family,
  style: (f) => f.style,
};

/**
 * The host's font for the request: the one with its PostScript name, else
 * (for a payload) the one with its family and style. Names are compared as
 * they are; nothing else is tried.
 */
export function findHostFont<T>(
  fonts: readonly T[],
  names: FontNames<T>,
  font: FontRequest,
): T | null {
  const ps = requestName(font);
  const byName = fonts.find((f) => hostName(names.postscriptName(f)) === ps);
  if (byName !== undefined) return byName;
  if (typeof font === 'string' || !font.family || !font.style) return null;
  const byStyle = fonts.find(
    (f) => hostName(names.family(f)) === font.family && hostName(names.style(f)) === font.style,
  );
  return byStyle ?? null;
}

const HOSTS: Record<string, FontSwitchHost> = {
  indesign: 'indesign',
  idsn: 'indesign',
  illustrator: 'illustrator',
  ilst: 'illustrator',
  photoshop: 'photoshop',
  phsp: 'photoshop',
  sketch: 'sketch',
};

/** The host for an app's name (UXP's `host.name`, a CEP host id); `null` for another app. */
export function hostFromName(name: string | null | undefined): FontSwitchHost | null {
  return HOSTS[(name ?? '').trim().toLowerCase()] ?? null;
}
