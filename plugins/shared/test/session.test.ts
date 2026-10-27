import { describe, expect, it, vi } from 'vitest';
import { FmClient, NOT_RUNNING } from '../src/client';
import { DocumentWatcher, type PluginUi, deactivateQuestion } from '../src/session';
import { FAKE_URL, fakeEndpoint } from '../src/testing';

const doc = { key: 'InDesign: Poster.indd #7', name: 'Poster.indd' };
const inter = { postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' };
const lora = { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' };

function setup(offer: number[] = [], answer = true) {
  const fake = fakeEndpoint({
    fonts_needed: () => ({ matched: [], unmatched: [], missing: 0, outcomes: [] }),
    document_closed: () => ({ offer }),
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
});
