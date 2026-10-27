// A fake of Sketch's modules for the tests: `sketch/dom` with documents of
// text layers, `sketch/ui`, `NSAlert` and `sketch/settings`.

import { vi } from 'vitest';
import type { SessionStore } from '../src/once';
import type { SketchDocument, SketchDom, SketchTextLayer } from '../src/sketch';
import type { NativeAlert, SketchUiModule } from '../src/ui';

/** A text layer whose native PostScript name is `postscript` (a fake `NSString`). */
export function textLayer(postscript: string | null, family: string | null): SketchTextLayer {
  return {
    style: { fontFamily: family },
    sketchObject: {
      fontPostscriptName: () => (postscript === null ? null : { toString: () => postscript }),
    },
  };
}

export interface MockDocument {
  native: { kind: 'MSDocument'; id: string };
  doc: SketchDocument;
  layers: SketchTextLayer[];
}

export function mockDocument(
  id: string,
  displayName: string | null,
  layers: SketchTextLayer[],
  path: string | null = null,
): MockDocument {
  return {
    native: { kind: 'MSDocument', id },
    doc: { id, path, sketchObject: { displayName: () => displayName } },
    layers,
  };
}

export function mockDom(docs: MockDocument[]) {
  const byNative = new Map(docs.map((d) => [d.native, d]));
  const byDoc = new Map(docs.map((d) => [d.doc, d]));
  return {
    find: vi.fn((selector: string, scope: SketchDocument) =>
      selector === 'Text' ? (byDoc.get(scope)?.layers ?? []) : [],
    ),
    Document: {
      fromNative: (native) => {
        const found = byNative.get(native as MockDocument['native']);
        if (!found) throw new Error('not a document');
        return found.doc;
      },
    },
  } satisfies SketchDom;
}

export function mockUi() {
  return { message: vi.fn<(text: string) => void>() } satisfies SketchUiModule;
}

/** `NSAlert`s that answer with the button at `answer` (0 is the first). */
export function mockAlerts(answer: number) {
  const shown: { message: string; informative: string; buttons: string[] }[] = [];
  const newAlert = (): NativeAlert => {
    const a = { message: '', informative: '', buttons: [] as string[] };
    return {
      setMessageText: (t) => void (a.message = t),
      setInformativeText: (t) => void (a.informative = t),
      addButtonWithTitle: (t) => a.buttons.push(t),
      runModal: () => {
        shown.push(a);
        return 1000 + answer;
      },
    };
  };
  return { newAlert, shown };
}

/** Session variables that outlive one action's script, as Sketch keeps them. */
export function mockSettings(): SessionStore {
  const values = new Map<string, unknown>();
  return {
    sessionVariable: (key) => values.get(key),
    setSessionVariable: (key, value) => void values.set(key, value),
  };
}
