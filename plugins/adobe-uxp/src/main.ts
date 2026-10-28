// The plugin's entry in InDesign and Photoshop. Wiring only: the client, the
// document logic and the missing-fonts panel are `@fm/plugin-shared`, the
// host events are `hosts/`, and a font dropped on the panel goes to
// `@fm/font-switch`'s adapter for the host (E34).

import {
  type InDesignFontSwitchApp,
  type PhotoshopFontSwitchModule,
  fontSwitchDropTarget,
  hostFromName,
  inDesignAdapter,
  photoshopAdapter,
} from '@fm/font-switch';
import {
  type ClientHost,
  DocumentWatcher,
  FmClient,
  MissingFontsPanel,
  type PanelSource,
  endpointFilePath,
} from '@fm/plugin-shared';
import { type InDesignModule, inDesignActive, watchInDesign } from './hosts/indesign';
import { type PhotoshopModule, photoshopActive, watchPhotoshop } from './hosts/photoshop';
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
const client = new FmClient(host);
const ui = uxpUi(document);
const watcher = new DocumentWatcher(client, ui);

const indesign =
  uxp.host.name === 'InDesign'
    ? (require('indesign') as InDesignModule & { app: InDesignFontSwitchApp })
    : null;
const photoshop =
  uxp.host.name === 'Photoshop'
    ? (require('photoshop') as PhotoshopModule & PhotoshopFontSwitchModule)
    : null;
const source: PanelSource = {
  activeDocument: () =>
    indesign ? inDesignActive(indesign) : photoshop ? photoshopActive(photoshop) : null,
};
const container = document.getElementById('missing') ?? document.body;
const panel = new MissingFontsPanel(container, client, source);
const refresh = () => void panel.refresh();

uxp.entrypoints.setup({ panels: { fmPanel: { show: refresh } } });

if (indesign) watchInDesign(indesign, watcher, refresh);
else if (photoshop) watchPhotoshop(photoshop, watcher, refresh);
refresh();

fontSwitchDropTarget(
  document,
  () => hostFromName(uxp.host.name),
  {
    indesign: indesign && inDesignAdapter(indesign.app),
    photoshop: photoshop && photoshopAdapter(photoshop),
  },
  (message) => ui.notify(message),
);
