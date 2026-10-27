// The Photoshop side: the `open`, `select` and `close` notifications of the
// UXP `photoshop` module, turned into DocumentWatcher calls; each of them
// also tells the missing-fonts panel the active document changed.

import type { DocumentWatcher, HostDocument, NeededFont, PanelDocument } from '@fm/plugin-shared';
import { text } from './collection';

export interface PhotoshopLayer {
  kind?: string;
  layers?: readonly PhotoshopLayer[] | null;
  textItem?: { characterStyle?: { font?: string } } | null;
}

export interface PhotoshopDocument {
  id: number;
  name: string;
  layers: readonly PhotoshopLayer[];
}

/** A notification's descriptor, the keys the plugin reads. */
export interface PhotoshopDescriptor {
  documentID?: number;
}

/** The part of the `photoshop` module the plugin uses. */
export interface PhotoshopModule {
  app: {
    documents: readonly PhotoshopDocument[];
    activeDocument?: PhotoshopDocument | null;
    /** The fonts Photoshop has. */
    fonts: readonly { postScriptName: string }[];
  };
  action: {
    addNotificationListener(
      events: { event: string }[],
      handler: (event: string, descriptor: PhotoshopDescriptor) => void,
    ): unknown;
  };
}

/** The PostScript names of the document's text layers, once each, in layer order. */
export function photoshopFonts(doc: PhotoshopDocument): NeededFont[] {
  const names = new Set<string>();
  const walk = (layers: readonly PhotoshopLayer[] | null | undefined) => {
    for (const layer of layers ?? []) {
      if (layer.kind === 'group') walk(layer.layers);
      if (layer.kind !== 'text') continue;
      let font: string | undefined;
      try {
        font = text(layer.textItem?.characterStyle?.font);
      } catch {
        // A layer Photoshop cannot describe has no font for us.
      }
      if (font) names.add(font);
    }
  };
  walk(doc.layers);
  return [...names].map((postscriptName) => ({ postscriptName }));
}

/** The fonts of the document that Photoshop does not have. */
export function photoshopMissingFonts(
  doc: PhotoshopDocument,
  installed: readonly { postScriptName: string }[],
): NeededFont[] {
  const have = new Set(installed.map((f) => f.postScriptName));
  return photoshopFonts(doc).filter((f) => !have.has(f.postscriptName ?? ''));
}

/** The document as the plugin names it to the app. */
export function photoshopHostDocument(doc: PhotoshopDocument): HostDocument {
  return { key: `Photoshop: ${doc.name} #${doc.id}`, name: doc.name };
}

/** The active document and its missing fonts, for the panel. */
export function photoshopActive(photoshop: PhotoshopModule): PanelDocument | null {
  const { app } = photoshop;
  if (app.documents.length === 0) return null;
  const doc = app.activeDocument;
  if (!doc) return null;
  return { doc: photoshopHostDocument(doc), missing: photoshopMissingFonts(doc, app.fonts) };
}

/**
 * Follows Photoshop's documents until the plugin unloads. `changed` runs
 * after every notification, once the watcher has handled it.
 */
export function watchPhotoshop(
  photoshop: PhotoshopModule,
  watcher: DocumentWatcher,
  changed: () => void = () => {},
): void {
  const { app, action } = photoshop;
  const open = new Map<number, HostDocument>();
  const byId = (id: number | undefined) => app.documents.find((d) => d.id === id);
  const work: Promise<void>[] = [];

  const opened = (doc: PhotoshopDocument) => {
    if (open.has(doc.id)) return;
    const host = photoshopHostDocument(doc);
    open.set(doc.id, host);
    work.push(watcher.opened(host, photoshopFonts(doc)));
  };
  const closed = (id: number) => {
    const host = open.get(id);
    if (!host) return;
    open.delete(id);
    work.push(watcher.closed(host));
  };

  action.addNotificationListener(
    [{ event: 'open' }, { event: 'select' }, { event: 'close' }],
    (event, descriptor) => {
      handle(event, descriptor);
      void Promise.all(work.splice(0)).then(changed);
    },
  );

  function handle(event: string, descriptor: PhotoshopDescriptor): void {
    if (event === 'open') {
      const doc = byId(descriptor?.documentID) ?? app.activeDocument;
      if (doc) opened(doc);
    } else if (event === 'select') {
      const doc = app.activeDocument;
      const host = doc && open.get(doc.id);
      if (doc && host) work.push(watcher.missing(host, photoshopMissingFonts(doc, app.fonts)));
    } else if (event === 'close') {
      // The descriptor names the document when Photoshop knows it; else
      // the closed ones are those no longer in `app.documents`.
      if (descriptor?.documentID !== undefined) return closed(descriptor.documentID);
      const still = new Set(app.documents.map((d) => d.id));
      for (const id of [...open.keys()]) if (!still.has(id)) closed(id);
    }
  }

  // Documents open before the plugin loaded.
  for (const doc of app.documents) opened(doc);
}
