// E58.3: every list of languages shows the order `list_languages` answers, follows
// `languageOrderChanged` at once, and never sorts or slices by a rule of its own. The API
// here answers an order no rule would give (not A to Z, not file order, the user's two
// out of order), so a list that sorts, reverses or cuts shows a different one.
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockLanguages, mockMoreLanguages } from '../api/mock';
import type { Event, FaceLanguageCoverage, LanguagePreset } from '../api/types';
import FilterChips from '../components/FilterChips.vue';
import { useDuplicatesStore } from '../stores/duplicates';
import { useFamilyRailStore } from '../stores/familyRail';
import { useLibraryStore } from '../stores/library';
import { usePreviewStore } from '../stores/preview';
import { choicesFor, useRuleEditorStore } from '../stores/ruleEditor';
import { useSearchStore } from '../stores/search';
import {
  LANGUAGES_SHOWN,
  languageMenu,
  presetsIn,
  sampleLanguages,
  startsAfterMine,
} from './languages';

const presets = [...mockLanguages, ...mockMoreLanguages];

/** The API's answer: `mine` first (in the given order), then the rest as given. */
function answer(mine: string[], rest: string[]): LanguagePreset[] {
  const of = (code: string) => {
    const preset = presets.find((l) => l.code === code);
    if (!preset) throw new Error(`no mock language ${code}`);
    return preset;
  };
  return [
    ...mine.map((c) => ({ ...of(c), group: 'mine' as const })),
    ...rest.map((c) => ({ ...of(c), group: 'others' as const })),
  ];
}

const all = presets.map((l) => l.code);
const byName = [...presets].sort((a, b) => a.name.localeCompare(b.name));
// Z to A by name past the user's two: neither the file's order nor an A-to-Z sort.
const scrambled = (mine: string[]) =>
  answer(
    mine,
    byName
      .map((l) => l.code)
      .reverse()
      .filter((c) => !mine.includes(c)),
  );

let client: FmClient;
let current: LanguagePreset[];
let reads = 0;
const handlers: ((event: Event) => void)[] = [];
const codes = (list: readonly { code: string }[]) => list.map((l) => l.code);

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({ moreLanguages: true });
  current = scrambled(['pl', 'el']);
  reads = 0;
  handlers.length = 0;
  client.listLanguages = async () => {
    reads += 1;
    return current.map((l) => ({ ...l }));
  };
  client.onEvent = async (handler) => {
    handlers.push(handler);
    return () => undefined;
  };
  setClient(client);
});
afterEach(() => setClient(undefined));

function send(event: Event): void {
  for (const h of handlers) h(event);
}

describe('the test data', () => {
  it('has more languages than the drawer shows and an order no sort gives', () => {
    expect(current.length).toBeGreaterThan(LANGUAGES_SHOWN + 2);
    expect(codes(current)).not.toEqual(all);
    expect(codes(current)).not.toEqual(codes(byName));
    expect(codes(current).slice(0, 2)).toEqual(['pl', 'el']);
  });
});

describe('every store keeps the API order (E58.3)', () => {
  it('search, preview, duplicates and the rule editor list the answer as it is', async () => {
    const search = useSearchStore();
    await search.loadLanguages();
    const want = codes(current);
    expect(codes(search.languages)).toEqual(want);
    expect(codes(usePreviewStore().languagePicks)).toEqual(want);

    const duplicates = useDuplicatesStore();
    await duplicates.load();
    expect(codes(duplicates.presets)).toEqual(want);

    const editor = useRuleEditorStore();
    await editor.openNew();
    const menu = choicesFor('language', editor.choices) ?? [];
    expect(menu.map((c) => c.value)).toEqual(want);
    // The hairline sits before the first language that is not the user's, and only there.
    expect(menu.flatMap((c, i) => (c.separatorBefore ? [i] : []))).toEqual([2]);
  });

  it('the family rail lists the covered languages in the API order', async () => {
    const search = useSearchStore();
    await search.loadLanguages();
    const rail = useFamilyRailStore();
    // fm-core's coverage answers in its own order; one code the list does not name ends it.
    const row = (code: string): FaceLanguageCoverage => ({
      code,
      name: code,
      covered: 3,
      total: 3,
      hasLocl: false,
    });
    rail.languages = ['en', 'de', 'xx', 'fr', 'el'].map(row);
    const want = codes(current).filter((c) => ['en', 'de', 'fr', 'el'].includes(c));
    expect(codes(rail.complete)).toEqual([...want, 'xx']);
    expect(codes(rail.complete)[0]).toBe('el');
  });

  it('a language event reads the list again and every list follows; others do not', async () => {
    const search = useSearchStore();
    const preview = usePreviewStore();
    const editor = useRuleEditorStore();
    await search.loadLanguages();
    await editor.openNew();
    const before = reads;

    send({ type: 'activationChanged' } as Event);
    await flushPromises();
    expect(reads).toBe(before);

    current = scrambled(['fr']);
    send({ type: 'languageOrderChanged' });
    await flushPromises();
    expect(reads).toBe(before + 1);
    const want = codes(current);
    expect(codes(search.languages)).toEqual(want);
    expect(codes(preview.languagePicks)).toEqual(want);
    expect(codes(useDuplicatesStore().presets)).toEqual(want);
    const menu = choicesFor('language', editor.choices) ?? [];
    expect(menu.map((c) => c.value)).toEqual(want);
    expect(menu.flatMap((c, i) => (c.separatorBefore ? [i] : []))).toEqual([1]);
  });

  it('subscribes once, however often the languages are loaded', async () => {
    const search = useSearchStore();
    await search.loadLanguages();
    await search.loadLanguages();
    await usePreviewStore().loadPangrams();
    expect(handlers).toHaveLength(1);
    expect(reads).toBe(1);
  });
});

describe('the filter drawer (E58.3)', () => {
  let wrapper: VueWrapper | undefined;
  afterEach(() => {
    wrapper?.unmount();
    wrapper = undefined;
  });

  async function drawer() {
    const pinia = createPinia();
    setActivePinia(pinia);
    await useLibraryStore().load();
    await useSearchStore().loadLanguages();
    wrapper = mount(FilterChips, {
      global: { plugins: [pinia], stubs: { teleport: true } },
      attachTo: document.body,
    });
    await flushPromises();
    await wrapper.get('button.opener').trigger('click');
    await flushPromises();
    return wrapper;
  }
  const chips = (w: VueWrapper) =>
    w
      .get('[role="group"][aria-label="Languages"]')
      .findAll('button.chip:not(.more)')
      .map((b) => b.text());
  const names = (list: readonly LanguagePreset[]) => list.map((l) => l.name);

  it('shows the first ten of the answer, then all and back', async () => {
    const w = await drawer();
    expect(chips(w)).toEqual(names(current.slice(0, LANGUAGES_SHOWN)));
    const more = w.get('[role="group"][aria-label="Languages"] button.more');
    await more.trigger('click');
    expect(chips(w)).toEqual(names(current));
    await more.trigger('click');
    expect(chips(w)).toEqual(names(current.slice(0, LANGUAGES_SHOWN)));
  });

  it('keeps a chosen language past the first ten in its place', async () => {
    const w = await drawer();
    const late = current[current.length - 1]!;
    useSearchStore().setFilters({ supports: { kind: 'languages', codes: [late.code] } });
    await flushPromises();
    expect(chips(w)).toEqual([...names(current.slice(0, LANGUAGES_SHOWN)), late.name]);
  });

  it('follows a language event without a reload', async () => {
    const w = await drawer();
    current = scrambled(['cs', 'en']);
    send({ type: 'languageOrderChanged' });
    await flushPromises();
    expect(chips(w).slice(0, 2)).toEqual(names(current.slice(0, 2)));
  });
});

describe('the helpers', () => {
  it('draw the line only after the user languages, never at the top', () => {
    const list = answer(['de', 'fr'], ['en', 'cs']);
    expect(list.map((_, i) => startsAfterMine(list, i))).toEqual([false, false, true, false]);
    const none = answer([], ['en', 'cs']);
    expect(none.map((_, i) => startsAfterMine(none, i))).toEqual([false, false]);
    const menu = languageMenu(list, (l) => ({ value: l.code, label: l.name }));
    expect(menu.map((o) => o.separatorBefore ?? false)).toEqual([false, false, true, false]);
  });

  it('Compare copies lists its sample languages in the API order, with a sample only', () => {
    // The user's languages, then the ones that differ: asked for in that order, shown in the
    // API's. Maltese has no pangram and no glyph line.
    const list = answer(['de', 'fr'], ['en', 'mt', 'cs', 'pl']);
    expect(codes(sampleLanguages(list, ['fr', 'de', 'pl', 'mt', 'en', 'zz']))).toEqual([
      'de',
      'fr',
      'en',
      'pl',
    ]);
  });

  it('keep the API order, not the order of the codes asked for', () => {
    const list = answer(['de', 'fr'], ['en', 'cs']);
    expect(codes(presetsIn(list, ['cs', 'de', 'zz']))).toEqual(['de', 'cs']);
  });
});
