// E41.3: the top rows and the foot of the sidebar, and the rule that a section's actions
// are icon buttons. How the rows look is checked in the browser (e2e/sidebar.spec.ts).
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { Problem } from '../api/types';
import { createAppRouter } from '../router';
import { useCatalogsStore } from '../stores/catalogs';
import { useCommandsStore } from '../stores/commands';
import { useLibraryStore } from '../stores/library';
import { useProblemsStore } from '../stores/problems';
import { useQuickCollectionStore } from '../stores/quickCollection';
import { useSearchStore } from '../stores/search';
import CollectionsSidebar from './CollectionsSidebar.vue';
import LibraryViewsSidebar from './LibraryViewsSidebar.vue';
import OnlineCatalogsSidebar from './OnlineCatalogsSidebar.vue';
import ProblemsSidebar from './ProblemsSidebar.vue';
import ProjectSetsSidebar from './ProjectSetsSidebar.vue';
import SourcesSidebar from './SourcesSidebar.vue';
import TagsSidebar from './TagsSidebar.vue';

type Options = Parameters<typeof createMockClient>[0];

async function mountIt(component: object, options: Options = {}) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const client = createMockClient(options);
  setClient(client);
  await useLibraryStore().load();
  const router = createAppRouter();
  const w = mount(component, { global: { plugins: [pinia, router] }, attachTo: document.body });
  await flushPromises();
  return { w, client, router };
}

afterEach(() => {
  document.body.innerHTML = '';
  setClient(undefined);
});

describe('the top rows', () => {
  it('names each by its view and describes it with its count', async () => {
    const { w } = await mountIt(LibraryViewsSidebar, { favoriteFaces: [1, 6] });
    const rows = w.findAll('button.side-row');
    expect(rows.map((r) => r.get('.side-label').text())).toEqual([
      'All fonts',
      'Recently added',
      'Activated',
      'Favorites',
    ]);
    const favorites = w.get('[data-view="favorites"]');
    const count = favorites.get('.side-count');
    expect(count.text()).toBe('2');
    expect(favorites.attributes('aria-describedby')).toBe(count.attributes('id'));
    expect(count.attributes('aria-label')).toMatch(/^2 families$/);
    expect(favorites.findComponent({ name: 'UiIcon' }).exists()).toBe(true);
  });

  it('marks the open view, moves it on a click, and marks none under a catalog', async () => {
    const { w } = await mountIt(LibraryViewsSidebar);
    const current = () => w.findAll('[aria-current="true"]').map((r) => r.attributes('data-view'));
    expect(current()).toEqual(['all']);
    await w.get('[data-view="activated"]').trigger('click');
    expect(useSearchStore().filters.active).toBe(true);
    expect(current()).toEqual(['activated']);

    useCatalogsStore().show('adobeFonts');
    await flushPromises();
    expect(current()).toEqual([]);
    useCatalogsStore().close();
    useQuickCollectionStore().show();
    await flushPromises();
    expect(current()).toEqual([]);
  });
});

describe('the foot of the sidebar', () => {
  const problem: Problem = {
    kind: 'legacyFormat',
    path: '/fonts/old.pfb',
    detail: 'Type 1',
    face: null,
    other: null,
  } as unknown as Problem;

  it('shows the dot and the count of the problems, and opens the view', async () => {
    const { w, router } = await mountIt(ProblemsSidebar, { extraProblems: [problem, problem] });
    await useProblemsStore().load();
    await flushPromises();
    const total = useProblemsStore().total;
    expect(total).toBeGreaterThan(1);
    const link = w.get('[data-testid="problem-count"]');
    expect(link.text()).toBe(`${total} problems`);
    expect(w.find('[data-testid="problem-dot"]').exists()).toBe(true);
    // One count and no list per kind: the view filters by kind.
    expect(w.findAll('a')).toHaveLength(1);
    expect(w.get('section').attributes('aria-labelledby')).toBe('fm-problems-heading');
    expect(w.get('#fm-problems-heading').text()).toBe('Problems');
    await link.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('problems');
  });

  it('is one quiet line without a dot when there are no problems', async () => {
    const { w } = await mountIt(ProblemsSidebar);
    expect(useProblemsStore().total).toBe(0);
    expect(w.get('[data-testid="problem-count"]').text()).toBe('No problems');
    expect(w.find('[data-testid="problem-dot"]').exists()).toBe(false);
  });

  it('has the palette key, which opens the palette', async () => {
    const { w } = await mountIt(ProblemsSidebar);
    const button = w.get('button');
    expect(button.get('kbd').text()).toMatch(/K$/);
    expect(button.text()).toContain('Commands');
    await button.trigger('click');
    expect(useCommandsStore().open).toBe(true);
  });
});

describe('a section heading', () => {
  it.each([
    ['Collections', CollectionsSidebar, ['New collection', 'New smart collection']],
    ['Sources', SourcesSidebar, ['Add folder']],
    ['Project sets', ProjectSetsSidebar, ['Import set…']],
  ] as const)('of %s has its actions as icon buttons only', async (heading, component, labels) => {
    const { w } = await mountIt(component);
    expect(w.get('h2').text()).toBe(heading);
    const actions = w.get('[data-testid="section-actions"]').findAll('button');
    expect(actions.map((b) => b.attributes('aria-label'))).toEqual(labels);
    for (const b of actions) {
      expect(b.classes()).toContain('ui-icon-button');
      expect(b.text()).toBe('');
    }
  });

  it('folds its group, and the group remembers it', async () => {
    const { w } = await mountIt(SourcesSidebar);
    const fold = w.get('h2 button');
    expect(fold.attributes('aria-expanded')).toBe('true');
    await fold.trigger('click');
    expect(fold.attributes('aria-expanded')).toBe('false');
    expect((w.get('#fm-sources-list').element as HTMLElement).style.display).toBe('none');
    await fold.trigger('click');
  });
});

describe('an empty section', () => {
  it('of Collections is one quiet line, gone with the first collection', async () => {
    const { w, client } = await mountIt(CollectionsSidebar);
    expect(w.get('[data-testid="collections-empty"]').text()).toBe('No collections yet');
    expect(w.findAll('.side-quiet')).toHaveLength(1);
    await client.createCollection({ name: 'One' });
    await w.get('button[title="Make a collection"]').trigger('click');
    await flushPromises();
    expect(w.find('[data-testid="collections-empty"]').exists()).toBe(false);
  });

  it('of Tags is one quiet line and no key hint', async () => {
    const { w } = await mountIt(TagsSidebar);
    const empty = w.findAll('.side-quiet');
    expect(empty.map((e) => e.text())).toEqual(['No tags yet']);
    expect(w.find('kbd').exists()).toBe(false);
  });

  it('of Project sets is one quiet line', async () => {
    const { w } = await mountIt(ProjectSetsSidebar);
    expect(w.findAll('.side-quiet').map((e) => e.text())).toEqual(['No sets yet.']);
  });
});

describe('tags', () => {
  it('are chips that keep their count for screen readers and in the tooltip', async () => {
    const { w } = await mountIt(TagsSidebar, { faceTags: { 1: ['display'], 6: ['display'] } });
    const chip = w.get('li[data-tag="display"]');
    expect(chip.classes()).toContain('chip');
    expect(chip.attributes('title')).toBe('2 fonts');
    expect(chip.get('.count').attributes('aria-label')).toBe('2 fonts');
    expect(chip.get('.count').classes()).toContain('sr-only');
  });
});

describe('catalogs', () => {
  it('says "off" for a catalog that is off, and offers to turn it on with an icon', async () => {
    const { w } = await mountIt(OnlineCatalogsSidebar);
    await useCatalogsStore().load();
    await flushPromises();
    const first = useCatalogsStore().catalogs[0]!;
    expect(first.enabled).toBe(false);
    expect(w.get(`[data-testid="catalog-count-${first.id}"]`).text()).toBe('off');
    expect(w.get(`[data-testid="catalog-turn-on-${first.id}"]`).classes()).toContain(
      'ui-icon-button',
    );
  });

  it('shows the family count of a catalog that is on, with no way to turn it on', async () => {
    const { w } = await mountIt(OnlineCatalogsSidebar);
    const store = useCatalogsStore();
    await store.load();
    const first = store.catalogs[0]!;
    await store.setEnabled(first.id, true);
    await flushPromises();
    const on = store.catalogs.find((c) => c.id === first.id)!;
    expect(on.enabled).toBe(true);
    expect(w.get(`[data-testid="catalog-count-${on.id}"]`).text()).toBe(String(on.familyCount));
    expect(w.find(`[data-testid="catalog-turn-on-${on.id}"]`).exists()).toBe(false);
  });
});
