// The plugin's two UI pieces in Sketch: a message at the bottom of the
// window (`sketch/ui`) and a yes/no question as an `NSAlert`.

import type { PluginUi } from '@fm/plugin-shared';

/** The part of `sketch/ui` the plugin uses. */
export interface SketchUiModule {
  message(text: string): void;
}

/** The part of a native `NSAlert` the plugin uses. */
export interface NativeAlert {
  setMessageText(text: string): void;
  setInformativeText(text: string): void;
  addButtonWithTitle(title: string): unknown;
  runModal(): number;
}

/** `NSAlertFirstButtonReturn`: the first button added was clicked. */
export const FIRST_BUTTON = 1000;

export function sketchUi(ui: SketchUiModule, newAlert: () => NativeAlert): PluginUi {
  return {
    notify: (message) => ui.message(message),
    async confirm(message) {
      const alert = newAlert();
      alert.setMessageText('Font Manager');
      alert.setInformativeText(message);
      alert.addButtonWithTitle('Deactivate');
      alert.addButtonWithTitle('Keep');
      return Number(alert.runModal()) === FIRST_BUTTON;
    },
  };
}
