// The InDesign side: `afterOpen`, `afterActivate` and `beforeClose` of the
// UXP `indesign` module, turned into DocumentWatcher calls.

import type { DocumentWatcher, HostDocument, NeededFont } from '@fm/plugin-shared';
import { elements, type HostCollection, text } from './collection';

/** A font of a document, the properties the plugin reads. */
export interface InDesignFont {
  postscriptName?: string;
  fontFamily?: string;
  fontStyleName?: string;
  status?: unknown;
}

export interface InDesignDocument {
  id: number;
  name: string;
  fonts: HostCollection<InDesignFont>;
}

/** An event's target: a document, or a layout window whose parent is one. */
export interface InDesignTarget {
  fonts?: HostCollection<InDesignFont>;
  parent?: unknown;
}

export interface InDesignEvent {
  target?: InDesignTarget | null;
}

/** The part of the `indesign` module the plugin uses. */
export interface InDesignModule {
  app: {
    documents: HostCollection<InDesignDocument>;
    addEventListener(type: string, handler: (event: InDesignEvent) => void): unknown;
  };
  FontStatus: { INSTALLED: unknown };
}

function isDocument(target: unknown): target is InDesignDocument {
  return typeof target === 'object' && target !== null && 'fonts' in target;
}

/** The document's fonts, as `fonts_needed` takes them. */
export function inDesignFonts(doc: InDesignDocument): NeededFont[] {
  return elements(doc.fonts).map(neededFont);
}

/** The fonts InDesign reports as not installed. */
export function inDesignMissingFonts(doc: InDesignDocument, installed: unknown): NeededFont[] {
  return elements(doc.fonts)
    .filter((f) => f.status !== installed)
    .map(neededFont);
}

function neededFont(f: InDesignFont): NeededFont {
  return {
    postscriptName: text(f.postscriptName),
    family: text(f.fontFamily),
    style: text(f.fontStyleName),
  };
}

/** Follows InDesign's documents until the plugin unloads. */
export function watchInDesign(indesign: InDesignModule, watcher: DocumentWatcher): void {
  const { app, FontStatus } = indesign;
  const open = new Map<number, HostDocument>();

  const opened = (doc: InDesignDocument) => {
    // `afterOpen` comes for the document and again for its layout window.
    if (open.has(doc.id)) return;
    const host = { key: `InDesign: ${doc.name} #${doc.id}`, name: doc.name };
    open.set(doc.id, host);
    void watcher.opened(host, inDesignFonts(doc));
  };

  app.addEventListener('afterOpen', (event) => {
    const target = event.target;
    const doc = isDocument(target) ? target : target?.parent;
    if (isDocument(doc)) opened(doc);
  });
  app.addEventListener('afterActivate', (event) => {
    const doc = event.target;
    if (!isDocument(doc)) return;
    const host = open.get(doc.id);
    if (host) void watcher.missing(host, inDesignMissingFonts(doc, FontStatus.INSTALLED));
  });
  app.addEventListener('beforeClose', (event) => {
    // Closing one layout window of several does not close the document.
    const doc = event.target;
    if (!isDocument(doc)) return;
    const host = open.get(doc.id);
    if (!host) return;
    open.delete(doc.id);
    void watcher.closed(host);
  });

  // Documents open before the plugin loaded.
  for (const doc of elements(app.documents)) opened(doc);
}
