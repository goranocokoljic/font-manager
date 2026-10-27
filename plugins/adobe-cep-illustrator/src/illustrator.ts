// The Illustrator side. CEP gives Illustrator no open or close event, so the
// extension compares the open documents (`fmOpenDocuments` in host.jsx) with
// the ones it knows, on every document (de)activation and on a timer: a new
// one is opened, a gone one is closed.

import type { DocumentWatcher, HostDocument, NeededFont } from '@fm/plugin-shared';
import { type CepEvents, type ExtendScript, scriptCall } from './cep';

/** The events after which the open documents are compared. */
export const DOCUMENT_EVENTS = ['documentAfterActivate', 'documentAfterDeactivate'];

/** How often the open documents are compared without an event, in ms. */
export const SYNC_EVERY_MS = 2000;

/** An open document as `fmOpenDocuments` names it. */
interface OpenDocument {
  key: string;
  name: string;
}

function isOpenDocument(value: unknown): value is OpenDocument {
  const d = value as Partial<OpenDocument> | null;
  return typeof d?.key === 'string' && typeof d.name === 'string';
}

function parseList(text: string): unknown[] {
  const value: unknown = JSON.parse(text);
  if (!Array.isArray(value)) throw new Error(`not a list: ${text}`);
  return value;
}

/** A name the host gave, or nothing for an empty one. */
function name(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() !== '' ? value : undefined;
}

/** `fmDocumentFonts`' answer as `fonts_needed` takes it; `null` for no such document. */
export function illustratorFonts(text: string): NeededFont[] | null {
  if (text === 'null') return null;
  return parseList(text).flatMap((f) => {
    const font = (f ?? {}) as Record<string, unknown>;
    const needed: NeededFont = {
      postscriptName: name(font.postscriptName),
      family: name(font.family),
      style: name(font.style),
    };
    return needed.postscriptName || needed.family ? [needed] : [];
  });
}

export interface IllustratorWatch {
  /** Compares the open documents now, after any comparison still running. */
  sync(): Promise<void>;
}

/** Follows Illustrator's documents until the extension unloads. */
export function watchIllustrator(
  script: ExtendScript,
  events: CepEvents,
  watcher: DocumentWatcher,
  every: (ms: number, run: () => void) => void = (ms, run) => setInterval(run, ms),
): IllustratorWatch {
  const open = new Map<string, HostDocument>();

  const compare = async () => {
    const docs = parseList(await script.evalScript('fmOpenDocuments()')).filter(isOpenDocument);
    const now = new Set(docs.map((d) => d.key));
    const work: Promise<void>[] = [];
    for (const [key, host] of [...open]) {
      if (now.has(key)) continue;
      open.delete(key);
      work.push(watcher.closed(host));
    }
    for (const doc of docs) {
      // Sent once per document: known ones are skipped on every later event.
      if (open.has(doc.key)) continue;
      const fonts = illustratorFonts(
        await script.evalScript(scriptCall('fmDocumentFonts', doc.key)),
      );
      if (fonts === null) continue;
      const host = { key: `Illustrator: ${doc.key}`, name: doc.name };
      open.set(doc.key, host);
      work.push(watcher.opened(host, fonts));
    }
    await Promise.all(work);
  };

  let queue: Promise<void> = Promise.resolve();
  const sync = () => {
    // A failed comparison (Illustrator busy, a script error) changes
    // nothing: the next event or tick tries again.
    queue = queue.then(compare).catch(() => {});
    return queue;
  };

  for (const type of DOCUMENT_EVENTS) events.addEventListener(type, () => void sync());
  every(SYNC_EVERY_MS, () => void sync());
  // Documents open before the extension loaded.
  void sync();
  return { sync };
}
