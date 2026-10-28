// The two pieces of CEP the extension uses: ExtendScript calls and host
// events. `window.__adobe_cep__` is what Adobe's CSInterface.js wraps; the
// extension talks to it directly, so it ships no copy of that file.

import type { ExtendScript } from '@fm/plugin-shared';

export { type ExtendScript, scriptCall, scriptString } from '@fm/plugin-shared';

/** The part of `window.__adobe_cep__` the extension uses. */
export interface CepNative {
  evalScript(script: string, callback: (result: string) => void): void;
  addEventListener(type: string, handler: (event: unknown) => void): void;
}

/** Host events, as CSInterface's `addEventListener` gives them. */
export interface CepEvents {
  addEventListener(type: string, handler: () => void): void;
}

/** What `evalScript` answers when the script threw. */
export const EVAL_SCRIPT_ERROR = 'EvalScript error.';

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
