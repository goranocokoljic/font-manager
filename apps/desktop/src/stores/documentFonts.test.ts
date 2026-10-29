import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockPickedDocuments, type MockOptions } from '../api/mock';
import { useActivationStore } from './activation';
import { useDocumentFontsStore } from './documentFonts';
import { useToastStore } from './toast';

let client: FmClient;

async function setup(options: MockOptions = {}) {
  client = createMockClient(options);
  setClient(client);
  await useActivationStore().load();
  return useDocumentFontsStore();
}

const statuses = (docs: ReturnType<typeof useDocumentFontsStore>) =>
  docs.report?.fonts.map((f) => [f.font.postscriptName, f.status]);

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => {
  mockPickedDocuments.length = 0;
  setClient(undefined);
});

describe('document fonts', () => {
  it('opens the picker filtered to the documents fm-core reads, then lists the statuses', async () => {
    const docs = await setup({ activeFaces: [6] });
    const pick = vi.spyOn(client, 'pickDocument');
    const check = vi.spyOn(client, 'checkDocumentFonts');
    mockPickedDocuments.push('/Users/demo/Brand brochure.indd');
    await docs.choose();
    expect(pick).toHaveBeenCalledWith("Check a document's fonts", 'Design documents');
    expect(check).toHaveBeenCalledWith({ path: '/Users/demo/Brand brochure.indd' });
    expect(docs.open).toBe(true);
    expect(statuses(docs)).toEqual([
      ['Futura-Medium', 'missing'],
      ['Inter-Regular', 'inLibrary'],
      ['JetBrainsMono-Regular', 'active'],
    ]);
    expect(docs.report?.toActivate).toEqual([1]);
  });

  it('activates the in-library faces for the session and follows the activation event', async () => {
    const docs = await setup({ activeFaces: [6] });
    mockPickedDocuments.push('/Users/demo/Brand brochure.indd');
    await docs.choose();
    const activate = vi.spyOn(client, 'activate');
    await docs.activate();
    await flushPromises();
    expect(activate).toHaveBeenCalledWith(
      expect.objectContaining({ target: { kind: 'faces', faces: [1] }, mode: 'session' }),
    );
    expect(statuses(docs)).toEqual([
      ['Futura-Medium', 'missing'],
      ['Inter-Regular', 'active'],
      ['JetBrainsMono-Regular', 'active'],
    ]);
    expect(docs.report?.toActivate).toEqual([]);
  });

  it('a file of another type or an unreadable document is a toast and no panel', async () => {
    const docs = await setup();
    const toasts = useToastStore();
    for (const path of ['/Users/demo/notes.txt', '/Users/demo/Damaged poster.psd']) {
      mockPickedDocuments.push(path);
      await docs.choose();
      expect(docs.open).toBe(false);
      expect(docs.report).toBeNull();
      expect(toasts.toast?.message).toBeTruthy();
    }
    mockPickedDocuments.push('/Users/demo/notes.txt');
    await docs.choose();
    expect(toasts.toast?.message).toContain('.indd');
  });

  it('a cancelled picker changes nothing', async () => {
    const docs = await setup();
    const check = vi.spyOn(client, 'checkDocumentFonts');
    mockPickedDocuments.push(null);
    await docs.choose();
    expect(check).not.toHaveBeenCalled();
    expect(docs.open).toBe(false);
  });

  it('closing stops following activation changes', async () => {
    const docs = await setup();
    mockPickedDocuments.push('/Users/demo/Brand brochure.indd');
    await docs.choose();
    await docs.close();
    const check = vi.spyOn(client, 'checkDocumentFonts');
    await client.activate({ target: { kind: 'faces', faces: [1] }, mode: 'session' });
    await flushPromises();
    expect(check).not.toHaveBeenCalled();
    expect(docs.open).toBe(false);
  });
});
