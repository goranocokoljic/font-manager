import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useLibraryStore } from '../stores/library';
import { useSearchStore } from '../stores/search';
import { useToastStore } from '../stores/toast';
import { useActiveFilters } from '../search/activeFilters';
import FoundriesSidebar from './FoundriesSidebar.vue';

let wrapper: VueWrapper | undefined;

async function mountSidebar() {
  const pinia = createPinia();
  setActivePinia(pinia);
  setClient(createMockClient({ library: 'wall' }));
  await useLibraryStore().load();
  wrapper = mount(FoundriesSidebar, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

const fold = (w: VueWrapper) => w.get('button[aria-controls="fm-foundries-list"]');
const list = (w: VueWrapper) => w.get('#fm-foundries-list');
const row = (w: VueWrapper, name: string) => w.get(`button[data-foundry="${name}"]`);
const rowNames = (w: VueWrapper) =>
  w.findAll('button[data-foundry]').map((b) => b.attributes('data-foundry'));
const picker = () => document.body.querySelector<HTMLElement>('[data-testid="foundry-picker"]');
const options = () =>
  [...(picker()?.querySelectorAll<HTMLButtonElement>('button') ?? [])].map(
    (b) => b.querySelector('.name')!.textContent,
  );

beforeEach(() => localStorage.clear());
afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  document.body.innerHTML = '';
  setClient(undefined);
});

describe('the Foundries group', () => {
  it('starts folded with the count in its heading, and remembers being opened', async () => {
    const w = await mountSidebar();
    expect(fold(w).attributes('aria-expanded')).toBe('false');
    expect((list(w).element as HTMLElement).style.display).toBe('none');
    expect(w.get('[data-testid="section-count"]').text()).toContain('46');
    expect(w.get('[data-testid="section-count"] .sr-only').text()).toBe('46 foundries');

    await fold(w).trigger('click');
    expect(fold(w).attributes('aria-expanded')).toBe('true');
    expect(w.find('[data-testid="section-count"]').exists()).toBe(false);
    w.unmount();

    const again = await mountSidebar();
    expect(fold(again).attributes('aria-expanded')).toBe('true');
  });

  it('lists the ten largest with family counts and "All 46 foundries…"', async () => {
    const w = await mountSidebar();
    await fold(w).trigger('click');
    expect(rowNames(w)).toEqual([
      'Omnibus-Type',
      'Font Diner',
      'Impallari Type',
      'Sorkin Type',
      'Astigmatic',
      'Canonical',
      'Etcetera Type Co',
      'TYPETR',
      'Ana Sanfelippo',
      'Be',
    ]);
    expect(row(w, 'Omnibus-Type').get('.side-count').text()).toBe('5');
    expect(row(w, 'Omnibus-Type').get('.sr-only').text()).toBe('5 families');
    expect(w.get('[data-testid="all-foundries"]').text()).toBe('All 46 foundries…');
  });

  it('filters by a click, marks the row and names the chip; a second click clears it', async () => {
    const w = await mountSidebar();
    await fold(w).trigger('click');
    const search = useSearchStore();
    await row(w, 'Sorkin Type').trigger('click');
    await flushPromises();
    expect(search.foundry).toBe('Sorkin Type');
    expect(row(w, 'Sorkin Type').attributes('aria-pressed')).toBe('true');
    expect(row(w, 'Font Diner').attributes('aria-pressed')).toBe('false');
    expect(search.hits).toHaveLength(3);
    expect(useActiveFilters().labels.value).toEqual(['Foundry · Sorkin Type']);

    await row(w, 'Sorkin Type').trigger('click');
    await flushPromises();
    expect(search.foundry).toBeNull();
    expect(row(w, 'Sorkin Type').attributes('aria-pressed')).toBe('false');
    expect(useActiveFilters().labels.value).toEqual([]);
  });

  it('searches every foundry from "All N foundries…"; a chosen small one gets a marked row', async () => {
    const w = await mountSidebar();
    await fold(w).trigger('click');
    await w.get('[data-testid="all-foundries"]').trigger('click');
    await flushPromises();
    expect(options()).toHaveLength(46);
    const field = picker()!.querySelector('input')!;
    field.value = 'github';
    field.dispatchEvent(new Event('input'));
    await flushPromises();
    expect(options()).toEqual(['GitHub']);
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await flushPromises();
    expect(useSearchStore().foundry).toBe('GitHub');
    expect(picker()).toBeNull();
    expect(row(w, 'GitHub').attributes('aria-pressed')).toBe('true');
    expect(document.activeElement).toBe(w.get('[data-testid="all-foundries"]').element);
  });

  it('merges from the right-click menu, with an undo toast', async () => {
    const w = await mountSidebar();
    await fold(w).trigger('click');
    await row(w, 'Font Diner').trigger('contextmenu');
    await flushPromises();
    const menu = document.body.querySelector<HTMLElement>('[role="menu"]')!;
    expect(menu.getAttribute('aria-label')).toBe('Foundry Font Diner');
    const item = menu.querySelector<HTMLButtonElement>('[role="menuitem"]')!;
    // The name is the visible text (WCAG 2.5.3); the menu's name says which foundry.
    expect(item.hasAttribute('aria-label')).toBe(false);
    expect(item.textContent!.trim()).toBe('Merge into…');
    expect(document.activeElement).toBe(item);
    item.click();
    await flushPromises();
    expect(options()).not.toContain('Font Diner');
    const target = [...picker()!.querySelectorAll<HTMLButtonElement>('button')].find(
      (b) => b.querySelector('.name')!.textContent === 'Omnibus-Type',
    )!;
    target.click();
    await flushPromises();
    expect(rowNames(w)).not.toContain('Font Diner');
    expect(row(w, 'Omnibus-Type').get('.side-count').text()).toBe('8');
    expect(document.activeElement).toBe(row(w, 'Omnibus-Type').element);

    const toast = useToastStore();
    expect(toast.toast!.message).toBe('Merged “Font Diner” into “Omnibus-Type”.');
    toast.act();
    await flushPromises();
    expect(rowNames(w)).toContain('Font Diner');
    expect(row(w, 'Omnibus-Type').get('.side-count').text()).toBe('5');
  });

  it('keeps a right press from focusing the row, so the menu keeps the focus', async () => {
    const w = await mountSidebar();
    await fold(w).trigger('click');
    const right = new MouseEvent('mousedown', { button: 2, bubbles: true, cancelable: true });
    row(w, 'Be').element.dispatchEvent(right);
    expect(right.defaultPrevented).toBe(true);
    const left = new MouseEvent('mousedown', { button: 0, bubbles: true, cancelable: true });
    row(w, 'Be').element.dispatchEvent(left);
    expect(left.defaultPrevented).toBe(false);
  });

  it('closes the menu with Escape and gives the focus back to the row', async () => {
    const w = await mountSidebar();
    await fold(w).trigger('click');
    await row(w, 'Be').trigger('contextmenu');
    await flushPromises();
    const menu = document.body.querySelector<HTMLElement>('[role="menu"]')!;
    menu.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await flushPromises();
    expect(document.body.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(row(w, 'Be').element);
  });
});
