import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useCatalogsStore } from '../stores/catalogs';
import AdobeFontsNote from './AdobeFontsNote.vue';
import OnlineCatalogsSidebar from './OnlineCatalogsSidebar.vue';

let wrapper: VueWrapper | undefined;

/** A mock client that records the name and arguments of every API call. */
function recordingClient(overrides: Partial<FmClient> = {}) {
  const calls: [string, unknown[]][] = [];
  const base = { ...createMockClient(), ...overrides };
  const client = new Proxy(base, {
    get(target, key, receiver) {
      const value = Reflect.get(target, key, receiver);
      if (typeof value !== 'function') return value;
      return (...args: unknown[]) => {
        calls.push([String(key), args]);
        return value.apply(target, args);
      };
    },
  });
  setClient(client);
  return calls;
}

function mountWith(component: typeof AdobeFontsNote | typeof OnlineCatalogsSidebar) {
  const pinia = createPinia();
  setActivePinia(pinia);
  wrapper = mount(component, { global: { plugins: [pinia] }, attachTo: document.body });
  return wrapper;
}

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('AdobeFontsNote', () => {
  it('explains why Adobe Fonts stay in Creative Cloud', async () => {
    recordingClient();
    const note = mountWith(AdobeFontsNote);
    await flushPromises();
    expect(note.get('#adobe-fonts-title').text()).toBe('Adobe Fonts');
    expect(note.text()).toContain(
      'Adobe Fonts are delivered encrypted through Creative Cloud, so no third-party app can manage them. Activate them in the Creative Cloud app.',
    );
    const link = note.get('a');
    expect(link.text()).toBe('Open Adobe Fonts');
    expect(link.attributes('href')).toBe('https://fonts.adobe.com');
  });

  it('makes no API call besides the link, which opens fonts.adobe.com', async () => {
    const calls = recordingClient({ openUrl: async () => undefined });
    const note = mountWith(AdobeFontsNote);
    await flushPromises();
    expect(calls, 'showing the note reads nothing').toEqual([]);

    await note.get('a').trigger('click');
    await flushPromises();
    expect(calls).toEqual([['openUrl', ['https://fonts.adobe.com']]]);
  });

  it('shows the error when the browser cannot be opened', async () => {
    recordingClient({
      openUrl: async () => {
        throw { code: 'io', message: 'The browser could not be opened.', detail: null };
      },
    });
    const note = mountWith(AdobeFontsNote);
    await note.get('a').trigger('click');
    await flushPromises();
    expect(note.get('[role="alert"]').text()).toContain('The browser could not be opened.');
  });
});

describe('OnlineCatalogsSidebar', () => {
  it('lists Adobe Fonts and opens its note without an API call', async () => {
    const calls = recordingClient();
    const sidebar = mountWith(OnlineCatalogsSidebar);
    await flushPromises();
    expect(sidebar.get('h2').text()).toBe('Online catalogs');
    const entry = sidebar.get('[data-testid="catalog-adobe-fonts"]');
    expect(entry.text()).toBe('Adobe Fonts');
    expect(entry.attributes('aria-current')).toBeUndefined();

    await entry.trigger('click');
    expect(useCatalogsStore().open).toBe('adobeFonts');
    expect(entry.attributes('aria-current')).toBe('true');
    await flushPromises();
    expect(calls).toEqual([]);
  });
});
