// @vitest-environment jsdom
// E30.4 criterion 3: the missing-fonts panel follows the host's active
// document through the host events.
import {
  DocumentWatcher,
  FmClient,
  type FontsNeededRequest,
  MissingFontsPanel,
  type PluginUi,
} from '@fm/plugin-shared';
import { fakeEndpoint } from '@fm/plugin-shared/testing';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  type InDesignDocument,
  type InDesignEvent,
  type InDesignModule,
  inDesignActive,
  watchInDesign,
} from '../src/hosts/indesign';
import {
  type PhotoshopDescriptor,
  type PhotoshopDocument,
  type PhotoshopModule,
  photoshopActive,
  watchPhotoshop,
} from '../src/hosts/photoshop';

const INSTALLED = 1;
const MISSING = 2;
const FACES = new Map([
  ['Lora-Bold', 11],
  ['Barlow-Italic', 12],
]);

function endpoint() {
  return fakeEndpoint({
    fonts_needed: (body) => {
      const req = body as FontsNeededRequest;
      const matched = req.fonts
        .filter((f) => FACES.has(f.postscriptName ?? ''))
        .map((font) => ({
          font,
          face: FACES.get(font.postscriptName ?? ''),
          matchedBy: 'postscriptName',
        }));
      const unmatched = req.fonts.filter((f) => !FACES.has(f.postscriptName ?? ''));
      return { matched, unmatched, missing: unmatched.length, outcomes: [] };
    },
    conflicts: () => [],
    document_closed: () => ({ offer: [] }),
  });
}

function mount(fake: ReturnType<typeof endpoint>, active: () => ReturnType<typeof inDesignActive>) {
  const container = document.createElement('div');
  document.body.append(container);
  const client = new FmClient(fake);
  const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => false) };
  const panel = new MissingFontsPanel(container, client, { activeDocument: active });
  return { container, panel, watcher: new DocumentWatcher(client, ui) };
}

const names = (c: HTMLElement) =>
  [...c.querySelectorAll('.fm-missing-name')].map((e) => e.textContent);
const count = (c: HTMLElement) => c.querySelector('[role="status"]')?.textContent;

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('the panel in InDesign', () => {
  const collection = <T>(items: T[]) => ({ length: items.length, item: (i: number) => items[i] });
  const doc = (id: number, name: string, font: string): InDesignDocument => ({
    id,
    name,
    fonts: collection([
      { postscriptName: 'MinionPro-Regular', status: INSTALLED },
      { postscriptName: font, status: MISSING },
    ]),
  });

  it('refreshes on switching and closing documents and says when none is open', async () => {
    const poster = doc(7, 'Poster.indd', 'Lora-Bold');
    const flyer = doc(9, 'Flyer.indd', 'Barlow-Italic');
    const listeners = new Map<string, ((e: InDesignEvent) => void)[]>();
    const indesign: InDesignModule = {
      app: {
        documents: collection([poster, flyer]),
        activeDocument: poster,
        addEventListener: (type, handler) => {
          listeners.set(type, [...(listeners.get(type) ?? []), handler]);
        },
      },
      FontStatus: { INSTALLED },
    };
    const fire = (type: string, target: unknown) =>
      (listeners.get(type) ?? []).forEach((h) => h({ target } as InDesignEvent));
    const fake = endpoint();
    const { container, panel, watcher } = mount(fake, () => inDesignActive(indesign));
    watchInDesign(indesign, watcher, () => void panel.refresh());
    await vi.waitFor(() => expect(names(container)).toEqual(['Lora-Bold']));
    expect(count(container)).toBe('1 font missing');

    indesign.app.activeDocument = flyer;
    fire('afterActivate', flyer);
    await vi.waitFor(() => expect(names(container)).toEqual(['Barlow-Italic']));
    expect(fake.calls('fonts_needed').at(-1)).toEqual({
      fonts: [{ postscriptName: 'Barlow-Italic' }],
      document: 'InDesign: Flyer.indd #9',
      activate: false,
    });

    fire('beforeClose', flyer);
    indesign.app.documents = collection([poster]);
    indesign.app.activeDocument = poster;
    fire('afterClose', null);
    await vi.waitFor(() => expect(names(container)).toEqual(['Lora-Bold']));

    fire('beforeClose', poster);
    indesign.app.documents = collection([]);
    indesign.app.activeDocument = undefined;
    fire('afterClose', null);
    await vi.waitFor(() => expect(count(container)).toBe('No document is open.'));
    expect(names(container)).toEqual([]);
  });
});

describe('the panel in Photoshop', () => {
  const text = (font: string) => ({ kind: 'text', textItem: { characterStyle: { font } } });
  const doc = (id: number, name: string, font: string): PhotoshopDocument => ({
    id,
    name,
    layers: [text('Inter-Regular'), text(font)],
  });

  it('refreshes on open, select and close and says when none is open', async () => {
    const poster = doc(12, 'Poster.psd', 'Lora-Bold');
    const flyer = doc(13, 'Flyer.psd', 'Barlow-Italic');
    let handler: ((event: string, d: PhotoshopDescriptor) => void) | undefined;
    const photoshop: PhotoshopModule = {
      app: { documents: [], activeDocument: null, fonts: [{ postScriptName: 'Inter-Regular' }] },
      action: { addNotificationListener: (_events, h) => void (handler = h) },
    };
    const fire = (event: string, d: PhotoshopDescriptor = {}) => handler?.(event, d);
    const fake = endpoint();
    const { container, panel, watcher } = mount(fake, () => photoshopActive(photoshop));
    watchPhotoshop(photoshop, watcher, () => void panel.refresh());
    await panel.refresh();
    expect(count(container)).toBe('No document is open.');

    photoshop.app.documents = [poster, flyer];
    photoshop.app.activeDocument = poster;
    fire('open', { documentID: 12 });
    await vi.waitFor(() => expect(names(container)).toEqual(['Lora-Bold']));

    photoshop.app.activeDocument = flyer;
    fire('select');
    await vi.waitFor(() => expect(names(container)).toEqual(['Barlow-Italic']));

    photoshop.app.documents = [];
    photoshop.app.activeDocument = null;
    fire('close');
    await vi.waitFor(() => expect(count(container)).toBe('No document is open.'));
    expect(names(container)).toEqual([]);
  });
});
