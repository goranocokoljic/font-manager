// The extension's two UI pieces: it has no visible panel, so the message and
// the question are ExtendScript dialogs (`fmTell`, `fmAsk` in host.jsx) with
// the UXP plugin's words and buttons.

import type { PluginUi } from '@fm/plugin-shared';
import { type ExtendScript, scriptCall } from './cep';

export function extendScriptUi(script: ExtendScript): PluginUi {
  return {
    notify(message) {
      script.evalScript(scriptCall('fmTell', message)).catch(() => {});
    },
    async confirm(message) {
      try {
        return (await script.evalScript(scriptCall('fmAsk', message))) === 'yes';
      } catch {
        // No answer: the fonts stay active.
        return false;
      }
    },
  };
}
