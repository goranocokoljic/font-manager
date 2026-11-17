import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { CatalogSampleTextRequest } from '../api/types';
import { useCatalogSamplesStore } from './catalogSamples';

let calls: CatalogSampleTextRequest[];
let fail: boolean;
const SERBIAN = 'čađavog lica';

beforeEach(async () => {
  setActivePinia(createPinia());
  calls = [];
  fail = false;
  // Google Fonts on, with the mock's four families cached.
  const mock = createMockClient({ catalog: 'offline' });
  const client: FmClient = {
    ...mock,
    catalogSampleTexts: (req) => {
      calls.push(structuredClone(req));
      return fail
        ? Promise.reject({ code: 'io', message: 'offline', detail: null })
        : mock.catalogSampleTexts(req);
    },
  };
  setClient(client);
});

afterEach(() => setClient(undefined));

describe('catalog samples store', () => {
  it('batches the asks of one tick into one call per catalog and sample', async () => {
    const samples = useCatalogSamplesStore();
    samples.want('googleFonts', 'Lora', SERBIAN);
    samples.want('googleFonts', 'Inter', SERBIAN);
    samples.want('googleFonts', 'Lora', SERBIAN);
    samples.want('googleFonts', 'Lora', 'Love');
    samples.want('googleFonts', 'Lora', '');
    await flushPromises();
    expect(calls).toEqual([
      { id: 'googleFonts', families: ['Lora', 'Inter'], text: SERBIAN },
      { id: 'googleFonts', families: ['Lora'], text: 'Love' },
    ]);
    expect(samples.get('googleFonts', 'Lora', SERBIAN)?.missing).toEqual(['č', 'đ']);
    expect(samples.get('googleFonts', 'Inter', SERBIAN)?.missing).toEqual([]);
    expect(samples.get('fontshare', 'Lora', SERBIAN)).toBeUndefined();

    // Answered: not asked again.
    samples.want('googleFonts', 'Lora', SERBIAN);
    await flushPromises();
    expect(calls).toHaveLength(2);
  });

  it('forgets a failed ask, so a later one tries again', async () => {
    const samples = useCatalogSamplesStore();
    fail = true;
    samples.want('googleFonts', 'Lora', SERBIAN);
    await flushPromises();
    expect(samples.get('googleFonts', 'Lora', SERBIAN)).toBeUndefined();
    fail = false;
    samples.want('googleFonts', 'Lora', SERBIAN);
    await flushPromises();
    expect(calls).toHaveLength(2);
    expect(samples.get('googleFonts', 'Lora', SERBIAN)?.missing).toEqual(['č', 'đ']);
  });
});
