// The plugin put together from Sketch's modules, so `main.ts` and the tests
// wire it the same way. Sketch has no pipe or socket API for plugins, so the
// app is reached over its local API (ADR 0015): `endpoint.json`, the bearer
// token, 127.0.0.1 only.

import type { SketchFontSwitchHost } from '@fm/font-switch';
import { type ClientHost, DocumentWatcher, FmClient, httpTransport } from '@fm/plugin-shared';
import { applyCopiedFontCommand } from './font-switch';
import { type SessionStore, notRunningOnce, rememberingReached } from './once';
import { type SketchDom, type SketchHandlers, sketchPlugin } from './sketch';
import { type NativeAlert, type SketchUiModule, sketchUi } from './ui';

/** What Sketch gives the plugin. */
export interface SketchModules {
  dom: SketchDom;
  ui: SketchUiModule;
  settings: SessionStore;
  newAlert(): NativeAlert;
  /** `endpoint.json` and `fetch`. */
  endpoint: ClientHost;
  /** Keeps the script running until the returned function is called. */
  keepAlive(): () => void;
  /** The selection and the fonts, for Font Switch. */
  fontSwitch: SketchFontSwitchHost;
  /** The text on the clipboard, or nothing. */
  clipboardText(): string | null;
}

/** The handlers the manifest names: the document events and Font Switch's command. */
export interface PluginHandlers extends SketchHandlers {
  onApplyCopiedFont(): Promise<void>;
}

export function createPlugin(modules: SketchModules): PluginHandlers {
  const { dom, settings } = modules;
  const client = new FmClient(rememberingReached(httpTransport(modules.endpoint), settings));
  const ui = notRunningOnce(sketchUi(modules.ui, modules.newAlert), settings);
  const watcher = new DocumentWatcher(client, ui, { reportMissing: true });
  return {
    ...sketchPlugin({ dom, ui, watcher, keepAlive: modules.keepAlive }),
    onApplyCopiedFont: applyCopiedFontCommand(modules.fontSwitch, modules.clipboardText, (text) =>
      modules.ui.message(text),
    ),
  };
}
