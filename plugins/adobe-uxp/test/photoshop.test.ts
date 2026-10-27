import { DocumentWatcher, FmClient, NOT_RUNNING, type PluginUi } from '@fm/plugin-shared';
import { fakeEndpoint } from '@fm/plugin-shared/testing';
import { describe, expect, it, vi } from 'vitest';
import {
  type PhotoshopDescriptor,
  type PhotoshopDocument,
  type PhotoshopLayer,
  type PhotoshopModule,
  photoshopFonts,
  watchPhotoshop,
} from '../src/hosts/photoshop';

const textLayer = (font: string): PhotoshopLayer => ({
  kind: 'text',
  textItem: { characterStyle: { font } },
});

const poster: PhotoshopDocument = {
  id: 12,
  name: 'Poster.psd',
  layers: [
    textLayer('Inter-Regular'),
    { kind: 'pixel' },
    { kind: 'group', layers: [textLayer('Lora-Bold'), textLayer('Inter-Regular')] },
  ],
};

function mockPhotoshop(open: PhotoshopDocument[] = []) {
  let handler: ((event: string, d: PhotoshopDescriptor) => void) | undefined;
  let events: string[] = [];
  const app: PhotoshopModule['app'] = {
    documents: [...open],
    activeDocument: open[0] ?? null,
    fonts: [{ postScriptName: 'Inter-Regular' }],
  };
  const module: PhotoshopModule = {
    app,
    action: {
      addNotificationListener: (list, h) => {
        events = list.map((e) => e.event);
        handler = h;
      },
    },
  };
  const fire = (event: string, descriptor: PhotoshopDescriptor = {}) =>
    handler?.(event, descriptor);
  return { module, app, fire, events: () => events };
}

function setup(offer: number[] = [], answer = true, open: PhotoshopDocument[] = []) {
  const fake = fakeEndpoint({
    fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    document_closed: () => ({ offer }),
    deactivate: () => [],
  });
  const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => answer) };
  const host = mockPhotoshop(open);
  watchPhotoshop(host.module, new DocumentWatcher(new FmClient(fake), ui));
  return { fake, ui, ...host };
}

function openDoc(app: PhotoshopModule['app'], doc: PhotoshopDocument) {
  app.documents = [...app.documents, doc];
  app.activeDocument = doc;
}

describe('photoshopFonts', () => {
  it("collects the text layers' fonts once each, groups included", () => {
    expect(photoshopFonts(poster)).toEqual([
      { postscriptName: 'Inter-Regular' },
      { postscriptName: 'Lora-Bold' },
    ]);
  });

  it('skips a text layer Photoshop cannot describe', () => {
    const broken: PhotoshopLayer = {
      kind: 'text',
      get textItem(): PhotoshopLayer['textItem'] {
        throw new Error('The command is not available');
      },
    };
    expect(photoshopFonts({ id: 1, name: 'x', layers: [broken, textLayer('Lora-Bold')] })).toEqual([
      { postscriptName: 'Lora-Bold' },
    ]);
  });
});

describe('watchPhotoshop', () => {
  it('listens to the open, select and close notifications', () => {
    expect(setup().events().sort()).toEqual(['close', 'open', 'select']);
  });

  it("sends one fonts_needed with the text layers' fonts on open", async () => {
    const { fake, app, fire } = setup();
    openDoc(app, poster);
    fire('open', { documentID: 12 });
    await vi.waitFor(() =>
      expect(fake.calls('fonts_needed')).toEqual([
        {
          document: 'Photoshop: Poster.psd #12',
          fonts: [{ postscriptName: 'Inter-Regular' }, { postscriptName: 'Lora-Bold' }],
        },
      ]),
    );
  });

  it('sends the fonts Photoshop later reports missing again', async () => {
    const { fake, app, fire } = setup();
    openDoc(app, poster);
    fire('open', { documentID: 12 });
    fire('select');
    fire('select');
    await vi.waitFor(() => expect(fake.calls('fonts_needed')).toHaveLength(2));
    await new Promise((r) => setTimeout(r, 10));
    expect(fake.calls('fonts_needed')).toEqual([
      expect.anything(),
      { document: 'Photoshop: Poster.psd #12', fonts: [{ postscriptName: 'Lora-Bold' }] },
    ]);
  });

  it('calls document_closed and deactivates the offer on yes', async () => {
    const { fake, app, fire, ui } = setup([5], true);
    openDoc(app, poster);
    fire('open', { documentID: 12 });
    app.documents = [];
    fire('close', { documentID: 12 });
    await vi.waitFor(() => expect(fake.calls('deactivate')).toHaveLength(1));
    expect(fake.calls('document_closed')).toEqual([{ document: 'Photoshop: Poster.psd #12' }]);
    expect(ui.confirm).toHaveBeenCalledWith('Deactivate 1 font used only by Poster.psd?');
    expect(fake.calls('deactivate')).toEqual([{ target: { kind: 'faces', faces: [5] } }]);
  });

  it('keeps the fonts on no', async () => {
    const { fake, app, fire, ui } = setup([5], false);
    openDoc(app, poster);
    fire('open', { documentID: 12 });
    fire('close', { documentID: 12 });
    await vi.waitFor(() => expect(ui.confirm).toHaveBeenCalledOnce());
    expect(fake.calls('deactivate')).toEqual([]);
  });

  it('finds the closed document when the descriptor does not name it', async () => {
    const other: PhotoshopDocument = { id: 13, name: 'Other.psd', layers: [] };
    const { fake, app, fire } = setup([], true, [poster, other]);
    app.documents = [other];
    fire('close');
    await vi.waitFor(() =>
      expect(fake.calls('document_closed')).toEqual([{ document: 'Photoshop: Poster.psd #12' }]),
    );
  });

  it('says "not running" once when the app is not running', async () => {
    const { fake, app, fire, ui } = setup([5]);
    fake.listening = false;
    openDoc(app, poster);
    fire('open', { documentID: 12 });
    fire('select');
    fire('close', { documentID: 12 });
    await vi.waitFor(() => expect(ui.notify).toHaveBeenCalledWith(NOT_RUNNING));
    await new Promise((r) => setTimeout(r, 10));
    expect(ui.notify).toHaveBeenCalledOnce();
    expect(ui.confirm).not.toHaveBeenCalled();
  });
});
