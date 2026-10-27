// "Font Manager is not running" is said once, until a call gets through
// again. Sketch runs every action handler in a fresh script, so the
// `DocumentWatcher` of one action does not know what the last one said; the
// flag lives in Sketch's session variables (`sketch/settings`) instead.

import { FmApiError, NOT_RUNNING, type PluginUi, type Transport } from '@fm/plugin-shared';

/** The part of `sketch/settings` the plugin uses: values kept until Sketch quits. */
export interface SessionStore {
  sessionVariable(key: string): unknown;
  setSessionVariable(key: string, value: unknown): void;
}

const TOLD = 'org.fontmanager.sketch.toldNotRunning';

/** `ui`, saying "not running" only when the last action did not say it already. */
export function notRunningOnce(ui: PluginUi, store: SessionStore): PluginUi {
  return {
    notify(message) {
      if (message === NOT_RUNNING) {
        if (store.sessionVariable(TOLD) === true) return;
        store.setSessionVariable(TOLD, true);
      }
      ui.notify(message);
    },
    confirm: (message) => ui.confirm(message),
  };
}

/** `transport`, forgetting that "not running" was said once the app answers. */
export function rememberingReached(transport: Transport, store: SessionStore): Transport {
  return {
    async call<T>(method: string, params: unknown): Promise<T> {
      try {
        const answer = await transport.call<T>(method, params);
        store.setSessionVariable(TOLD, false);
        return answer;
      } catch (e) {
        if (e instanceof FmApiError) store.setSessionVariable(TOLD, false);
        throw e;
      }
    },
  };
}
