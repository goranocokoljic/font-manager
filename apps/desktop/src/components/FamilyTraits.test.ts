import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { DetectedTrait } from '../api/types';
import FamilyTraits from './FamilyTraits.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(traits: DetectedTrait[]) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const wrapper = mount(FamilyTraits, { props: { traits }, global: { plugins: [pinia] } });
  await flushPromises();
  return wrapper;
}

beforeEach(() => setClient(createMockClient({ library: 'wall' })));
afterEach(() => setClient(undefined));

describe('the trait tags (E49.10)', () => {
  it('shows nothing, not even the heading, for a face without detected traits', async () => {
    const w = await mountWith([]);
    expect(w.find('section').exists()).toBe(false);
    expect(w.text()).toBe('');
  });

  it('lists the tags as plain text with the example drawn and hidden from screen readers', async () => {
    const w = await mountWith([
      { traitId: 'O.roundness', value: 'round', confidence: 0.9 },
      { traitId: 'g.storeys', value: 'double', confidence: 0.8 },
      { traitId: 'e.bar', value: 'horizontal', confidence: 0.8 },
    ]);
    expect(w.get('h3').text()).toBe('Letter shapes');
    const tags = w.findAll('[data-testid="trait-tag"]');
    expect(tags.map((t) => t.get('.words').text().replace(/\s+/g, ' '))).toEqual([
      'O · round',
      // The label names the aspect already: not "g · storeys · double-storey".
      'g · double-storey',
      'e · bar · horizontal',
    ]);
    // The tag is text, not a control; only its action is a button.
    expect(tags[0]!.element.tagName).toBe('LI');
    expect(tags[0]!.attributes('role')).toBeUndefined();
    expect(tags[0]!.get('svg').attributes('aria-hidden')).toBe('true');
    expect(tags[0]!.get('svg path').attributes('d')).toBeTruthy();
    for (const dot of tags[1]!.findAll('.dot')) expect(dot.attributes('aria-hidden')).toBe('true');
  });

  it("emits the tag on 'find fonts with this'", async () => {
    const w = await mountWith([{ traitId: 'O.roundness', value: 'round', confidence: 0.9 }]);
    const find = w.get('button');
    expect(find.text()).toBe('find fonts with this');
    expect(find.attributes('aria-label')).toBe('Find fonts with O round');
    await find.trigger('click');
    expect(w.emitted('find')?.[0]?.[0]).toMatchObject({
      traitId: 'O.roundness',
      value: 'round',
      glyph: 'O',
    });
  });
});
