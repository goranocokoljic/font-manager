// What a plugin does when a design app opens or closes a document (E29.2,
// E29.3). The host adapters turn the app's events into these calls; the
// matching, the activation and the offer are the app's.

import type { NeededFont } from './api';
import { FmApiError, type FmClient, NOT_RUNNING, NotRunningError } from './client';
import { PANEL_STRINGS } from './panel-strings';

/** An open document as a host adapter names it. */
export interface HostDocument {
  /** Stable while the document is open; sent as the `document` of the calls. */
  key: string;
  /** The name the user knows it by. */
  name: string;
}

/** The plugin's UI in the host. */
export interface PluginUi {
  /** A message that does not wait for the user. */
  notify(message: string): void;
  /** A yes/no question; true for yes. */
  confirm(message: string): Promise<boolean>;
}

/** The question asked when a closed document leaves faces only it used. */
export function deactivateQuestion(count: number, document: string): string {
  const fonts = count === 1 ? '1 font' : `${count} fonts`;
  return `Deactivate ${fonts} used only by ${document}?`;
}

/** The name a plugin shows for a font: its PostScript or full name, else family and style. */
export function fontLabel(font: NeededFont, unknown: string = PANEL_STRINGS.unknownFont): string {
  const named = font.postscriptName ?? font.fullName;
  if (named) return named;
  const family = [font.family, font.style].filter(Boolean).join(' ');
  return family || unknown;
}

/** The one message for the fonts the app does not have: "N fonts missing: A, B". */
export function missingFontsMessage(fonts: NeededFont[]): string {
  const names = [...new Set(fonts.map((f) => fontLabel(f)))];
  return `${PANEL_STRINGS.missing(names.length)}: ${names.join(', ')}`;
}

/** How a `DocumentWatcher` behaves beyond the calls every plugin makes. */
export interface WatcherOptions {
  /**
   * Say `missingFontsMessage` when the app does not have every font of an
   * opened document; for hosts without a missing-fonts panel.
   */
  reportMissing?: boolean;
}

function fontKey(font: NeededFont): string {
  return JSON.stringify([font.postscriptName, font.fullName, font.family, font.style]);
}

/** One per plugin: follows the host's documents and talks to the app. */
export class DocumentWatcher {
  // Missing fonts sent last, per open document key.
  private readonly lastMissing = new Map<string, string>();
  private toldNotRunning = false;
  // Events are handled one after the other, in the order the host sent them.
  private queue: Promise<void> = Promise.resolve();

  constructor(
    private readonly client: FmClient,
    private readonly ui: PluginUi,
    private readonly options: WatcherOptions = {},
  ) {}

  /** A document opened: one `fonts_needed` with all of its fonts. */
  opened(doc: HostDocument, fonts: NeededFont[]): Promise<void> {
    return this.run(async () => {
      this.lastMissing.set(doc.key, '');
      if (fonts.length === 0) return;
      const { unmatched } = await this.reached(
        this.client.fontsNeeded({ fonts, document: doc.key }),
      );
      if (this.options.reportMissing && unmatched.length > 0) {
        this.ui.notify(missingFontsMessage(unmatched));
      }
    });
  }

  /**
   * The host reports these fonts of an open document missing: they are sent
   * again, unless the same fonts were the last ones sent as missing.
   */
  missing(doc: HostDocument, fonts: NeededFont[]): Promise<void> {
    return this.run(async () => {
      const keys = [...new Set(fonts.map(fontKey))].sort().join('\n');
      if (keys === (this.lastMissing.get(doc.key) ?? '')) return;
      // Recorded only once sent, so a call that failed is tried again.
      if (fonts.length > 0) {
        await this.reached(this.client.fontsNeeded({ fonts, document: doc.key }));
      }
      this.lastMissing.set(doc.key, keys);
    });
  }

  /** A document closed: `document_closed`, then the offer as a question. */
  closed(doc: HostDocument): Promise<void> {
    return this.run(async () => {
      this.lastMissing.delete(doc.key);
      const { offer } = await this.reached(this.client.documentClosed({ document: doc.key }));
      if (offer.length === 0) return;
      if (!(await this.ui.confirm(deactivateQuestion(offer.length, doc.name)))) return;
      await this.reached(this.client.deactivate({ target: { kind: 'faces', faces: offer } }));
    });
  }

  private run(work: () => Promise<void>): Promise<void> {
    const next = this.queue.then(async () => {
      try {
        await work();
      } catch (e) {
        this.report(e);
      }
    });
    this.queue = next;
    return next;
  }

  // The app answered: the next "not running" is news again. Only a call
  // that got through counts; an event that made no call changes nothing.
  private async reached<T>(call: Promise<T>): Promise<T> {
    const answer = await call;
    this.toldNotRunning = false;
    return answer;
  }

  // Never throws into the host's event loop. "Not running" is said once,
  // until a call gets through again.
  private report(e: unknown): void {
    if (e instanceof NotRunningError) {
      if (this.toldNotRunning) return;
      this.toldNotRunning = true;
      this.ui.notify(NOT_RUNNING);
    } else if (e instanceof FmApiError) {
      this.toldNotRunning = false;
      this.ui.notify(e.message);
    } else {
      this.ui.notify(`Font Manager: ${String(e)}`);
    }
  }
}
