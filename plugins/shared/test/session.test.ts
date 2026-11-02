import { describe, expect, it, vi } from 'vitest';
import { FmClient, NOT_RUNNING } from '../src/client';
import {
  DocumentWatcher,
  type PluginUi,
  deactivateQuestion,
  downloadedFontsMessage,
  fontLabel,
  missingFontsMessage,
  offeredNames,
} from '../src/session';
import type { OfferedFace } from '../src/api';
import { FAKE_URL, fakeEndpoint } from '../src/testing';

const doc = { key: 'InDesign: Poster.indd #7', name: 'Poster.indd' };
const inter = { postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' };
const lora = { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' };

function setup(offer: number[] = [], answer = true, offered?: OfferedFace[]) {
  const fake = fakeEndpoint({
    fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    document_closed: () => (offered ? { offer, offered } : { offer }),
    deactivate: (body) =>
      (body as { target: { faces: number[] } }).target.faces.map((face) => ({
        face,
        outcome: 'done',
      })),
  });
  const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => answer) };
  return { fake, ui, watcher: new DocumentWatcher(new FmClient(fake), ui) };
}

describe('DocumentWatcher', () => {
  it("sends one fonts_needed with all of a document's fonts when it opens", async () => {
    const { fake, watcher } = setup();
    await watcher.opened(doc, [inter, lora]);
    expect(fake.calls('fonts_needed')).toEqual([{ fonts: [inter, lora], document: doc.key }]);
  });

  it('sends nothing for a document without fonts', async () => {
    const { fake, watcher } = setup();
    await watcher.opened(doc, []);
    expect(fake.requests).toEqual([]);
  });

  it('sends fonts the host later reports missing again, once per change', async () => {
    const { fake, watcher } = setup();
    await watcher.opened(doc, [inter, lora]);
    await watcher.missing(doc, [lora]);
    await watcher.missing(doc, [lora]);
    await watcher.missing(doc, []);
    await watcher.missing(doc, [lora]);
    expect(fake.calls('fonts_needed')).toEqual([
      { fonts: [inter, lora], document: doc.key },
      { fonts: [lora], document: doc.key },
      { fonts: [lora], document: doc.key },
    ]);
  });

  it('asks on close and deactivates the offer when the answer is yes', async () => {
    const { fake, ui, watcher } = setup([4, 9], true);
    await watcher.closed(doc);
    expect(fake.calls('document_closed')).toEqual([{ document: doc.key }]);
    expect(ui.confirm).toHaveBeenCalledWith('Deactivate 2 fonts used only by Poster.indd?');
    expect(fake.calls('deactivate')).toEqual([{ target: { kind: 'faces', faces: [4, 9] } }]);
  });

  it('names the offered fonts in the question and deactivates them on yes', async () => {
    const offered: OfferedFace[] = [
      { face: 4, family: 'Alegreya', style: 'Bold', postscriptName: 'Alegreya-Bold' },
      { face: 9, family: 'Actor', style: 'Regular', postscriptName: null },
    ];
    const { fake, ui, watcher } = setup([4, 9], true, offered);
    await watcher.closed(doc);
    expect(ui.confirm).toHaveBeenCalledWith(
      'Deactivate 2 fonts used only by Poster.indd? Alegreya-Bold, Actor Regular',
    );
    expect(fake.calls('deactivate')).toEqual([{ target: { kind: 'faces', faces: [4, 9] } }]);
  });

  it('keeps the named fonts when the answer is no', async () => {
    const offered: OfferedFace[] = [
      { face: 4, family: 'Alegreya', style: 'Bold', postscriptName: 'Alegreya-Bold' },
    ];
    const { fake, ui, watcher } = setup([4], false, offered);
    await watcher.closed(doc);
    expect(ui.confirm).toHaveBeenCalledWith(
      'Deactivate 1 font used only by Poster.indd? Alegreya-Bold',
    );
    expect(fake.calls('deactivate')).toEqual([]);
  });

  it('keeps the fonts when the answer is no', async () => {
    const { fake, ui, watcher } = setup([4, 9], false);
    await watcher.closed(doc);
    expect(ui.confirm).toHaveBeenCalledOnce();
    expect(fake.calls('deactivate')).toEqual([]);
  });

  it('asks nothing when the offer is empty', async () => {
    const { fake, ui, watcher } = setup([]);
    await watcher.closed(doc);
    expect(ui.confirm).not.toHaveBeenCalled();
    expect(fake.calls('deactivate')).toEqual([]);
  });

  it('says "not running" once and does nothing else when the app is not running', async () => {
    const { fake, ui, watcher } = setup([4]);
    fake.listening = false;
    await watcher.opened(doc, [inter]);
    await watcher.missing(doc, [inter]);
    await watcher.closed(doc);
    expect(ui.notify).toHaveBeenCalledOnce();
    expect(ui.notify).toHaveBeenCalledWith(NOT_RUNNING);
    expect(ui.confirm).not.toHaveBeenCalled();
    expect(fake.requests).toEqual([]);
  });

  it('says "not running" once when the token is refused, and asks nothing', async () => {
    const { fake, ui, watcher } = setup([4]);
    fake.discovery = JSON.stringify({ url: FAKE_URL, token: 'old' });
    await watcher.opened(doc, [inter]);
    await watcher.closed(doc);
    expect(ui.notify).toHaveBeenCalledOnce();
    expect(ui.notify).toHaveBeenCalledWith(NOT_RUNNING);
    expect(ui.confirm).not.toHaveBeenCalled();
    expect(fake.calls('deactivate')).toEqual([]);
  });

  it('says it again after the app came back and went away again', async () => {
    const { fake, ui, watcher } = setup();
    fake.listening = false;
    await watcher.opened(doc, [inter]);
    fake.listening = true;
    await watcher.opened(doc, [inter]);
    fake.listening = false;
    await watcher.opened(doc, [inter]);
    expect(ui.notify).toHaveBeenCalledTimes(2);
  });

  it('keeps "not running" said once while the host switches between documents', async () => {
    const { fake, ui, watcher } = setup([4]);
    fake.listening = false;
    const other = { key: 'InDesign: Other.indd #8', name: 'Other.indd' };
    await watcher.opened(doc, [lora]);
    await watcher.opened(other, []);
    for (let i = 0; i < 3; i++) {
      // Nothing missing in Other: no call, which must not count as the app answering.
      await watcher.missing(other, []);
      await watcher.missing(doc, [lora]);
    }
    await watcher.closed(other);
    expect(ui.notify).toHaveBeenCalledOnce();
    expect(ui.notify).toHaveBeenCalledWith(NOT_RUNNING);
  });

  it('retries a missing report whose call failed', async () => {
    const { fake, watcher } = setup();
    fake.listening = false;
    await watcher.missing(doc, [lora]);
    fake.listening = true;
    await watcher.missing(doc, [lora]);
    expect(fake.calls('fonts_needed')).toEqual([{ fonts: [lora], document: doc.key }]);
  });

  it('handles events in the order the host sent them', async () => {
    const { fake, watcher } = setup([4]);
    await Promise.all([watcher.opened(doc, [inter]), watcher.closed(doc)]);
    expect(fake.requests.map((r) => r.method)).toEqual([
      'fonts_needed',
      'document_closed',
      'deactivate',
    ]);
  });

  it("shows the app's error message and never throws into the host", async () => {
    const fake = fakeEndpoint({
      document_closed: () => ({ status: 503, body: { code: 'busy', message: 'Busy.' } }),
    });
    const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    const watcher = new DocumentWatcher(new FmClient(fake), ui);
    await expect(watcher.closed(doc)).resolves.toBeUndefined();
    expect(ui.notify).toHaveBeenCalledWith('Busy.');
  });
});

describe('deactivateQuestion', () => {
  it('counts the fonts', () => {
    expect(deactivateQuestion(1, 'A.psd')).toBe('Deactivate 1 font used only by A.psd?');
    expect(deactivateQuestion(3, 'A.psd')).toBe('Deactivate 3 fonts used only by A.psd?');
  });

  it('counts only when there are no names (an app older than E30.5)', () => {
    expect(deactivateQuestion(3, 'brand.ai', [])).toBe('Deactivate 3 fonts used only by brand.ai?');
  });

  it('names one font', () => {
    expect(deactivateQuestion(1, 'brand.ai', ['Actor'])).toBe(
      'Deactivate 1 font used only by brand.ai? Actor',
    );
  });

  it('names three fonts', () => {
    expect(deactivateQuestion(3, 'brand.ai', ['Alegreya Bold', 'Alegreya Italic', 'Actor'])).toBe(
      'Deactivate 3 fonts used only by brand.ai? Alegreya Bold, Alegreya Italic, Actor',
    );
  });

  it('names the first eight of nine fonts and counts the rest', () => {
    const names = ['F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9'];
    expect(deactivateQuestion(9, 'brand.ai', names)).toBe(
      'Deactivate 9 fonts used only by brand.ai? F1, F2, F3, F4, F5, F6, F7, F8 and 1 more',
    );
  });

  it('names exactly eight fonts without "more"', () => {
    const names = ['F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8'];
    expect(deactivateQuestion(8, 'brand.ai', names)).toBe(
      'Deactivate 8 fonts used only by brand.ai? F1, F2, F3, F4, F5, F6, F7, F8',
    );
  });
});

describe('offeredNames', () => {
  it("labels each offered face by fontLabel's rule, in offer order", () => {
    expect(
      offeredNames({
        offer: [2, 1],
        offered: [
          { face: 2, family: 'Alegreya', style: 'Italic', postscriptName: 'Alegreya-Italic' },
          { face: 1, family: 'Actor', style: 'Regular', postscriptName: null },
        ],
      }),
    ).toEqual(['Alegreya-Italic', 'Actor Regular']);
  });

  it('is empty without offered, or when offered does not match the offer', () => {
    expect(offeredNames({ offer: [1] })).toEqual([]);
    expect(offeredNames({ offer: [1, 2], offered: [] })).toEqual([]);
  });
});

describe('missing fonts message', () => {
  const unmatchedLora = () => ({ matched: [], unmatched: [lora], missing: 1, outcomes: [] });

  it('says one "N fonts missing" message with the names when the option is on', async () => {
    const fake = fakeEndpoint({
      fonts_needed: () => ({
        matched: [],
        unmatched: [lora, { family: 'Roboto Slab', style: 'Light' }, lora],
        missing: 3,
        outcomes: [],
      }),
    });
    const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    const watcher = new DocumentWatcher(new FmClient(fake), ui, { reportMissing: true });
    await watcher.opened(doc, [inter, lora]);
    expect(ui.notify).toHaveBeenCalledTimes(1);
    expect(ui.notify).toHaveBeenCalledWith('2 fonts missing: Lora-Bold, Roboto Slab Light');
  });

  it('says nothing when every font matched, or when the option is off', async () => {
    const all = fakeEndpoint({
      fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    });
    const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    await new DocumentWatcher(new FmClient(all), ui, { reportMissing: true }).opened(doc, [inter]);
    const some = fakeEndpoint({ fonts_needed: unmatchedLora });
    await new DocumentWatcher(new FmClient(some), ui).opened(doc, [lora]);
    expect(ui.notify).not.toHaveBeenCalled();
  });

  it('names a font by PostScript name, then full name, then family and style', () => {
    expect(fontLabel({ postscriptName: 'A-B', fullName: 'A B' })).toBe('A-B');
    expect(fontLabel({ fullName: 'A B', family: 'A' })).toBe('A B');
    expect(fontLabel({ family: 'A', style: 'Bold' })).toBe('A Bold');
    expect(fontLabel({})).toBe('Unnamed font');
    expect(missingFontsMessage([{ family: 'A' }])).toBe('1 font missing: A');
  });
  it('says what the app downloaded, then counts only what is still missing', async () => {
    const alegreya = { family: 'Alegreya', style: 'Bold' };
    const roboto = { fullName: 'Roboto Slab Light' };
    const fake = fakeEndpoint({
      fonts_needed: () => ({
        matched: [{ font: alegreya, face: 12, matchedBy: 'family' }],
        unmatched: [roboto],
        missing: 1,
        outcomes: [{ face: 12, outcome: 'done' }],
        fetched: [{ font: alegreya, catalog: 'googleFonts', family: 'Alegreya', faces: [12, 13] }],
      }),
    });
    const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    const watcher = new DocumentWatcher(new FmClient(fake), ui, { reportMissing: true });
    await watcher.opened(doc, [alegreya, roboto]);
    expect(vi.mocked(ui.notify).mock.calls).toEqual([
      ['Downloaded 1 family from Google Fonts: Alegreya'],
      ['1 font missing: Roboto Slab Light'],
    ]);
    // Fonts the host reports missing later go the same way.
    await watcher.missing(doc, [alegreya]);
    expect(ui.notify).toHaveBeenCalledTimes(3);
    expect(ui.notify).toHaveBeenLastCalledWith('Downloaded 1 family from Google Fonts: Alegreya');
  });

  it('says nothing about downloads when nothing was fetched, or the app sends no list', async () => {
    const ui: PluginUi = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    const none = fakeEndpoint({
      fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [], fetched: [] }),
    });
    await new DocumentWatcher(new FmClient(none), ui).opened(doc, [inter]);
    const older = fakeEndpoint({
      fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    });
    await new DocumentWatcher(new FmClient(older), ui).opened(doc, [inter]);
    expect(ui.notify).not.toHaveBeenCalled();
  });

  it('counts families once per catalog and names each catalog', () => {
    const f = (family: string, catalog: 'googleFonts' | 'fontshare') => ({
      font: { family },
      catalog,
      family,
      faces: [1],
    });
    expect(downloadedFontsMessage([])).toBe('');
    expect(
      downloadedFontsMessage([
        f('Alegreya', 'googleFonts'),
        f('Alegreya', 'googleFonts'),
        f('Lato', 'googleFonts'),
        f('General Sans', 'fontshare'),
      ]),
    ).toBe(
      'Downloaded 2 families from Google Fonts: Alegreya, Lato. Downloaded 1 family from Fontshare: General Sans',
    );
  });
});
