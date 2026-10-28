// What a drop (or the clipboard) carries, read into a font request, and the
// drop target a plugin panel puts on its document: the font goes to the
// adapter of the app the panel runs in.

import type { FontSwitchPayload } from '@fm/plugin-shared';
import type { FontRequest, FontSwitchAdapter, FontSwitchHost, FontSwitchResult } from './adapter';

/** The `DataTransfer` type of the JSON `FontSwitchPayload` (ADR 0017). */
export const FONT_SWITCH_MIME = 'application/x-fm-font-switch';

/** The answer when the panel runs in an app Font Switch has no adapter for. */
export const NO_ADAPTER = 'Font Switch does not work in this app';

/** The answer when the clipboard holds no font. */
export const COPY_A_FONT = 'Copy a font in Font Manager first (Copy for Font Switch)';

/** The part of a `DataTransfer` the drop reads. */
export interface DropData {
  getData(type: string): string;
}

/** The part of a drop event the handler uses. */
export interface FontDropEvent {
  dataTransfer?: DropData | null;
  preventDefault(): void;
}

/** Where the panel's drops land: its document. */
export interface DropTarget {
  addEventListener(type: 'dragover' | 'drop', listener: (event: FontDropEvent) => void): void;
}

/** The app the code runs in; `null` for one Font Switch does not know. */
export type HostDetector = () => FontSwitchHost | null;

/** The adapters a plugin has, by host; a host it does not run in has none. */
export type FontSwitchAdapters = Partial<Record<FontSwitchHost, FontSwitchAdapter | null>>;

const isText = (v: unknown): v is string => typeof v === 'string' && v.trim() !== '';

/** The payload in `text`, or `null` when it is not one. */
export function payloadFromJson(text: string): FontSwitchPayload | null {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return null;
  }
  const p = value as Partial<FontSwitchPayload> | null;
  if (typeof p !== 'object' || p === null || !isText(p.postscriptName)) return null;
  return {
    postscriptName: p.postscriptName.trim(),
    family: typeof p.family === 'string' ? p.family : '',
    style: typeof p.style === 'string' ? p.style : '',
    fileUrl: typeof p.fileUrl === 'string' ? p.fileUrl : '',
  };
}

/**
 * The font in a text: a payload as JSON (what "Copy for Font Switch" puts on
 * the clipboard), else a PostScript name (one word, as PostScript names are).
 */
export function fontFromText(text: string | null | undefined): FontRequest | null {
  const trimmed = (text ?? '').trim();
  if (trimmed === '') return null;
  const payload = payloadFromJson(trimmed);
  if (payload) return payload;
  return /\s/.test(trimmed) ? null : trimmed;
}

function read(data: DropData, type: string): string {
  try {
    return data.getData(type);
  } catch {
    // A host that does not carry the type throws instead of answering "".
    return '';
  }
}

/** The font a drop carries: the JSON payload first, else the plain text. */
export function fontFromDrop(data: DropData | null | undefined): FontRequest | null {
  if (!data) return null;
  return payloadFromJson(read(data, FONT_SWITCH_MIME)) ?? fontFromText(read(data, 'text/plain'));
}

async function applyTo(
  font: FontRequest,
  detect: HostDetector,
  adapters: FontSwitchAdapters,
): Promise<FontSwitchResult> {
  const host = detect();
  const adapter = host ? adapters[host] : null;
  if (!adapter) return { applied: false, message: NO_ADAPTER };
  return adapter.apply(font);
}

/**
 * Applies the font a drop carries with the adapter of the running host;
 * `null` (and the drop left alone) when the drop carries no font.
 */
export async function handleFontDrop(
  event: FontDropEvent,
  detect: HostDetector,
  adapters: FontSwitchAdapters,
): Promise<FontSwitchResult | null> {
  const font = fontFromDrop(event.dataTransfer);
  if (!font) return null;
  event.preventDefault();
  return applyTo(font, detect, adapters);
}

/** Applies the font copied as text (the clipboard) with the adapter of the running host. */
export async function applyCopiedFont(
  text: string | null | undefined,
  detect: HostDetector,
  adapters: FontSwitchAdapters,
): Promise<FontSwitchResult> {
  const font = fontFromText(text);
  if (!font) return { applied: false, message: COPY_A_FONT };
  return applyTo(font, detect, adapters);
}

/** The words for a result the user must see; `null` when the font was applied. */
export function failureMessage(result: FontSwitchResult | null): string | null {
  return result && !result.applied ? result.message : null;
}

/**
 * Makes `target` take font drops: each is applied with the adapter of the
 * running host, and `report` gets what went wrong (nothing on success).
 */
export function fontSwitchDropTarget(
  target: DropTarget,
  detect: HostDetector,
  adapters: FontSwitchAdapters,
  report: (message: string) => void,
): void {
  // Without this the webview refuses the drop.
  target.addEventListener('dragover', (event) => event.preventDefault());
  target.addEventListener('drop', (event) => {
    handleFontDrop(event, detect, adapters).then(
      (result) => {
        const message = failureMessage(result);
        if (message) report(message);
      },
      (e: unknown) => report(`Font Switch failed: ${String(e)}`),
    );
  });
}
