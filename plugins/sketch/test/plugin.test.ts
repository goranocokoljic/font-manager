import { COPY_A_FONT, type SketchFontSwitchHost } from '@fm/font-switch';
import { NOT_RUNNING } from '@fm/plugin-shared';
import { FAKE_TOKEN, FAKE_URL, type FakeEndpoint, fakeEndpoint } from '@fm/plugin-shared/testing';
import { describe, expect, it, vi } from 'vitest';
import { nativeFonts } from '../src/font-switch';
import { createPlugin } from '../src/plugin';
import {
  type MockDocument,
  mockAlerts,
  mockDocument,
  mockDom,
  mockSettings,
  mockUi,
  textLayer,
} from './mock-sketch';

const poster = mockDocument(
  'D3C1A1F0-6E7B-4F0E-9C1A-2B7E5D0A9F11',
  'Poster',
  [
    textLayer('Inter-Regular', 'Inter'),
    textLayer('Lora-Bold', 'Lora'),
    textLayer('Inter-Regular', 'Inter'),
    textLayer(null, 'Roboto Slab'),
    textLayer(null, null),
  ],
  '/Users/ana/Designs/Poster.sketch',
);
const key = `Sketch: ${poster.doc.id}`;
const inter = { postscriptName: 'Inter-Regular', family: 'Inter' };
const lora = { postscriptName: 'Lora-Bold', family: 'Lora' };
const slab = { family: 'Roboto Slab' };

interface Options {
  unmatched?: unknown[];
  offer?: number[];
  answer?: number;
  docs?: MockDocument[];
  settings?: ReturnType<typeof mockSettings>;
  fake?: FakeEndpoint;
  fontSwitch?: SketchFontSwitchHost;
  clipboard?: string | null;
}

function setup(o: Options = {}) {
  const fake =
    o.fake ??
    fakeEndpoint({
      fonts_needed: () => ({
        matched: [],
        unmatched: o.unmatched ?? [],
        missing: (o.unmatched ?? []).length,
        outcomes: [],
      }),
      document_closed: () => ({ offer: o.offer ?? [] }),
      deactivate: () => [],
    });
  const ui = mockUi();
  const alerts = mockAlerts(o.answer ?? 0);
  const release = vi.fn();
  const keepAlive = vi.fn(() => release);
  const dom = mockDom(o.docs ?? [poster]);
  const plugin = createPlugin({
    dom,
    ui,
    settings: o.settings ?? mockSettings(),
    newAlert: alerts.newAlert,
    endpoint: fake,
    keepAlive,
    fontSwitch: o.fontSwitch ?? { selectedLayers: () => [], fonts: () => [] },
    clipboardText: () => o.clipboard ?? null,
  });
  return { fake, ui, alerts, keepAlive, release, dom, plugin };
}

const context = (doc: MockDocument) => ({ actionContext: { document: doc.native } });

describe('OpenDocument', () => {
  it('sends one fonts_needed with the fonts of every text layer and the document id', async () => {
    const { fake, dom, plugin } = setup();
    await plugin.onOpenDocument(context(poster));
    expect(dom.find).toHaveBeenCalledWith('Text', poster.doc);
    expect(fake.calls('fonts_needed')).toEqual([{ fonts: [inter, lora, slab], document: key }]);
  });

  it('finds the app through endpoint.json and sends the bearer token to 127.0.0.1', async () => {
    const { fake, plugin } = setup();
    await plugin.onOpenDocument(context(poster));
    expect(fake.requests).toHaveLength(1);
    expect(fake.requests[0].url).toBe(`${FAKE_URL}/v1/fonts_needed`);
    expect(fake.requests[0].headers.Authorization).toBe(`Bearer ${FAKE_TOKEN}`);
  });

  it('sends nothing for a document without text', async () => {
    const empty = mockDocument('E-1', 'Empty', []);
    const { fake, ui, plugin } = setup({ docs: [empty] });
    await plugin.onOpenDocument(context(empty));
    expect(fake.requests).toEqual([]);
    expect(ui.message).not.toHaveBeenCalled();
  });

  it('keeps the script alive until the call is done', async () => {
    const { keepAlive, release, plugin } = setup();
    const done = plugin.onOpenDocument(context(poster));
    expect(keepAlive).toHaveBeenCalledTimes(1);
    expect(release).not.toHaveBeenCalled();
    await done;
    expect(release).toHaveBeenCalledTimes(1);
  });

  it('does nothing for an action without a document', async () => {
    const { fake, ui, release, plugin } = setup();
    await plugin.onOpenDocument({});
    expect(fake.requests).toEqual([]);
    expect(ui.message).not.toHaveBeenCalled();
    expect(release).toHaveBeenCalledTimes(1);
  });

  it('says what went wrong when Sketch cannot read the document, and sends nothing', async () => {
    const { fake, ui, release, plugin } = setup();
    await plugin.onOpenDocument({ actionContext: { document: { kind: 'other' } } });
    expect(fake.requests).toEqual([]);
    expect(ui.message).toHaveBeenCalledWith('Font Manager: Error: not a document');
    expect(release).toHaveBeenCalledTimes(1);
  });
});

describe('missing fonts', () => {
  it('shows one "N fonts missing" message with their names', async () => {
    const { ui, plugin } = setup({ unmatched: [lora, slab] });
    await plugin.onOpenDocument(context(poster));
    expect(ui.message).toHaveBeenCalledTimes(1);
    expect(ui.message).toHaveBeenCalledWith('2 fonts missing: Lora-Bold, Roboto Slab');
  });

  it('shows nothing when the app has every font', async () => {
    const { ui, plugin } = setup();
    await plugin.onOpenDocument(context(poster));
    expect(ui.message).not.toHaveBeenCalled();
  });
});

describe('CloseDocument', () => {
  it('calls document_closed and, on Deactivate, deactivate with the offered faces', async () => {
    const { fake, alerts, plugin } = setup({ offer: [4, 9], answer: 0 });
    await plugin.onCloseDocument(context(poster));
    expect(fake.calls('document_closed')).toEqual([{ document: key }]);
    expect(alerts.shown).toHaveLength(1);
    expect(alerts.shown[0].informative).toBe('Deactivate 2 fonts used only by Poster?');
    expect(alerts.shown[0].buttons).toEqual(['Deactivate', 'Keep']);
    expect(fake.calls('deactivate')).toEqual([{ target: { kind: 'faces', faces: [4, 9] } }]);
  });

  it('keeps the fonts when the answer is Keep', async () => {
    const { fake, alerts, plugin } = setup({ offer: [4], answer: 1 });
    await plugin.onCloseDocument(context(poster));
    expect(alerts.shown[0].informative).toBe('Deactivate 1 font used only by Poster?');
    expect(fake.calls('deactivate')).toEqual([]);
  });

  it('asks nothing when the app offers no fonts', async () => {
    const { fake, alerts, plugin } = setup({ offer: [] });
    await plugin.onCloseDocument(context(poster));
    expect(fake.calls('document_closed')).toHaveLength(1);
    expect(alerts.shown).toEqual([]);
  });

  it('names a document without a window title by its file', async () => {
    const saved = mockDocument('S-1', null, [], '/Users/ana/Logo.sketch');
    const { alerts, plugin } = setup({ docs: [saved], offer: [1], answer: 1 });
    await plugin.onCloseDocument(context(saved));
    expect(alerts.shown[0].informative).toBe('Deactivate 1 font used only by Logo.sketch?');
  });
});

describe('app not running', () => {
  it('says "Font Manager is not running" once and does nothing else', async () => {
    const settings = mockSettings();
    const fake = fakeEndpoint();
    fake.discovery = null;
    // Every action runs in a fresh script: a new plugin, the same session.
    const first = setup({ fake, settings, offer: [1] });
    await first.plugin.onOpenDocument(context(poster));
    const second = setup({ fake, settings, offer: [1] });
    await second.plugin.onCloseDocument(context(poster));
    await second.plugin.onOpenDocument(context(poster));
    expect(first.ui.message).toHaveBeenCalledTimes(1);
    expect(first.ui.message).toHaveBeenCalledWith(NOT_RUNNING);
    expect(second.ui.message).not.toHaveBeenCalled();
    expect(second.alerts.shown).toEqual([]);
    expect(fake.requests).toEqual([]);
  });

  it('says it again after the app answered in between', async () => {
    const settings = mockSettings();
    const fake = fakeEndpoint({
      fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    });
    fake.listening = false;
    const a = setup({ fake, settings });
    await a.plugin.onOpenDocument(context(poster));
    fake.listening = true;
    const b = setup({ fake, settings });
    await b.plugin.onOpenDocument(context(poster));
    fake.listening = false;
    const c = setup({ fake, settings });
    await c.plugin.onOpenDocument(context(poster));
    expect(a.ui.message).toHaveBeenCalledWith(NOT_RUNNING);
    expect(b.ui.message).not.toHaveBeenCalled();
    expect(c.ui.message).toHaveBeenCalledWith(NOT_RUNNING);
  });

  it('talks only to 127.0.0.1: an endpoint.json naming another host is not followed', async () => {
    const fake = fakeEndpoint();
    fake.discovery = JSON.stringify({ url: 'http://192.168.1.20:49152', token: FAKE_TOKEN });
    const fetch = vi.spyOn(fake, 'fetch');
    const { ui, plugin } = setup({ fake });
    await plugin.onOpenDocument(context(poster));
    expect(fetch).not.toHaveBeenCalled();
    expect(ui.message).toHaveBeenCalledWith(NOT_RUNNING);
  });
});

describe('Apply Copied Font (Font Switch)', () => {
  const payload = {
    postscriptName: 'Inter-Regular',
    family: 'Inter',
    style: 'Regular',
    fileUrl: 'file:///Users/ana/Fonts/Inter-Regular.otf',
  };
  const fonts = [{ postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' }];
  const selected = () => {
    const setFontPostscriptName = vi.fn<(name: string) => void>();
    return {
      layer: { type: 'Text', sketchObject: { setFontPostscriptName } },
      setFontPostscriptName,
    };
  };

  it('gives the selected text layers the font copied in the app', async () => {
    const text = selected();
    const { ui, fake, plugin } = setup({
      clipboard: JSON.stringify(payload),
      fontSwitch: { selectedLayers: () => [text.layer], fonts: () => fonts },
    });
    await plugin.onApplyCopiedFont();
    expect(text.setFontPostscriptName).toHaveBeenCalledWith('Inter-Regular');
    expect(ui.message).toHaveBeenCalledWith('Inter-Regular applied');
    // Font Switch applies; it does not call the app.
    expect(fake.requests).toEqual([]);
  });

  it.each<[string, string | null, boolean, string]>([
    ['no text is selected', 'Inter-Regular', false, 'Select some text first'],
    ['the clipboard holds no font', 'two words', true, COPY_A_FONT],
    ['the clipboard is empty', null, true, COPY_A_FONT],
    ['the Mac does not have the font', 'Lora-Bold', true, 'Lora-Bold is not active'],
  ])('says so when %s', async (_, clipboard, withText, message) => {
    const text = selected();
    const { ui, plugin } = setup({
      clipboard,
      fontSwitch: { selectedLayers: () => (withText ? [text.layer] : []), fonts: () => fonts },
    });
    await plugin.onApplyCopiedFont();
    expect(text.setFontPostscriptName).not.toHaveBeenCalled();
    expect(ui.message).toHaveBeenCalledWith(message);
  });

  it('says what went wrong when Sketch throws', async () => {
    const { ui, plugin } = setup({
      clipboard: 'Inter-Regular',
      fontSwitch: {
        selectedLayers: () => {
          throw new Error('no document');
        },
        fonts: () => fonts,
      },
    });
    await plugin.onApplyCopiedFont();
    expect(ui.message).toHaveBeenCalledWith('Font Manager: Error: no document');
  });
});

describe('nativeFonts', () => {
  const array = <T>(items: T[]) => ({
    count: () => items.length,
    objectAtIndex: (i: number) => items[i],
  });

  it('lists every member of every family with its PostScript name and style', () => {
    const members: Record<string, unknown[][]> = {
      Inter: [
        ['Inter-Regular', 'Regular', 5, 0],
        ['Inter-Bold', 'Bold', 9, 2],
      ],
      Lora: [['Lora-Italic', 'Italic', 5, 1]],
    };
    const manager = {
      availableFontFamilies: () => array(['Inter', 'Lora', 'Gone']),
      availableMembersOfFontFamily: (family: unknown) => {
        const list = members[String(family)];
        return list ? array(list.map(array)) : null;
      },
    };
    expect(nativeFonts(manager)).toEqual([
      { postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' },
      { postscriptName: 'Inter-Bold', family: 'Inter', style: 'Bold' },
      { postscriptName: 'Lora-Italic', family: 'Lora', style: 'Italic' },
    ]);
  });
});
