// The Sketch side of the plugin: the document of an action and the fonts of
// its text layers, read through Sketch's JavaScript API (`sketch/dom`). It
// matches nothing; the app's `fonts_needed` and `document_closed` do.

import type { DocumentWatcher, HostDocument, NeededFont, PluginUi } from '@fm/plugin-shared';

/** A text layer of `sketch/dom`, the part the plugin reads. */
export interface SketchTextLayer {
  style?: { fontFamily?: unknown } | null;
  /** The native `MSTextLayer`. */
  sketchObject?: { fontPostscriptName?(): unknown } | null;
}

/** A document of `sketch/dom`. */
export interface SketchDocument {
  /** Sketch's id of the document, stable while it is open. */
  id: string;
  /** The file, or nothing while the document is unsaved. */
  path?: string | null;
  /** The native `MSDocument`. */
  sketchObject?: { displayName?(): unknown } | null;
}

/** The part of `sketch/dom` the plugin uses. */
export interface SketchDom {
  find(selector: string, scope: SketchDocument): SketchTextLayer[];
  Document: { fromNative(native: unknown): SketchDocument };
}

/** What Sketch passes to an action handler. */
export interface ActionContext {
  actionContext?: { document?: unknown } | null;
}

// Native values are `NSString`s or `nil`.
function text(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  const s = String(value).trim();
  return s === '' ? null : s;
}

/** The fonts of every text layer on every page, each once, in document order. */
export function documentFonts(dom: SketchDom, doc: SketchDocument): NeededFont[] {
  const fonts: NeededFont[] = [];
  const seen = new Set<string>();
  for (const layer of dom.find('Text', doc)) {
    const postscriptName = text(layer.sketchObject?.fontPostscriptName?.());
    const family = text(layer.style?.fontFamily);
    if (!postscriptName && !family) continue;
    const font: NeededFont = {};
    if (postscriptName) font.postscriptName = postscriptName;
    if (family) font.family = family;
    const key = JSON.stringify([postscriptName, family]);
    if (seen.has(key)) continue;
    seen.add(key);
    fonts.push(font);
  }
  return fonts;
}

/** The document as the app knows it: Sketch's id is the key. */
export function hostDocument(doc: SketchDocument): HostDocument {
  const fileName = doc.path ? doc.path.slice(doc.path.lastIndexOf('/') + 1) : null;
  const name = text(doc.sketchObject?.displayName?.()) ?? text(fileName) ?? 'Untitled';
  return { key: `Sketch: ${doc.id}`, name };
}

/** What the handlers need from the host. */
export interface SketchPluginHost {
  dom: SketchDom;
  watcher: DocumentWatcher;
  ui: PluginUi;
  /** Keeps the script running until the returned function is called. */
  keepAlive(): () => void;
}

/** The action handlers the manifest names. */
export interface SketchHandlers {
  onOpenDocument(context: ActionContext): Promise<void>;
  onCloseDocument(context: ActionContext): Promise<void>;
}

export function sketchPlugin(host: SketchPluginHost): SketchHandlers {
  const { dom, watcher, ui } = host;
  const documentOf = (context: ActionContext): SketchDocument | null => {
    const native = context?.actionContext?.document;
    return native ? dom.Document.fromNative(native) : null;
  };
  // Sketch ends a script when its handler returns; the calls are async.
  const held = async (work: () => Promise<void>): Promise<void> => {
    const release = host.keepAlive();
    try {
      await work();
    } catch (e) {
      // A Sketch API that threw; never into Sketch's event loop.
      ui.notify(`Font Manager: ${String(e)}`);
    } finally {
      release();
    }
  };
  return {
    onOpenDocument: (context) =>
      held(async () => {
        const doc = documentOf(context);
        if (!doc) return;
        await watcher.opened(hostDocument(doc), documentFonts(dom, doc));
      }),
    onCloseDocument: (context) =>
      held(async () => {
        const doc = documentOf(context);
        if (!doc) return;
        await watcher.closed(hostDocument(doc));
      }),
  };
}
