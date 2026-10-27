import { DocumentWatcher, FmClient, NOT_RUNNING, type PluginUi } from '@fm/plugin-shared';
import { fakeEndpoint } from '@fm/plugin-shared/testing';
import { describe, expect, it, vi } from 'vitest';
import {
  type InDesignDocument,
  type InDesignEvent,
  type InDesignFont,
  type InDesignModule,
  watchInDesign,
} from '../src/hosts/indesign';

const INSTALLED = 1718831470;
const NOT_AVAILABLE = 1718840929;

// InDesign collections are `length` plus `item(i)`, not arrays.
const collection = <T>(items: T[]) => ({ length: items.length, item: (i: number) => items[i] });

function inDesignDoc(id: number, name: string, fonts: InDesignFont[]) {
  return { id, name, fonts: collection(fonts) } satisfies InDesignDocument;
}

const minion: InDesignFont = {
  postscriptName: 'MinionPro-Regular',
  fontFamily: 'Minion Pro',
  fontStyleName: 'Regular',
  status: INSTALLED,
};
const lora: InDesignFont = {
  postscriptName: 'Lora-Bold',
  fontFamily: 'Lora',
  fontStyleName: 'Bold',
  status: NOT_AVAILABLE,
};

function mockInDesign(open: InDesignDocument[] = []) {
  const listeners = new Map<string, ((e: InDesignEvent) => void)[]>();
  const module: InDesignModule = {
    app: {
      documents: collection(open),
      addEventListener: (type, handler) => {
        listeners.set(type, [...(listeners.get(type) ?? []), handler]);
      },
    },
    FontStatus: { INSTALLED },
  };
  const fire = (type: string, target: unknown) =>
    (listeners.get(type) ?? []).forEach((h) => h({ target } as InDesignEvent));
  return { module, fire, listeners };
}

function setup(offer: number[] = [], answer = true, open: InDesignDocument[] = []) {
  const fake = fakeEndpoint({
    fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    document_closed: () => ({ offer }),
    deactivate: () => [],
  });
  const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => answer) };
  const host = mockInDesign(open);
  watchInDesign(host.module, new DocumentWatcher(new FmClient(fake), ui));
  return { fake, ui, ...host };
}

describe('watchInDesign', () => {
  it('listens to afterOpen, afterActivate and beforeClose', () => {
    const { listeners } = setup();
    expect([...listeners.keys()].sort()).toEqual(['afterActivate', 'afterOpen', 'beforeClose']);
  });

  it("sends one fonts_needed with the document's PostScript, family and style names on open", async () => {
    const { fake, fire } = setup();
    const doc = inDesignDoc(7, 'Poster.indd', [minion, lora]);
    fire('afterOpen', doc);
    // InDesign fires afterOpen again for the document's layout window.
    fire('afterOpen', { parent: doc });
    await vi.waitFor(() => expect(fake.calls('fonts_needed')).toHaveLength(1));
    expect(fake.calls('fonts_needed')).toEqual([
      {
        document: 'InDesign: Poster.indd #7',
        fonts: [
          { postscriptName: 'MinionPro-Regular', family: 'Minion Pro', style: 'Regular' },
          { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' },
        ],
      },
    ]);
  });

  it('sends the fonts InDesign later reports missing again', async () => {
    const { fake, fire } = setup();
    const doc = inDesignDoc(7, 'Poster.indd', [minion, lora]);
    fire('afterOpen', doc);
    fire('afterActivate', doc);
    await vi.waitFor(() => expect(fake.calls('fonts_needed')).toHaveLength(2));
    expect(fake.calls('fonts_needed')[1]).toEqual({
      document: 'InDesign: Poster.indd #7',
      fonts: [{ postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' }],
    });
  });

  it('sends nothing more when no font is missing', async () => {
    const { fake, fire } = setup();
    const doc = inDesignDoc(7, 'Poster.indd', [minion]);
    fire('afterOpen', doc);
    fire('afterActivate', doc);
    fire('afterActivate', { parent: doc });
    await vi.waitFor(() => expect(fake.calls('fonts_needed')).toHaveLength(1));
    await new Promise((r) => setTimeout(r, 10));
    expect(fake.calls('fonts_needed')).toHaveLength(1);
  });

  it('calls document_closed on close and deactivates the offer on yes', async () => {
    const { fake, fire, ui } = setup([4, 9], true);
    const doc = inDesignDoc(7, 'Poster.indd', [minion]);
    fire('afterOpen', doc);
    fire('beforeClose', doc);
    await vi.waitFor(() => expect(fake.calls('deactivate')).toHaveLength(1));
    expect(fake.calls('document_closed')).toEqual([{ document: 'InDesign: Poster.indd #7' }]);
    expect(ui.confirm).toHaveBeenCalledWith('Deactivate 2 fonts used only by Poster.indd?');
    expect(fake.calls('deactivate')).toEqual([{ target: { kind: 'faces', faces: [4, 9] } }]);
  });

  it('keeps the fonts on no', async () => {
    const { fake, fire, ui } = setup([4], false);
    const doc = inDesignDoc(7, 'Poster.indd', [minion]);
    fire('afterOpen', doc);
    fire('beforeClose', doc);
    await vi.waitFor(() => expect(ui.confirm).toHaveBeenCalledOnce());
    expect(fake.calls('deactivate')).toEqual([]);
  });

  it('does not treat closing one layout window as closing the document', async () => {
    const { fake, fire } = setup([4]);
    const doc = inDesignDoc(7, 'Poster.indd', [minion]);
    fire('afterOpen', doc);
    fire('beforeClose', { parent: doc });
    await vi.waitFor(() => expect(fake.calls('fonts_needed')).toHaveLength(1));
    await new Promise((r) => setTimeout(r, 10));
    expect(fake.calls('document_closed')).toEqual([]);
  });

  it('picks up the documents open before the plugin loaded', async () => {
    const { fake } = setup([], true, [inDesignDoc(3, 'Old.indd', [minion])]);
    await vi.waitFor(() =>
      expect(fake.calls('fonts_needed')).toEqual([
        {
          document: 'InDesign: Old.indd #3',
          fonts: [{ postscriptName: 'MinionPro-Regular', family: 'Minion Pro', style: 'Regular' }],
        },
      ]),
    );
  });

  it('says "not running" once when the app is not running', async () => {
    const { fake, fire, ui } = setup([4]);
    fake.listening = false;
    const doc = inDesignDoc(7, 'Poster.indd', [minion, lora]);
    fire('afterOpen', doc);
    fire('afterActivate', doc);
    fire('beforeClose', doc);
    await vi.waitFor(() => expect(ui.notify).toHaveBeenCalledWith(NOT_RUNNING));
    await new Promise((r) => setTimeout(r, 10));
    expect(ui.notify).toHaveBeenCalledOnce();
    expect(ui.confirm).not.toHaveBeenCalled();
  });
});
