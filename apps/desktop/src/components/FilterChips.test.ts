import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { Filters } from '../api/types';
import { useCollectionsStore } from '../stores/collections';
import { useLibraryStore } from '../stores/library';
import { useSearchStore } from '../stores/search';
import FilterChips from './FilterChips.vue';

let wrapper: VueWrapper | undefined;
let client: FmClient;

async function mountChips(filters: Filters = {}) {
  const pinia = createPinia();
  setActivePinia(pinia);
  client = createMockClient();
  setClient(client);
  await useLibraryStore().load();
  useSearchStore().setFilters(filters);
  wrapper = mount(FilterChips, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

const saveButton = (w: VueWrapper) =>
  w.findAll('button').find((b) => b.text() === 'Save as smart collection');
const nameField = (w: VueWrapper) => w.find('input[aria-label="Smart collection name"]');

async function saveAs(w: VueWrapper, name: string) {
  await saveButton(w)!.trigger('click');
  await flushPromises();
  await nameField(w).setValue(name);
  await w.get('form.save').trigger('submit');
  await flushPromises();
}

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('saving the filter chips as a smart collection', () => {
  it('is disabled while no filter is chosen, an open collection alone included', async () => {
    const w = await mountChips();
    expect(saveButton(w)!.attributes('disabled')).toBeDefined();
    const made = await client.createCollection({ name: 'Clients' });
    useSearchStore().setFilters({ collection: made.id });
    await flushPromises();
    expect(saveButton(w)!.attributes('disabled')).toBeDefined();
    useSearchStore().setFilters({ collection: made.id, slant: 'italic' });
    await flushPromises();
    expect(saveButton(w)!.attributes('disabled')).toBeUndefined();
  });

  it('saves exactly the chosen filters as the rules and lists the collection with its count', async () => {
    const chosen: Filters = {
      formats: ['ttf'],
      variable: true,
      supports: { kind: 'languages', codes: ['pl'] },
      weight: { min: 300, max: 700 },
      width: { min: 3, max: 7 },
    };
    const w = await mountChips(chosen);
    const create = vi.spyOn(client, 'createCollection');
    await saveAs(w, 'Polish variable');
    expect(create).toHaveBeenCalledTimes(1);
    expect(create).toHaveBeenCalledWith({ name: 'Polish variable', rules: chosen });
    const row = useCollectionsStore().rows.find((r) => r.collection.name === 'Polish variable');
    expect(row?.collection.rules).toEqual(chosen);
    // The count is the API's: the faces the rules find.
    const faces = await client.search({
      query: '',
      filters: chosen,
      page: { offset: 0, limit: 500 },
    });
    const expected = faces.families.reduce((n, f) => n + f.faces.length, 0);
    expect(expected).toBeGreaterThan(0);
    expect(row?.collection.faceCount).toBe(expected);
    expect(w.get('[role="status"]').text()).toBe('Saved “Polish variable” as a smart collection.');
    expect(nameField(w).exists()).toBe(false);
  });

  it('shows the API message next to the name field for an empty or taken name, and saves nothing', async () => {
    const w = await mountChips({ slant: 'italic' });
    await client.createCollection({ name: 'Italics' });
    const before = (await client.listCollections()).length;
    await saveAs(w, '  ');
    const field = nameField(w);
    const alert = w.get('form.save [role="alert"]');
    expect(alert.text()).toBe('Give the collection a name.');
    expect(field.attributes('aria-invalid')).toBe('true');
    expect(field.attributes('aria-describedby')).toBe(alert.attributes('id'));
    await field.setValue('Italics');
    await w.get('form.save').trigger('submit');
    await flushPromises();
    expect(w.get('form.save [role="alert"]').text()).toContain('already exists');
    expect((await client.listCollections()).length).toBe(before);
  });

  it('focuses the name field; Escape cancels and gives focus back to the button', async () => {
    const w = await mountChips({ slant: 'italic' });
    const create = vi.spyOn(client, 'createCollection');
    await saveButton(w)!.trigger('click');
    await flushPromises();
    expect(document.activeElement).toBe(nameField(w).element);
    await nameField(w).setValue('Never');
    await nameField(w).trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(nameField(w).exists()).toBe(false);
    expect(document.activeElement).toBe(saveButton(w)!.element);
    expect(create).not.toHaveBeenCalled();
  });
});
