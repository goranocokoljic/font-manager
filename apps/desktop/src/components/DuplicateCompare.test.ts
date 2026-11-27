// E58.3: the sample chips of Compare copies follow the order `list_languages` answers. The
// presets here come in an order no rule gives (the user's two first, then not A to Z), and
// the languages that differ are asked for in yet another order, so a component that sorts,
// or lists them as it collects them, shows a different row.
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockDuplicateCopies, mockLanguages } from '../api/mock';
import type { LanguagePreset } from '../api/types';
import DuplicateCompare from './DuplicateCompare.vue';

const preset = (code: string, group: LanguagePreset['group']): LanguagePreset => {
  const found = mockLanguages.find((l) => l.code === code);
  if (!found) throw new Error(`no mock language ${code}`);
  return { ...found, group };
};

let wrapper: VueWrapper | undefined;
afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('DuplicateCompare sample languages (E58.3)', () => {
  it('lists the sample chips in the API order, the user languages first', async () => {
    setActivePinia(createPinia());
    const client = createMockClient({ copies: mockDuplicateCopies });
    setClient(client);
    const sets = await client.listDuplicateSets({});
    const set = sets.items.find((s) => s.postscriptName === 'Inter-Regular')!;
    const [a, b] = set.copies;
    const compared = await client.compareCopies({ a: a!.face!, b: b!.face! });
    // The languages that differ, collected in an order unlike the API's.
    const comparison = {
      ...compared,
      differences: {
        ...compared.differences!,
        languagesLost: ['el', 'sr-Cyrl'],
        languagesGained: ['de'],
        loclLost: [],
        loclGained: [],
      },
    };
    const presets = [
      preset('pl', 'mine'),
      preset('en', 'mine'),
      preset('sr-Cyrl', 'others'),
      preset('de', 'others'),
      preset('el', 'others'),
      preset('fr', 'others'),
    ];
    wrapper = mount(DuplicateCompare, {
      props: {
        set,
        comparison,
        languages: presets.slice(0, 2),
        presets,
        sample: 'Hamburgefonstiv',
        choices: {},
        keeper: null,
        fixed: () => false,
        trash: 'Trash',
      },
      attachTo: document.body,
    });
    await flushPromises();
    const chips = wrapper
      .get('[data-testid="compare-samples"]')
      .findAll('button[data-language]')
      .map((c) => c.attributes('data-language'));
    // Neither A to Z by name nor the order the codes were collected in.
    expect(chips).toEqual(['pl', 'en', 'sr-Cyrl', 'de', 'el']);
  });
});
