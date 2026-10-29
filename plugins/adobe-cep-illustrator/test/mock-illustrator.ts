// A mocked Illustrator for the tests: `window.__adobe_cep__` whose
// `evalScript` runs the real `jsx/host.jsx` against a fake `app`, with
// `alert` and ScriptUI's `Window` recorded.

import hostJsx from '../jsx/host.jsx?raw';
import { type CepNative, EVAL_SCRIPT_ERROR } from '../src/cep';

export interface MockFont {
  name: string;
  family?: string;
  style?: string;
}

/** A text run: its font, or a function that throws as a broken run does. */
type MockRange = MockFont | (() => never);

export interface MockDocument {
  name: string;
  path: string;
  fullName: { fsName: string };
  textFrames: { textRanges: { characterAttributes: { textFont: MockFont } }[] }[];
  /** The XMP packet of a saved document, as Illustrator 29 writes it. */
  XMPString?: string;
}

/**
 * The font of a run whose font Illustrator does not have: a `TextFont`
 * whose every property throws UKFG, as Illustrator 29.7 answers.
 */
export function missingFont(): MockFont {
  const ukfg = (): never => {
    throw new Error("an Illustrator error occurred: 1430996551 ('UKFG')");
  };
  return {
    get name(): string {
      return ukfg();
    },
    get family(): string {
      return ukfg();
    },
    get style(): string {
      return ukfg();
    },
  };
}

const xml = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** An XMP packet with the `xmpTPg:Fonts` bag Illustrator writes on save. */
export function xmpPacket(fonts: MockFont[]): string {
  const items = fonts.map(
    (f) => `               <rdf:li rdf:parseType="Resource">
                  <stFnt:fontName>${xml(f.name)}</stFnt:fontName>
                  <stFnt:fontFamily>${xml(f.family ?? '')}</stFnt:fontFamily>
                  <stFnt:fontFace>${xml(f.style ?? '')}</stFnt:fontFace>
                  <stFnt:fontType>TrueType</stFnt:fontType>
                  <stFnt:versionString>Version 1.003</stFnt:versionString>
                  <stFnt:composite>False</stFnt:composite>
                  <stFnt:fontFileName/>
               </rdf:li>`,
  );
  return `<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description>
         <xmpTPg:Fonts>
            <rdf:Bag>
${items.join('\n')}
            </rdf:Bag>
         </xmpTPg:Fonts>
</rdf:Description></rdf:RDF></x:xmpmeta>`;
}

/**
 * A document; `path` empty for an unsaved one. Each frame is a list of runs;
 * `xmp` the fonts its XMP metadata names (a saved document).
 */
export function mockDocument(
  name: string,
  path: string,
  frames: MockRange[][],
  xmp?: MockFont[],
): MockDocument {
  return {
    name,
    path,
    fullName: { fsName: path === '' ? '' : `${path}/${name}` },
    ...(xmp ? { XMPString: xmpPacket(xmp) } : {}),
    textFrames: frames.map((ranges) => ({
      textRanges: ranges.map((r) =>
        typeof r === 'function'
          ? {
              get characterAttributes(): never {
                return r();
              },
            }
          : { characterAttributes: { textFont: r } },
      ),
    })),
  };
}

export interface Dialog {
  title: string;
  message: string;
  buttons: string[];
}

export function mockIllustrator(open: MockDocument[] = []) {
  const app = { documents: [...open] };
  const alerts: string[] = [];
  const dialogs: Dialog[] = [];
  const state = { press: 'Keep them', broken: false };
  const listeners = new Map<string, ((event: unknown) => void)[]>();
  const scripts: string[] = [];

  // ScriptUI's dialog: the button named in `state.press` is pressed. A
  // group is the dialog itself here, so its buttons land on the dialog.
  class Window {
    private readonly dialog: Dialog;
    constructor(_type: string, title: string) {
      this.dialog = { title, message: '', buttons: [] };
      dialogs.push(this.dialog);
    }
    add(type: string, _bounds?: unknown, text = '') {
      if (type === 'statictext') this.dialog.message = text;
      if (type === 'button') this.dialog.buttons.push(text);
      return this;
    }
    show() {
      const pressed = this.dialog.buttons.indexOf(state.press);
      return pressed === 1 ? 1 : 2;
    }
  }
  const alert = (message: string) => alerts.push(message);

  const run = (script: string): string => {
    const body = `${hostJsx}\nreturn ${script};`;
    return String(new Function('app', 'alert', 'Window', body)(app, alert, Window));
  };

  const cep: CepNative = {
    evalScript(script, callback) {
      scripts.push(script);
      setTimeout(() => {
        if (state.broken) return callback(EVAL_SCRIPT_ERROR);
        try {
          callback(run(script));
        } catch {
          callback(EVAL_SCRIPT_ERROR);
        }
      }, 0);
    },
    addEventListener(type, handler) {
      listeners.set(type, [...(listeners.get(type) ?? []), handler]);
    },
  };
  const fire = (type: string) => (listeners.get(type) ?? []).forEach((h) => h({ type }));
  const close = (doc: MockDocument) => {
    app.documents.splice(app.documents.indexOf(doc), 1);
    fire('documentAfterDeactivate');
  };
  const openDocument = (doc: MockDocument) => {
    app.documents.push(doc);
    fire('documentAfterActivate');
  };
  return { app, cep, fire, close, openDocument, alerts, dialogs, state, listeners, scripts, run };
}
