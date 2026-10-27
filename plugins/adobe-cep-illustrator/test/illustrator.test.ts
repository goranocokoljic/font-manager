import { DocumentWatcher, FmClient, NOT_RUNNING } from '@fm/plugin-shared';
import { fakeEndpoint } from '@fm/plugin-shared/testing';
import { describe, expect, it, vi } from 'vitest';
import { cepEvents, cepExtendScript, scriptCall } from '../src/cep';
import {
  DOCUMENT_EVENTS,
  SYNC_EVERY_MS,
  illustratorFonts,
  watchIllustrator,
} from '../src/illustrator';
import { extendScriptUi } from '../src/ui';
import { type MockDocument, mockDocument, mockIllustrator } from './mock-illustrator';

const minion = { name: 'MinionPro-Regular', family: 'Minion Pro', style: 'Regular' };
const lora = { name: 'Lora-Bold', family: 'Lora', style: 'Bold' };
const inter = { name: 'Inter-Italic', family: 'Inter', style: 'Italic' };

const poster = () => mockDocument('Poster.ai', '/work', [[minion, lora, minion], [inter], []]);

function setup(open: MockDocument[] = [], offer: number[] = []) {
  const fake = fakeEndpoint({
    fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    document_closed: () => ({ offer }),
    deactivate: () => [],
  });
  const host = mockIllustrator(open);
  const ticks: [number, () => void][] = [];
  const script = cepExtendScript(host.cep);
  const watcher = new DocumentWatcher(new FmClient(fake), extendScriptUi(script));
  const watch = watchIllustrator(script, cepEvents(host.cep), watcher, (ms, run) =>
    ticks.push([ms, run]),
  );
  return { fake, ticks, watch, ...host };
}

describe('watchIllustrator', () => {
  it('compares the open documents on document (de)activation and on a timer', () => {
    const { listeners, ticks } = setup();
    expect([...listeners.keys()].sort()).toEqual([...DOCUMENT_EVENTS].sort());
    expect(ticks.map(([ms]) => ms)).toEqual([SYNC_EVERY_MS]);
  });

  it('sends one fonts_needed with the fonts of every text frame on open', async () => {
    const { fake, openDocument, watch } = setup();
    openDocument(poster());
    await watch.sync();
    expect(fake.calls('fonts_needed')).toEqual([
      {
        document: 'Illustrator: /work/Poster.ai',
        fonts: [
          { postscriptName: 'MinionPro-Regular', family: 'Minion Pro', style: 'Regular' },
          { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' },
          { postscriptName: 'Inter-Italic', family: 'Inter', style: 'Italic' },
        ],
      },
    ]);
  });

  it('sends a document once, not on every activation or tick', async () => {
    const { fake, fire, openDocument, ticks, watch } = setup();
    openDocument(poster());
    for (let i = 0; i < 3; i++) {
      fire('documentAfterActivate');
      fire('documentAfterDeactivate');
      ticks[0][1]();
    }
    await watch.sync();
    expect(fake.calls('fonts_needed')).toHaveLength(1);
  });

  it('sends each open document on its own, and an unsaved one by its name', async () => {
    const { fake, openDocument, watch } = setup();
    openDocument(poster());
    openDocument(mockDocument('Untitled-1', '', [[lora]]));
    await watch.sync();
    expect(fake.calls('fonts_needed').map((c) => (c as { document: string }).document)).toEqual([
      'Illustrator: /work/Poster.ai',
      'Illustrator: Untitled-1',
    ]);
  });

  it('picks up the documents open before the extension loaded', async () => {
    const { fake, watch } = setup([poster()]);
    await watch.sync();
    expect(fake.calls('fonts_needed')).toHaveLength(1);
  });

  it('sends nothing for a document without text', async () => {
    const { fake, openDocument, watch } = setup();
    openDocument(mockDocument('Logo.ai', '/work', []));
    await watch.sync();
    expect(fake.calls('fonts_needed')).toEqual([]);
  });

  it('tries again after ExtendScript fails', async () => {
    const { fake, openDocument, state, watch } = setup();
    state.broken = true;
    openDocument(poster());
    await watch.sync();
    expect(fake.calls('fonts_needed')).toEqual([]);
    state.broken = false;
    await watch.sync();
    expect(fake.calls('fonts_needed')).toHaveLength(1);
  });

  it.each([
    ['Deactivate', 1],
    ['Keep them', 0],
  ])('on close calls document_closed and offers to deactivate (%s)', async (press, calls) => {
    const doc = poster();
    const { fake, close, dialogs, state, watch } = setup([doc], [4, 9]);
    state.press = press;
    await watch.sync();
    close(doc);
    await watch.sync();
    expect(fake.calls('document_closed')).toEqual([{ document: 'Illustrator: /work/Poster.ai' }]);
    expect(dialogs).toEqual([
      {
        title: 'Font Manager',
        message: 'Deactivate 2 fonts used only by Poster.ai?',
        buttons: ['Keep them', 'Deactivate'],
      },
    ]);
    expect(fake.calls('deactivate')).toEqual(
      calls ? [{ target: { kind: 'faces', faces: [4, 9] } }] : [],
    );
  });

  it('asks nothing when the app offers nothing', async () => {
    const doc = poster();
    const { fake, close, dialogs, watch } = setup([doc], []);
    await watch.sync();
    close(doc);
    await watch.sync();
    expect(fake.calls('document_closed')).toHaveLength(1);
    expect(dialogs).toEqual([]);
  });

  it('says "not running" once when the app is not running', async () => {
    const doc = poster();
    const { fake, alerts, close, openDocument, watch } = setup([], [4]);
    fake.listening = false;
    openDocument(doc);
    openDocument(mockDocument('Second.ai', '/work', [[lora]]));
    await watch.sync();
    close(doc);
    await watch.sync();
    await vi.waitFor(() => expect(alerts).toEqual([NOT_RUNNING]));
  });
});

describe('host.jsx', () => {
  it('skips a text run Illustrator cannot describe', () => {
    const broken = () => {
      throw new Error('missing font');
    };
    const { run } = mockIllustrator([mockDocument('A.ai', '/w', [[broken, lora]])]);
    expect(illustratorFonts(run(scriptCall('fmDocumentFonts', '/w/A.ai')))).toEqual([
      { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' },
    ]);
  });

  it('answers null for a document that is no longer open', () => {
    const { run } = mockIllustrator([poster()]);
    expect(illustratorFonts(run(scriptCall('fmDocumentFonts', '/work/Gone.ai')))).toBeNull();
  });

  it('keeps quotes, backslashes and line separators in names intact', () => {
    const odd = 'Q"uo\\te\u2028Sans';
    const doc = mockDocument(`${odd}.ai`, '', [[{ name: odd, family: odd, style: '' }]]);
    const { run } = mockIllustrator([doc]);
    expect(JSON.parse(run('fmOpenDocuments()'))).toEqual([{ key: `${odd}.ai`, name: `${odd}.ai` }]);
    expect(illustratorFonts(run(scriptCall('fmDocumentFonts', `${odd}.ai`)))).toEqual([
      { postscriptName: odd, family: odd, style: undefined },
    ]);
  });
});
