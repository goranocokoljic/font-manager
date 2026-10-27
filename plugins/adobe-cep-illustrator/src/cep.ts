// The two pieces of CEP the extension uses: ExtendScript calls and host
// events. `window.__adobe_cep__` is what Adobe's CSInterface.js wraps; the
// extension talks to it directly, so it ships no copy of that file.

/** The part of `window.__adobe_cep__` the extension uses. */
export interface CepNative {
  evalScript(script: string, callback: (result: string) => void): void;
  addEventListener(type: string, handler: (event: unknown) => void): void;
}

/** Runs a script in Illustrator's ExtendScript engine (`jsx/host.jsx`). */
export interface ExtendScript {
  evalScript(script: string): Promise<string>;
}

/** Host events, as CSInterface's `addEventListener` gives them. */
export interface CepEvents {
  addEventListener(type: string, handler: () => void): void;
}

/** What `evalScript` answers when the script threw. */
export const EVAL_SCRIPT_ERROR = 'EvalScript error.';

/**
 * `value` as an ExtendScript string literal. ExtendScript is ES3, where
 * U+2028 and U+2029 end a line, so JSON leaves them unescaped and they are
 * escaped here.
 */
export function scriptString(value: string): string {
  return JSON.stringify(value)
    .replace(/\u2028/g, '\\u2028')
    .replace(/\u2029/g, '\\u2029');
}

/** A call of a `host.jsx` function with string arguments. */
export function scriptCall(name: string, ...args: string[]): string {
  return `${name}(${args.map(scriptString).join(',')})`;
}

export function cepExtendScript(cep: CepNative): ExtendScript {
  return {
    evalScript: (script) =>
      new Promise((resolve, reject) =>
        cep.evalScript(script, (result) => {
          if (result === EVAL_SCRIPT_ERROR) reject(new Error(`ExtendScript failed: ${script}`));
          else resolve(result);
        }),
      ),
  };
}

export function cepEvents(cep: CepNative): CepEvents {
  return { addEventListener: (type, handler) => cep.addEventListener(type, () => handler()) };
}
