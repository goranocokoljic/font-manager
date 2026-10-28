// The plugin's entry in Sketch. Wiring only: the client and the document
// logic are `@fm/plugin-shared`, the Sketch side is `sketch.ts`, and
// `plugin.ts` puts them together.

import type { SketchSelectedLayer } from '@fm/font-switch';
import { endpointFilePath } from '@fm/plugin-shared';
import { type NativeFontManager, nativeFonts } from './font-switch';
import type { SessionStore } from './once';
import { createPlugin } from './plugin';
import type { SketchDom } from './sketch';
import type { NativeAlert, SketchUiModule } from './ui';

// Sketch's own modules, resolved by the host at run time.
declare const require: (id: string) => unknown;
// Cocoa classes are globals in Sketch's script runtime.
declare const NSAlert: { alloc(): { init(): NativeAlert } };
declare const NSFontManager: { sharedFontManager(): NativeFontManager };
declare const NSPasteboard: { generalPasteboard(): { stringForType(type: unknown): unknown } };
declare const NSPasteboardTypeString: unknown;

interface SelectionDom {
  getSelectedDocument(): { selectedLayers?: { layers?: SketchSelectedLayer[] } } | undefined;
}

interface OsModule {
  platform(): string;
  homedir(): string;
}
interface FsModule {
  readFileSync(path: string, encoding: 'utf8'): string;
}
interface AsyncModule {
  createFiber(): { cleanup(): void };
}

const os = require('os') as OsModule;
const fs = require('fs') as FsModule;
const fibers = require('sketch/async') as AsyncModule;
const endpointFile = endpointFilePath(os.platform(), os.homedir());

const dom = require('sketch/dom') as SketchDom & SelectionDom;

const handlers = createPlugin({
  dom,
  ui: require('sketch/ui') as SketchUiModule,
  settings: require('sketch/settings') as SessionStore,
  newAlert: () => NSAlert.alloc().init(),
  endpoint: {
    // A missing file throws; the client reports that as "not running".
    readEndpointFile: async () => fs.readFileSync(endpointFile, 'utf8'),
    fetch: (url, init) => fetch(url, init),
  },
  keepAlive: () => {
    const fiber = fibers.createFiber();
    return () => fiber.cleanup();
  },
  fontSwitch: {
    selectedLayers: () => dom.getSelectedDocument()?.selectedLayers?.layers ?? [],
    fonts: () => nativeFonts(NSFontManager.sharedFontManager()),
  },
  clipboardText: () => {
    // `nil` when the clipboard holds no text.
    const text = NSPasteboard.generalPasteboard().stringForType(NSPasteboardTypeString);
    return text === null || text === undefined ? null : String(text);
  },
});

// Sketch calls the handlers the manifest names as globals.
Object.assign(globalThis, handlers);
