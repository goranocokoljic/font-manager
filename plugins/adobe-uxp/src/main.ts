// The plugin's entry in InDesign and Photoshop. Wiring only: the client and
// the document logic are `@fm/plugin-shared`, the host events are `hosts/`.

import { type ClientHost, DocumentWatcher, FmClient, endpointFilePath } from '@fm/plugin-shared';
import { type InDesignModule, watchInDesign } from './hosts/indesign';
import { type PhotoshopModule, watchPhotoshop } from './hosts/photoshop';
import { uxpUi } from './ui';

// UXP's own modules, resolved by the host at run time.
declare const require: (id: string) => unknown;

interface UxpModule {
  host: { name: string };
  entrypoints: { setup(entrypoints: unknown): void };
}
interface OsModule {
  platform(): string;
  homedir(): string;
}
interface FsModule {
  readFile(path: string, options: { encoding: 'utf-8' }): Promise<string>;
}

const uxp = require('uxp') as UxpModule;
const os = require('os') as OsModule;
const fs = require('fs') as FsModule;

const endpointFile = endpointFilePath(os.platform(), os.homedir());
const host: ClientHost = {
  readEndpointFile: () => fs.readFile(endpointFile, { encoding: 'utf-8' }),
  fetch: (url, init) => fetch(url, init),
};
const watcher = new DocumentWatcher(new FmClient(host), uxpUi(document));

uxp.entrypoints.setup({ panels: { fmPanel: { show() {} } } });

if (uxp.host.name === 'InDesign') {
  watchInDesign(require('indesign') as InDesignModule, watcher);
} else if (uxp.host.name === 'Photoshop') {
  watchPhotoshop(require('photoshop') as PhotoshopModule, watcher);
}
