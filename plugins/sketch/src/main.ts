// The plugin's entry in Sketch. Wiring only: the client and the document
// logic are `@fm/plugin-shared`, the Sketch side is `sketch.ts`, and
// `plugin.ts` puts them together.

import { endpointFilePath } from '@fm/plugin-shared';
import type { SessionStore } from './once';
import { createPlugin } from './plugin';
import type { SketchDom } from './sketch';
import type { NativeAlert, SketchUiModule } from './ui';

// Sketch's own modules, resolved by the host at run time.
declare const require: (id: string) => unknown;
// Cocoa classes are globals in Sketch's script runtime.
declare const NSAlert: { alloc(): { init(): NativeAlert } };

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

const handlers = createPlugin({
  dom: require('sketch/dom') as SketchDom,
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
});

// Sketch calls the handlers the manifest names as globals.
Object.assign(globalThis, handlers);
